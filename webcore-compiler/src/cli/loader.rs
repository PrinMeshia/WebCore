//! File loading utilities: parse `.webc` sources, locales, and data imports.

use crate::core::ast;
use crate::parser;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// Iterate files with a given extension in a flat directory, calling `loader(path, source)` for each.
/// Loader failure: keeps parse errors structured (file/span/message) so
/// `webc check --json` can emit precise editor diagnostics, while remaining
/// printable for the human-facing build/check paths.
pub(crate) enum LoadError {
    Parse(crate::parser::ParseError),
    Other(String),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::Parse(e) => write!(f, "{e}"),
            LoadError::Other(s) => write!(f, "{s}"),
        }
    }
}

impl From<String> for LoadError {
    fn from(s: String) -> Self {
        LoadError::Other(s)
    }
}

pub(crate) fn load_webc_dir<F>(
    dir: &Path,
    label: &str,
    ext: &str,
    mut loader: F,
) -> Result<(), LoadError>
where
    F: FnMut(&Path, &str) -> Result<(), LoadError>,
{
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir).map_err(|e| format!("Failed to read {label}: {e}"))? {
        let entry = entry.map_err(|e| format!("Failed to read entry: {e}"))?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some(ext) {
            let source = fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
            loader(&path, &source)?;
        }
    }
    Ok(())
}

/// Walk a directory tree and call `visitor` for every file found.
/// Subdirectories are traversed recursively.
pub(crate) fn walk_files<F>(dir: &Path, mut visitor: F) -> std::io::Result<()>
where
    F: FnMut(&Path) -> std::io::Result<()>,
{
    walk_files_inner(dir, &mut visitor)
}

fn walk_files_inner<F>(dir: &Path, visitor: &mut F) -> std::io::Result<()>
where
    F: FnMut(&Path) -> std::io::Result<()>,
{
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let p = entry.path();
        if p.is_dir() {
            walk_files_inner(&p, visitor)?;
        } else {
            visitor(&p)?;
        }
    }
    Ok(())
}

/// Load and parse all `.webc` source files and locale translations into a single document.
///
/// Scans `src/app.webc`, `src/layouts/`, `src/components/`, `src/pages/`, and `locales/`.
/// Does not touch `dist/` or run any build tools — pure parsing.
pub(crate) fn load_webc_document(default_locale: &str) -> Result<ast::WebCoreDocument, LoadError> {
    let mut document = ast::WebCoreDocument {
        app: None,
        store: Vec::new(),
        store_computed: Vec::new(),
        locales: BTreeMap::new(),
        default_locale: default_locale.to_string(),
        wasm_module: None,
        layouts: BTreeMap::new(),
        pages: BTreeMap::new(),
        components: BTreeMap::new(),
        imports: Vec::new(),
        data_imports: BTreeMap::new(),
        source_files: BTreeMap::new(),
    };

    // Load app.webc first
    let app_path = Path::new("src/app.webc");
    if app_path.exists() {
        let content =
            fs::read_to_string(app_path).map_err(|e| format!("Failed to read app.webc: {e}"))?;
        let parsed = parser::parse_webc(&content).map_err(|mut e| {
            e.file = Some(app_path.to_path_buf());
            LoadError::Parse(e)
        })?;
        document.app = parsed.app;
        document.store.extend(parsed.store);
        document.imports.extend(parsed.imports);
    }

    // Load layouts
    load_webc_dir(
        Path::new("src/layouts"),
        "layouts",
        "webc",
        |path, source| {
            let parsed = parser::parse_webc(source).map_err(|mut e| {
                e.file = Some(path.to_path_buf());
                LoadError::Parse(e)
            })?;
            for (name, layout) in parsed.layouts {
                document
                    .source_files
                    .insert(name.clone(), path.to_path_buf());
                document.layouts.insert(name, layout);
            }
            document.imports.extend(parsed.imports);
            Ok(())
        },
    )?;

    // Load components
    load_webc_dir(
        Path::new("src/components"),
        "components",
        "webc",
        |path, source| {
            let parsed = parser::parse_webc(source).map_err(|mut e| {
                e.file = Some(path.to_path_buf());
                LoadError::Parse(e)
            })?;
            for (name, component) in parsed.components {
                document
                    .source_files
                    .insert(name.clone(), path.to_path_buf());
                document.components.insert(name, component);
            }
            document.imports.extend(parsed.imports);
            Ok(())
        },
    )?;

    // Load pages
    load_webc_dir(Path::new("src/pages"), "pages", "webc", |path, source| {
        let parsed = parser::parse_webc(source).map_err(|mut e| {
            e.file = Some(path.to_path_buf());
            LoadError::Parse(e)
        })?;
        for (name, page) in parsed.pages {
            document
                .source_files
                .insert(name.clone(), path.to_path_buf());
            document.pages.insert(name, page);
        }
        for (name, component) in parsed.components {
            document
                .source_files
                .insert(name.clone(), path.to_path_buf());
            document.components.insert(name, component);
        }
        document.imports.extend(parsed.imports);
        Ok(())
    })?;

    // Load locale files from locales/ directory (flat TOML: key = "value")
    load_webc_dir(Path::new("locales"), "locales/", "toml", |path, source| {
        if let Some(code) = path.file_stem().and_then(|s| s.to_str()) {
            let entries: BTreeMap<String, String> = toml::from_str(source)
                .map_err(|e| format!("Failed to parse locale {}: {e}", path.display()))?;
            document.locales.insert(code.to_string(), entries);
            println!("🌍 Loaded locale: {code}");
        }
        Ok(())
    })?;

    Ok(document)
}

