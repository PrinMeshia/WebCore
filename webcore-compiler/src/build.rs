//! Build pipeline: compile `.webc` sources into `dist/`.

use crate::{ast, codegen, css_processor, error, parser, ssg, theme};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

// ── Project configuration ────────────────────────────────────────────────────

#[derive(Debug)]
pub(crate) struct Config {
    pub(crate) app_title: String,
    pub(crate) app_lang: String,
    pub(crate) locale: String,
    pub(crate) mode: String,
}

#[derive(Debug, Deserialize)]
struct WebcToml {
    app: Option<AppSection>,
}

#[derive(Debug, Deserialize)]
struct AppSection {
    title: Option<String>,
    lang: Option<String>,
    locale: Option<String>,
    mode: Option<String>,
}

pub(crate) fn read_config() -> Result<Config, String> {
    let config_path = Path::new("webc.toml");
    if !config_path.exists() {
        return Err("webc.toml not found".to_string());
    }

    let content =
        fs::read_to_string(config_path).map_err(|e| format!("Failed to read webc.toml: {e}"))?;

    let parsed: WebcToml =
        toml::from_str(&content).map_err(|e| format!("Failed to parse webc.toml: {e}"))?;
    let app_title = parsed
        .app
        .as_ref()
        .and_then(|a| a.title.clone())
        .unwrap_or_else(|| "WebCore App".to_string());
    let app_lang = parsed
        .app
        .as_ref()
        .and_then(|a| a.lang.clone())
        .unwrap_or_else(|| "fr".to_string());
    let mode = parsed
        .app
        .as_ref()
        .and_then(|a| a.mode.clone())
        .unwrap_or_else(|| "dev".to_string());
    let locale = parsed
        .app
        .as_ref()
        .and_then(|a| a.locale.clone())
        .unwrap_or_else(|| app_lang.clone());

    Ok(Config {
        app_title,
        app_lang,
        locale,
        mode,
    })
}

// ── WASM module detection ────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct WasmCargoToml {
    package: WasmPackage,
}

#[derive(Debug, Deserialize)]
struct WasmPackage {
    name: String,
}

/// Read the `[package] name` from a `Cargo.toml` file and return its `snake_case` form.
pub(crate) fn read_wasm_module_name(path: &Path) -> Result<String, String> {
    let content =
        fs::read_to_string(path).map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
    let parsed: WasmCargoToml =
        toml::from_str(&content).map_err(|e| format!("Failed to parse {}: {e}", path.display()))?;
    Ok(parsed.package.name.replace('-', "_"))
}

// ── File loading utilities ───────────────────────────────────────────────────

