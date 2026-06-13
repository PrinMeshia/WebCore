//! Build pipeline: compile `.webc` sources into `dist/`.

use crate::core::{ast, css_processor, error, ssg, theme};
use super::assets;
use crate::{codegen, parser};
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
pub(crate) fn load_webc_dir<F>(dir: &Path, label: &str, ext: &str, mut loader: F) -> Result<(), String>
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
    }

    // Load layouts
    load_webc_dir(Path::new("src/layouts"), "layouts", "webc", |path, source| {
        let parsed = parser::parse_webc(source)
            .map_err(|e| format!("Parse error in {}:\n{}", path.display(), e))?;
        for (name, layout) in parsed.layouts {
            document.layouts.insert(name, layout);
        }
        Ok(())
    })?;

    // Load components
    load_webc_dir(Path::new("src/components"), "components", "webc", |path, source| {
        let parsed = parser::parse_webc(source)
            .map_err(|e| format!("Parse error in {}:\n{}", path.display(), e))?;
        for (name, component) in parsed.components {
            document.components.insert(name, component);
        }
        Ok(())
    })?;

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
                let rel = if prefix.is_empty() { name.clone() } else { format!("{prefix}/{name}") };
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
            if s.ends_with(".html") { 0 }
            else if s.ends_with(".js") { 1 }
            else if s.ends_with(".css") { 2 }
            else { 3 }
        }
        rank(a).cmp(&rank(b)).then(a.cmp(b))
    });

    let total_bytes: u64 = files.iter().map(|(_, s)| s).sum();
    let max_name = files.iter().map(|(n, _)| n.len()).max().unwrap_or(10);

    println!("\ndist/");
    let count = files.len();
    for (i, (name, size)) in files.iter().enumerate() {
        let branch = if i + 1 == count { "└──" } else { "├──" };
        println!("  {}  {:<width$}  {}", branch, name, fmt_size(*size), width = max_name);
    }
    let mode_label = if minified { "minified" } else { "dev" };
    println!("\n  {} file{}  {}  ({})\n",
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
        ("bindIf",           "bindIf (conditionals)",    320),
        ("bindFor",          "bindFor (loops)",           512),
        ("bindAttrs",        "bindAttrs (dyn attrs)",     180),
        ("bindValidation",   "bindValidation",            640),
        ("const LOCALES=",   "i18n / t()",               210),
        ("const ROUTES=",    "router (param routes)",     380),
        ("const toFile=",    "router (simple nav)",        90),
        ("const WASM=",      "WASM loader",               120),
        ("DESTROY_HOOKS",    "on:destroy hooks",           80),
        ("const COMPUTED=",  "computed vars",              95),
    ];

    // Rough estimate: state-class boilerplate ~350 bytes per reactive component.
    let core_bytes = js.matches("class _S").count() * 350 + 420;

    let mut total: u64 = core_bytes as u64;
    println!("\n  Bundle analysis");
    println!("  ──────────────────────────────────────────────");
    println!("  {:<35} {:<10} Status", "Feature", "Size");
    println!("  {:<35} {:<10} ──────", "───────", "────");
    println!("  {:<35} {:<10} ✓ included", "runtime core", fmt_bytes(core_bytes as u64));

    for &(marker, label, est) in FEATURES {
        let included = js.contains(marker);
        if included {
            total += est as u64;
        }
        let status = if included { "✓ included" } else { "- tree-shaken" };
        let size_str = if included { fmt_bytes(est as u64) } else { "   —".to_string() };
        println!("  {:<35} {:<10} {}", label, size_str, status);
    }

    println!("  {}", "─".repeat(54));
    println!("  {:<35} {:<10}", "estimated total (unminified)", fmt_bytes(total));
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
    let options = codegen::html::HtmlPageOptions {
        lang: config.app_lang.clone(),
        title: config.app_title.clone(),
        extra_css_files,
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
        match codegen::html::generate_page_content_only(&document, page_name, &options) {
            Ok(html_result) => {
                all_handlers.extend(html_result.handlers);
                let ssg_html = ssg::apply_ssg_with_locales(
                    &html_result.html,
                    &initial_state,
                    &document.locales,
                    &config.locale,
                );
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
                if let Err(e) = fs::write(&output_path, ssg_html) {
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
            let temp_doc =
                build_temp_doc_for_component(&document, temp_page, component_name);

            match codegen::html::generate_page_content_only(
                &temp_doc,
                component_name,
                &options,
            ) {
                Ok(html_result) => {
                    all_handlers.extend(html_result.handlers);
                    let ssg_html = ssg::apply_ssg_with_locales(
                        &html_result.html,
                        &initial_state,
                        &document.locales,
                        &config.locale,
                    );
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
                    if let Err(e) = fs::write(&output_path, ssg_html) {
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

    // Generate CSS (theme + scoped component styles)
    let combined_css = codegen::css::generate_combined_css(theme.as_ref(), &document);
    let processed_css = if config.mode == "prod" {
        css_processor::minify_css(&combined_css)?
    } else {
        css_processor::format_css(&combined_css)?
    };

    let css_path = assets_dir.join("theme.css");
    fs::write(&css_path, &processed_css)
        .map_err(|e| error::CompileError::Io { path: css_path.clone(), source: e })?;

    // Generate WebCore runtime JS with compiled handlers and state variables
    let runtime_js = codegen::js::generate_runtime_js(&all_handlers, &document);
    let final_js = if config.mode == "prod" {
        codegen::js::minify_js(&runtime_js)
    } else {
        runtime_js
    };

    let js_path = assets_dir.join("webcore.js");
    fs::write(&js_path, &final_js).map_err(|e| error::CompileError::Io { path: js_path.clone(), source: e })?;

    // Add a content-hash query param to <script src="webcore.js"> in every HTML file
    let js_hash = assets::fnv1a_hash(final_js.as_bytes());
    assets::patch_html_files(dist_dir, &format!(r#"src="/assets/webcore.js?v={js_hash}""#));

    // Copy public assets, fingerprinting images along the way
    let public_dir = Path::new("public");
    if public_dir.exists() {
        assets::copy_dir_recursive(public_dir, &assets_dir, config.mode == "prod")?;
        // Build fingerprint map for images and apply to HTML/CSS
        let fingerprint_map = assets::fingerprint_images(public_dir, &assets_dir)?;
        if !fingerprint_map.is_empty() {
            assets::rewrite_asset_refs(dist_dir, &fingerprint_map);
        }
    }

    print_dist_tree(dist_dir, config.mode == "prod");
    print_bundle_analysis(&final_js);
    Ok(())
}