/// Build a temporary `WebCoreDocument` that is a clone of `base` with one extra
/// synthetic page inserted.
///
/// Isolates the intent of cloning the document for component-as-page generation.
/// The fields that actually need to be copied are `pages` (to add the synthetic
/// entry) plus all shared-read fields (`layouts`, `components`, `store`, etc.).
/// Because `WebCoreDocument` doesn't support partial borrows we still clone the
/// whole struct, but the cost is bounded to one clone per `*Page` component,
/// whereas previously the inline clone was easy to miss during review.
///
/// Future work: replace with an `Arc`-sharing approach once the AST supports it.
pub(crate) fn build_temp_doc_for_component(
    base: &ast::WebCoreDocument,
    page: ast::Page,
    page_name: &str,
) -> ast::WebCoreDocument {
    let mut temp = base.clone();
    temp.pages.insert(page_name.to_string(), page);
    temp
}

/// Resolve `import name from "path"` declarations: read each JSON/TOML file
/// and store its content as a compact JSON string in `document.data_imports`.
///
/// Security: paths are canonicalized and must stay inside the project
/// directory — `import x from "../../etc/passwd"` is rejected.
pub(crate) fn resolve_data_imports(document: &mut ast::WebCoreDocument) -> Result<(), String> {
    if document.imports.is_empty() {
        return Ok(());
    }
    let root = std::env::current_dir()
        .and_then(|d| d.canonicalize())
        .map_err(|e| format!("Cannot resolve project directory: {e}"))?;

    for imp in &document.imports {
        let canon = Path::new(&imp.path)
            .canonicalize()
            .map_err(|e| format!("import '{}': cannot read '{}': {e}", imp.name, imp.path))?;
        if !canon.starts_with(&root) {
            return Err(format!(
                "import '{}': path '{}' escapes the project directory",
                imp.name, imp.path
            ));
        }
        let raw = fs::read_to_string(&canon)
            .map_err(|e| format!("import '{}': failed to read '{}': {e}", imp.name, imp.path))?;
        let json = match canon.extension().and_then(|e| e.to_str()) {
            Some("json") => {
                let value: serde_json::Value = serde_json::from_str(&raw).map_err(|e| {
                    format!(
                        "import '{}': '{}' is not valid JSON: {e}",
                        imp.name, imp.path
                    )
                })?;
                serde_json::to_string(&value).unwrap_or_default()
            }
            Some("toml") => {
                let value: toml::Value = toml::from_str(&raw).map_err(|e| {
                    format!(
                        "import '{}': '{}' is not valid TOML: {e}",
                        imp.name, imp.path
                    )
                })?;
                serde_json::to_string(&value).map_err(|e| {
                    format!("import '{}': TOML→JSON conversion failed: {e}", imp.name)
                })?
            }
            _ => {
                return Err(format!(
                    "import '{}': unsupported file type '{}' (only .json and .toml)",
                    imp.name, imp.path
                ))
            }
        };
        document.data_imports.insert(imp.name.clone(), json);
        println!("📦 Data import: {} ← {}", imp.name, imp.path);
    }
    Ok(())
}

/// Expand an SSG collection route into one entry per item of the collection.
///
/// `route` must contain a `:param` segment (e.g. `/post/:slug`); `items_json`
/// must be a JSON array of objects each carrying the `param` field.
/// Returns `(relative_output_path, param_name, param_value)` triples.
pub(crate) fn expand_collection(
    route: &str,
    items_json: &str,
) -> Result<Vec<(String, String, String)>, String> {
    let param = route
        .split('/')
        .find_map(|seg| seg.strip_prefix(':'))
        .ok_or_else(|| format!("collection route '{route}' has no ':param' segment"))?;

    let items: serde_json::Value = serde_json::from_str(items_json)
        .map_err(|e| format!("collection data for '{route}' is not valid JSON: {e}"))?;
    let arr = items
        .as_array()
        .ok_or_else(|| format!("collection data for '{route}' must be a JSON array"))?;

    let mut out = Vec::new();
    for item in arr {
        let value = match item.get(param) {
            Some(serde_json::Value::String(s)) => s.clone(),
            Some(serde_json::Value::Number(n)) => n.to_string(),
            Some(_) => {
                return Err(format!(
                    "collection item field '{param}' must be a string or number (route '{route}')"
                ))
            }
            None => {
                return Err(format!(
                    "collection item is missing field '{param}' required by route '{route}'"
                ))
            }
        };
        // The value becomes a directory name under dist/ — reject traversal attempts.
        if value.is_empty()
            || value.contains('/')
            || value.contains('\\')
            || value.contains("..")
            || value.contains('\0')
        {
            return Err(format!(
                "collection item field '{param}' has unsafe value '{value}' (route '{route}')"
            ));
        }
        let rel = route
            .trim_start_matches('/')
            .replace(&format!(":{param}"), &value);
        out.push((format!("{rel}/index.html"), param.to_string(), value));
    }
    Ok(out)
}