/// Iterate files with a given extension in a flat directory, calling `loader(path, source)` for each.
pub(crate) fn load_webc_dir<F>(
    dir: &Path,
    label: &str,
    ext: &str,
    mut loader: F,
) -> Result<(), String>
where
    F: FnMut(&Path, &str) -> Result<(), String>,
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
pub(crate) fn load_webc_document(default_locale: &str) -> Result<ast::WebCoreDocument, String> {
    let mut document = ast::WebCoreDocument {
        app: None,
        store: Vec::new(),
        locales: HashMap::new(),
        default_locale: default_locale.to_string(),
        wasm_module: None,
        layouts: HashMap::new(),
        pages: HashMap::new(),
        components: HashMap::new(),
        imports: Vec::new(),
        data_imports: HashMap::new(),
    };

    // Load app.webc first
    let app_path = Path::new("src/app.webc");
    if app_path.exists() {
        let content =
            fs::read_to_string(app_path).map_err(|e| format!("Failed to read app.webc: {e}"))?;
        let parsed =
            parser::parse_webc(&content).map_err(|e| format!("Parse error in app.webc:\n{e}"))?;
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
            let parsed = parser::parse_webc(source)
                .map_err(|e| format!("Parse error in {}:\n{}", path.display(), e))?;
            for (name, layout) in parsed.layouts {
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
            let parsed = parser::parse_webc(source)
                .map_err(|e| format!("Parse error in {}:\n{}", path.display(), e))?;
            for (name, component) in parsed.components {
                document.components.insert(name, component);
            }
            document.imports.extend(parsed.imports);
            Ok(())
        },
    )?;

    // Load pages
    load_webc_dir(Path::new("src/pages"), "pages", "webc", |path, source| {
        let parsed = parser::parse_webc(source)
            .map_err(|e| format!("Parse error in {}:\n{}", path.display(), e))?;
        for (name, page) in parsed.pages {
            document.pages.insert(name, page);
        }
        for (name, component) in parsed.components {
            document.components.insert(name, component);
        }
        document.imports.extend(parsed.imports);
        Ok(())
    })?;

    // Load locale files from locales/ directory (flat TOML: key = "value")
    load_webc_dir(Path::new("locales"), "locales/", "toml", |path, source| {
        if let Some(code) = path.file_stem().and_then(|s| s.to_str()) {
            let entries: HashMap<String, String> = toml::from_str(source)
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

// ── Asset utilities ──────────────────────────────────────────────────────────

/// FNV-1a 32-bit hash — returns an 8 hex-char string.
#[must_use]
pub(crate) fn fnv1a_hash(data: &[u8]) -> String {
    let mut h: u32 = 2_166_136_261;
    for &b in data {
        h ^= u32::from(b);
        h = h.wrapping_mul(16_777_619);
    }
    format!("{h:08x}")
}

/// Image file extensions that are subject to content-hash fingerprinting.
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "svg", "ico", "avif"];

/// For every image file directly inside `public_dir`, compute a content hash,
/// copy the file to `assets_dir/<stem>.<hash>.<ext>`, and return a mapping
/// `"original.png"` → `"original.<hash>.png"`.
pub(crate) fn fingerprint_images(
    public_dir: &Path,
    assets_dir: &Path,
) -> Result<HashMap<String, String>, String> {
    let mut map: HashMap<String, String> = HashMap::new();

    fn walk(
        dir: &Path,
        assets_dir: &Path,
        map: &mut HashMap<String, String>,
    ) -> Result<(), String> {
        let entries =
            fs::read_dir(dir).map_err(|e| format!("Failed to read {}: {e}", dir.display()))?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, assets_dir, map)?;
                continue;
            }
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            if !IMAGE_EXTENSIONS.contains(&ext.as_str()) {
                continue;
            }
            let file_name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            let bytes =
                fs::read(&path).map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
            let hash = fnv1a_hash(&bytes);
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(&file_name);
            let hashed_name = format!("{stem}.{hash}.{ext}");
            let dst = assets_dir.join(&hashed_name);
            fs::copy(&path, &dst).map_err(|e| {
                format!("Failed to copy {} → {}: {e}", path.display(), dst.display())
            })?;
            map.insert(file_name, hashed_name);
        }
        Ok(())
    }

    walk(public_dir, assets_dir, &mut map)?;
    Ok(map)
}

/// Post-process all `.html` files under `dist_dir` and all `.css` files under
/// `dist_dir/assets/`, replacing `/assets/<original>` references with
/// `/assets/<hashed>`.
pub(crate) fn rewrite_asset_refs(dist_dir: &Path, map: &HashMap<String, String>) {
    // Rewrite HTML files (any depth)
    rewrite_in_dir(dist_dir, "html", map, false);
    // Rewrite CSS files in dist/assets/
    let assets_dir = dist_dir.join("assets");
    if assets_dir.is_dir() {
        rewrite_in_dir(&assets_dir, "css", map, true);
    }
}

fn rewrite_in_dir(dir: &Path, ext: &str, map: &HashMap<String, String>, css_mode: bool) {
    let rewrite_file = |p: &Path| -> std::io::Result<()> {
        if p.extension().and_then(|e| e.to_str()) == Some(ext) {
            if let Ok(content) = fs::read_to_string(p) {
                let mut updated = content.clone();
                for (orig, hashed) in map {
                    if css_mode {
                        // In CSS: url("/assets/orig") and url('/assets/orig')
                        let dq = format!(r#"url("/assets/{orig}")"#);
                        let dq_new = format!(r#"url("/assets/{hashed}")"#);
                        let sq = format!("url('/assets/{orig}')");
                        let sq_new = format!("url('/assets/{hashed}')");
                        updated = updated.replace(&dq, &dq_new);
                        updated = updated.replace(&sq, &sq_new);
                    } else {
                        // In HTML: /assets/orig (bare path)
                        let old_ref = format!("/assets/{orig}");
                        let new_ref = format!("/assets/{hashed}");
                        updated = updated.replace(&old_ref, &new_ref);
                    }
                }
                if updated != content {
                    let _ = fs::write(p, updated);
                }
            }
        }
        Ok(())
    };
    if css_mode {
        // CSS mode: flat scan of a single directory (assets/), no recursion
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() {
                    let _ = rewrite_file(&p);
                }
            }
        }
    } else {
        let _ = walk_files(dir, rewrite_file);
    }
}

pub(crate) fn copy_dir_recursive(src: &Path, dst: &Path, minify: bool) -> Result<(), String> {
    if src.is_dir() {
        let src_owned = src.to_path_buf();
        let dst_owned = dst.to_path_buf();
        fs::create_dir_all(&dst_owned)
            .map_err(|e| format!("Failed to create dir {}: {e}", dst_owned.display()))?;
        walk_files(src, |file_path| {
            let rel = file_path.strip_prefix(&src_owned).unwrap_or(file_path);
            let dst_path = dst_owned.join(rel);
            if let Some(parent) = dst_path.parent() {
                fs::create_dir_all(parent)?;
            }
            if minify && file_path.extension().and_then(|e| e.to_str()) == Some("css") {
                let raw = fs::read_to_string(file_path)?;
                let minified = css_processor::minify_css(&raw).map_err(std::io::Error::other)?;
                fs::write(&dst_path, minified)?;
            } else {
                fs::copy(file_path, &dst_path)?;
            }
            Ok(())
        })
        .map_err(|e| format!("Failed to copy {}: {e}", src.display()))?;
    } else if minify && src.extension().and_then(|e| e.to_str()) == Some("css") {
        let raw = fs::read_to_string(src)
            .map_err(|e| format!("Failed to read {}: {e}", src.display()))?;
        let minified = css_processor::minify_css(&raw)?;
        fs::write(dst, minified).map_err(|e| format!("Failed to write {}: {e}", dst.display()))?;
    } else {
        fs::copy(src, dst)
            .map_err(|e| format!("Failed to copy {} to {}: {e}", src.display(), dst.display()))?;
    }
    Ok(())
}

fn patch_html_files(dir: &Path, js_src: &str) {
    let _ = walk_files(dir, |p| {
        if p.extension().and_then(|e| e.to_str()) == Some("html") {
            if let Ok(html) = fs::read_to_string(p) {
                let patched = html.replace(r#"src="/assets/webcore.js""#, js_src);
                if patched != html {
                    let _ = fs::write(p, patched);
                }
            }
        }
        Ok(())
    });
}

/// Replace all occurrences of `from` with `to` in every HTML file under `dir`.
pub(crate) fn replace_in_html_files(dir: &Path, from: &str, to: &str) {
    let _ = walk_files(dir, |p| {
        if p.extension().and_then(|e| e.to_str()) == Some("html") {
            if let Ok(html) = fs::read_to_string(p) {
                let patched = html.replace(from, to);
                if patched != html {
                    let _ = fs::write(p, patched);
                }
            }
        }
        Ok(())
    });
}

// ── Data imports & SSG collections ───────────────────────────────────────────

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

/// Compute a SHA-256 SRI hash string (`sha256-<base64>`) for the given data.
fn sri_hash(data: &[u8]) -> String {
    use base64::{engine::general_purpose, Engine as _};
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(data);
    format!("sha256-{}", general_purpose::STANDARD.encode(hash))
}

// ── Output formatting ────────────────────────────────────────────────────────

fn fmt_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else {
        format!("{:.1} kB", bytes as f64 / 1024.0)
    }
}

pub(crate) fn fmt_bytes(b: u64) -> String {
    if b >= 1024 {
        format!("{:.1} kB", b as f64 / 1024.0)
    } else {
        format!("{b} B")
    }
}

pub(crate) fn print_dist_tree(dist_dir: &Path, minified: bool) {
    // Collect all files recursively
    let mut files: Vec<(String, u64)> = Vec::new();
    fn collect(dir: &Path, prefix: &str, out: &mut Vec<(String, u64)>) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();
                let rel = if prefix.is_empty() {
                    name.clone()
                } else {
                    format!("{prefix}/{name}")
                };
                if path.is_dir() {
                    collect(&path, &rel, out);
                } else if let Ok(meta) = fs::metadata(&path) {
                    out.push((rel, meta.len()));
                }
            }
        }
    }
    collect(dist_dir, "", &mut files);

    // Sort: html first (alpha), then js, then css, then rest
    files.sort_by(|(a, _), (b, _)| {
        fn rank(s: &str) -> u8 {
            if s.ends_with(".html") {
                0
            } else if s.ends_with(".js") {
                1
            } else if s.ends_with(".css") {
                2
            } else {
                3
            }
        }
        rank(a).cmp(&rank(b)).then(a.cmp(b))
    });

    let total_bytes: u64 = files.iter().map(|(_, s)| s).sum();
    let max_name = files.iter().map(|(n, _)| n.len()).max().unwrap_or(10);

    println!("\ndist/");
    let count = files.len();
    for (i, (name, size)) in files.iter().enumerate() {
        let branch = if i + 1 == count {
            "└──"
        } else {
            "├──"
        };
        println!(
            "  {}  {:<width$}  {}",
            branch,
            name,
            fmt_size(*size),
            width = max_name
        );
    }
    let mode_label = if minified { "minified" } else { "dev" };
    println!(
        "\n  {} file{}  {}  ({})\n",
        count,
        if count == 1 { "" } else { "s" },
        fmt_size(total_bytes),
        mode_label,
    );
}

/// Print a bundle analysis table showing which runtime features were included
/// or tree-shaken, along with estimated byte contributions.
pub(crate) fn print_bundle_analysis(js: &str) {
    /// (marker_string, human_label, estimated_bytes)
    const FEATURES: &[(&str, &str, usize)] = &[
        ("bindIf", "bindIf (conditionals)", 320),
        ("bindFor", "bindFor (loops)", 512),
        ("bindAttrs", "bindAttrs (dyn attrs)", 180),
        ("bindValidation", "bindValidation", 640),
        ("const LOCALES=", "i18n / t()", 210),
        ("const ROUTES=", "router (param routes)", 380),
        ("const toFile=", "router (simple nav)", 90),
        ("const WASM=", "WASM loader", 120),
        ("DESTROY_HOOKS", "on:destroy hooks", 80),
        ("const COMPUTED=", "computed vars", 95),
    ];

    // Rough estimate: state-class boilerplate ~350 bytes per reactive component.
    let core_bytes = js.matches("class _S").count() * 350 + 420;

    let mut total: u64 = core_bytes as u64;
    println!("\n  Bundle analysis");
    println!("  ──────────────────────────────────────────────");
    println!("  {:<35} {:<10} Status", "Feature", "Size");
    println!("  {:<35} {:<10} ──────", "───────", "────");
    println!(
        "  {:<35} {:<10} ✓ included",
        "runtime core",
        fmt_bytes(core_bytes as u64)
    );

    for &(marker, label, est) in FEATURES {
        let included = js.contains(marker);
        if included {
            total += est as u64;
        }
        let status = if included {
            "✓ included"
        } else {
            "- tree-shaken"
        };
        let size_str = if included {
            fmt_bytes(est as u64)
        } else {
            "   —".to_string()
        };
        println!("  {:<35} {:<10} {}", label, size_str, status);
    }

    println!("  {}", "─".repeat(54));
    println!(
        "  {:<35} {:<10}",
        "estimated total (unminified)",
        fmt_bytes(total)
    );
    println!();
}

// ── Main build pipeline ──────────────────────────────────────────────────────

/// Compile the current project (reads from `src/`, writes to `dist/`).
pub(crate) fn build_project() -> Result<(), error::CompileErrors> {
    println!("🔨 Building WebCore project...");

    // Read project config
    let config = read_config()?;
    println!("📁 Project: {}", config.app_title);

    // Create dist/ and dist/assets/
    let dist_dir = Path::new("dist");
    if dist_dir.exists() {
        fs::remove_dir_all(dist_dir).map_err(|e| error::CompileError::Io {
            path: dist_dir.to_path_buf(),
            source: e,
        })?;
    }
    fs::create_dir_all(dist_dir).map_err(|e| error::CompileError::Io {
        path: dist_dir.to_path_buf(),
        source: e,
    })?;
    let assets_dir = dist_dir.join("assets");
    fs::create_dir_all(&assets_dir).map_err(|e| error::CompileError::Io {
        path: assets_dir.clone(),
        source: e,
    })?;

    // Load theme
    let theme = if Path::new("theme.toml").exists() {
        Some(theme::load_theme("theme.toml")?)
    } else {
        println!("⚠️  No theme.toml found, using default theme");
        None
    };

    // Load and parse all WebCore files
    let mut document = load_webc_document(&config.locale)?;

    // Resolve build-time data imports (JSON/TOML → document.data_imports)
    resolve_data_imports(&mut document)?;

    // Detect and compile WASM module (wasm/Cargo.toml → dist/wasm/)
    let wasm_cargo = Path::new("wasm/Cargo.toml");
    if wasm_cargo.exists() {
        match read_wasm_module_name(wasm_cargo) {
            Ok(module_name) => {
                println!("🦀 WASM module detected: {module_name}");
                document.wasm_module = Some(module_name.clone());

                let wasm_out = assets_dir.join("wasm");
                let status = std::process::Command::new("wasm-pack")
                    .args([
                        "build",
                        "--target",
                        "web",
                        "--out-dir",
                        &wasm_out.to_string_lossy(),
                    ])
                    .current_dir("wasm")
                    .status();

                match status {
                    Ok(s) if s.success() => {
                        println!("✅ WASM compiled: dist/wasm/{module_name}.js");
                    }
                    Ok(_) => {
                        println!(
                            "⚠️  wasm-pack build failed — JS loader included, compile manually"
                        );
                    }
                    Err(_) => {
                        println!("⚠️  wasm-pack not found — JS loader included, compile manually");
                    }
                }
            }
            Err(e) => {
                println!("⚠️  Could not read wasm/Cargo.toml: {e}");
            }
        }
    }

    // Collect any CSS files from public/ to include in <head>
    let extra_css_files: Vec<String> = {
        let public_dir = Path::new("public");
        if public_dir.exists() {
            let mut files = Vec::new();
            if let Ok(entries) = fs::read_dir(public_dir) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.extension().and_then(|e| e.to_str()) == Some("css") {
                        if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                            files.push(name.to_string());
                        }
                    }
                }
            }
            files.sort();
            files
        } else {
            Vec::new()
        }
    };
    let options = codegen::codegen_html::HtmlPageOptions {
        lang: config.app_lang.clone(),
        title: config.app_title.clone(),
        extra_css_files,
        critical_css: None,
    };

    // Generate CSS up front (theme + scoped component styles) so prod mode can
    // inline each page's critical CSS into its <head>.
    let combined_css = codegen::codegen_css::generate_combined_css(theme.as_ref(), &document);
    let processed_css = if config.mode == "prod" {
        css_processor::minify_css(&combined_css)?
    } else {
        css_processor::format_css(&combined_css)?
    };

    // Pre-minified CSS parts for critical-CSS assembly (prod only):
    // global (theme vars + base) + one entry per styled component.
    let critical_parts: Option<(String, HashMap<String, String>)> = if config.mode == "prod" {
        let global =
            css_processor::minify_css(&codegen::codegen_css::generate_global_css(theme.as_ref()))?;
        let mut per_component = HashMap::new();
        for (name, component) in &document.components {
            let scoped = codegen::codegen_css::generate_scoped_css(component);
            if !scoped.is_empty() {
                per_component.insert(name.clone(), css_processor::minify_css(&scoped)?);
            }
        }
        Some((global, per_component))
    } else {
        None
    };

    // Assemble the critical CSS for one page: global styles + the styles of
    // every component actually used on that page.
    let critical_css_for = |doc: &ast::WebCoreDocument, page_name: &str| -> Option<String> {
        let (global, per_component) = critical_parts.as_ref()?;
        let used = codegen::codegen_html::collect_page_components(doc, page_name);
        let mut css = global.clone();
        let mut names: Vec<&String> = used
            .iter()
            .filter(|n| per_component.contains_key(*n))
            .collect();
        names.sort(); // deterministic output
        for name in names {
            css.push_str(&per_component[name]);
        }
        Some(css)
    };

    // Generate separate HTML files for each page/component
    let mut all_handlers = Vec::new();
    // Collect errors during page rendering to report all at once (error aggregation).
    let mut page_errors: Vec<error::CompileError> = Vec::new();

    // Collect initial state once for SSG pre-rendering
    let initial_state = ssg::build_initial_state(&document);

    // Helper to convert page name to clean-URL path (e.g. "syntax" → "syntax/index.html")
    fn page_to_filename(name: &str) -> String {
        let route = name
            .to_lowercase()
            .replace("page", "")
            .replace("home", "index");
        if route.is_empty() || route == "index" {
            "index.html".to_string()
        } else {
            format!("{route}/index.html")
        }
    }

    // Generate HTML for each page — collect errors rather than stopping at the first
    let page_count = document.pages.len();
    for (idx, page_name) in document.pages.keys().enumerate() {
        println!("  [{}/{}] {page_name}", idx + 1, page_count);
        let filename = page_to_filename(page_name);
        let page_options = codegen::codegen_html::HtmlPageOptions {
            critical_css: critical_css_for(&document, page_name),
            ..options.clone()
        };
        match codegen::codegen_html::generate_page_content_only(&document, page_name, &page_options)
        {
            Ok(html_result) => {
                all_handlers.extend(html_result.handlers);
                let ssg_html = ssg::apply_ssg_with_locales(
                    &html_result.html,
                    &initial_state,
                    &document.locales,
                    &config.locale,
                );
                let final_html = if config.mode == "prod" {
                    codegen::codegen_html::minify_html(&ssg_html)
                } else {
                    ssg_html
                };
                let output_path = dist_dir.join(&filename);
                if let Some(parent) = output_path.parent() {
                    if let Err(e) = fs::create_dir_all(parent) {
                        page_errors.push(error::CompileError::Io {
                            path: parent.to_path_buf(),
                            source: e,
                        });
                        continue;
                    }
                }
                if let Err(e) = fs::write(&output_path, final_html) {
                    page_errors.push(error::CompileError::Io {
                        path: output_path.clone(),
                        source: e,
                    });
                }
            }
            Err(e) => {
                page_errors.push(e);
            }
        }
    }

    // Generate HTML for each component that looks like a page
    for (component_name, component) in &document.components {
        if component_name.ends_with("Page") {
            let filename = page_to_filename(component_name);
            println!("📄 Generating: {component_name} → {filename}");
            let temp_page = ast::Page {
                name: component_name.clone(),
                head: None,
                content: component.view.clone(),
                span: component.span,
            };
            let temp_doc = build_temp_doc_for_component(&document, temp_page, component_name);

            let page_options = codegen::codegen_html::HtmlPageOptions {
                critical_css: critical_css_for(&temp_doc, component_name),
                ..options.clone()
            };
            match codegen::codegen_html::generate_page_content_only(
                &temp_doc,
                component_name,
                &page_options,
            ) {
                Ok(html_result) => {
                    all_handlers.extend(html_result.handlers);
                    let ssg_html = ssg::apply_ssg_with_locales(
                        &html_result.html,
                        &initial_state,
                        &document.locales,
                        &config.locale,
                    );
                    let final_html = if config.mode == "prod" {
                        codegen::codegen_html::minify_html(&ssg_html)
                    } else {
                        ssg_html
                    };
                    let output_path = dist_dir.join(&filename);
                    if let Some(parent) = output_path.parent() {
                        if let Err(e) = fs::create_dir_all(parent) {
                            page_errors.push(error::CompileError::Io {
                                path: parent.to_path_buf(),
                                source: e,
                            });
                            continue;
                        }
                    }
                    if let Err(e) = fs::write(&output_path, final_html) {
                        page_errors.push(error::CompileError::Io {
                            path: output_path.clone(),
                            source: e,
                        });
                    }
                }
                Err(e) => {
                    page_errors.push(e);
                }
            }
        }
    }

    // SSG collections: `"/post/:slug": PostPage each posts` — generate one
    // static page per item of the bound data import, with the route param
    // pre-rendered into the HTML.
    let collections: Vec<(String, String)> = document
        .app
        .as_ref()
        .map(|app| {
            let mut c: Vec<(String, String)> = app
                .collections
                .iter()
                .map(|(r, i)| (r.clone(), i.clone()))
                .collect();
            c.sort();
            c
        })
        .unwrap_or_default();
    for (route, collection) in collections {
        let Some(component_name) = document
            .app
            .as_ref()
            .and_then(|app| app.routes.get(&route))
            .cloned()
        else {
            continue;
        };
        let Some(items_json) = document.data_imports.get(&collection).cloned() else {
            page_errors.push(error::CompileError::Custom(format!(
                "route '{route}' is bound to collection '{collection}' but no `import {collection} from \"...\"` was found"
            )));
            continue;
        };
        let Some(component) = document.components.get(&component_name).cloned() else {
            page_errors.push(error::CompileError::Custom(format!(
                "route '{route}' references unknown component '{component_name}'"
            )));
            continue;
        };
        let entries = match expand_collection(&route, &items_json) {
            Ok(entries) => entries,
            Err(e) => {
                page_errors.push(error::CompileError::Custom(e));
                continue;
            }
        };

        let temp_page = ast::Page {
            name: component_name.clone(),
            head: None,
            content: component.view.clone(),
            span: component.span,
        };
        let temp_doc = build_temp_doc_for_component(&document, temp_page, &component_name);
        let page_options = codegen::codegen_html::HtmlPageOptions {
            critical_css: critical_css_for(&temp_doc, &component_name),
            ..options.clone()
        };

        println!(
            "🗂  Collection '{collection}': {} page(s) for {route}",
            entries.len()
        );
        for (rel_path, param, value) in entries {
            match codegen::codegen_html::generate_page_content_only(
                &temp_doc,
                &component_name,
                &page_options,
            ) {
                Ok(html_result) => {
                    all_handlers.extend(html_result.handlers);
                    // Pre-render `{$route.<param>}` with this item's value
                    let mut item_state = initial_state.clone();
                    item_state.insert(format!("$route.{param}"), value.clone());
                    let ssg_html = ssg::apply_ssg_with_locales(
                        &html_result.html,
                        &item_state,
                        &document.locales,
                        &config.locale,
                    );
                    let final_html = if config.mode == "prod" {
                        codegen::codegen_html::minify_html(&ssg_html)
                    } else {
                        ssg_html
                    };
                    let output_path = dist_dir.join(&rel_path);
                    if let Some(parent) = output_path.parent() {
                        if let Err(e) = fs::create_dir_all(parent) {
                            page_errors.push(error::CompileError::Io {
                                path: parent.to_path_buf(),
                                source: e,
                            });
                            continue;
                        }
                    }
                    println!("  📄 {rel_path}");
                    if let Err(e) = fs::write(&output_path, final_html) {
                        page_errors.push(error::CompileError::Io {
                            path: output_path.clone(),
                            source: e,
                        });
                    }
                }
                Err(e) => {
                    page_errors.push(e);
                }
            }
        }
    }

    // Report all page-rendering errors together if any occurred
    if !page_errors.is_empty() {
        return Err(error::CompileErrors(page_errors));
    }

    // Write the full stylesheet (generated before the page loop)
    let css_path = assets_dir.join("theme.css");
    fs::write(&css_path, &processed_css).map_err(|e| error::CompileError::Io {
        path: css_path.clone(),
        source: e,
    })?;

    // Generate WebCore runtime JS with compiled handlers and state variables
    let runtime_js = codegen::codegen_js::generate_runtime_js(&all_handlers, &document);
    let final_js = if config.mode == "prod" {
        codegen::codegen_js::minify_js(&runtime_js)
    } else {
        runtime_js
    };

    let js_path = assets_dir.join("webcore.js");
    fs::write(&js_path, &final_js).map_err(|e| error::CompileError::Io {
        path: js_path.clone(),
        source: e,
    })?;

    // Add a content-hash query param to <script src="webcore.js"> in every HTML file
    let js_hash = {
        let mut h: u32 = 2_166_136_261;
        for b in final_js.bytes() {
            h ^= u32::from(b);
            h = h.wrapping_mul(16_777_619);
        }
        format!("{h:08x}")
    };
    patch_html_files(
        dist_dir,
        &format!(r#"src="/assets/webcore.js?v={js_hash}""#),
    );

    // CSS content hash — add version query param to stylesheet and preload hint
    let css_hash = fnv1a_hash(processed_css.as_bytes());
    replace_in_html_files(
        dist_dir,
        r#"href="/assets/theme.css""#,
        &format!(r#"href="/assets/theme.css?v={css_hash}""#),
    );
    // Patch the preload hint with js hash (emitted as `as="script" href="..."`)
    replace_in_html_files(
        dist_dir,
        r#"as="script" href="/assets/webcore.js""#,
        &format!(r#"as="script" href="/assets/webcore.js?v={js_hash}""#),
    );

    // SRI hashes — only in prod mode
    if config.mode == "prod" {
        let js_sri = sri_hash(final_js.as_bytes());
        let css_sri = sri_hash(processed_css.as_bytes());
        // Patch script tag (already has ?v=hash)
        replace_in_html_files(
            dist_dir,
            &format!(r#"src="/assets/webcore.js?v={js_hash}""#),
            &format!(
                r#"src="/assets/webcore.js?v={js_hash}" integrity="{js_sri}" crossorigin="anonymous""#
            ),
        );
        // Patch stylesheet link
        replace_in_html_files(
            dist_dir,
            &format!(r#"href="/assets/theme.css?v={css_hash}""#),
            &format!(
                r#"href="/assets/theme.css?v={css_hash}" integrity="{css_sri}" crossorigin="anonymous""#
            ),
        );
        // Patch preload hint
        replace_in_html_files(
            dist_dir,
            &format!(r#"as="script" href="/assets/webcore.js?v={js_hash}""#),
            &format!(
                r#"as="script" href="/assets/webcore.js?v={js_hash}" integrity="{js_sri}" crossorigin="anonymous""#
            ),
        );
    }

    // Copy public assets, fingerprinting images along the way
    let public_dir = Path::new("public");
    if public_dir.exists() {
        copy_dir_recursive(public_dir, &assets_dir, config.mode == "prod")?;
        // Build fingerprint map for images and apply to HTML/CSS
        let fingerprint_map = fingerprint_images(public_dir, &assets_dir)?;
        if !fingerprint_map.is_empty() {
            rewrite_asset_refs(dist_dir, &fingerprint_map);
        }
    }

    print_dist_tree(dist_dir, config.mode == "prod");
    print_bundle_analysis(&final_js);
    Ok(())
}
