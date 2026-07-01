//! Build pipeline: compile `.webc` sources into `dist/`.

use super::assets::{self, copy_dir_recursive, fingerprint_images, rewrite_asset_refs};
use super::config::{read_config, read_wasm_module_name};
use super::loader::{
    build_temp_doc_for_component, expand_collection, load_webc_document, resolve_component_imports,
    resolve_data_imports,
};
use super::output::{print_bundle_analysis, print_dist_tree};

use crate::codegen;
use crate::core::{ast, css_processor, error, ssg, theme};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

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

    // Resolve component imports (.webc → document.components + document.page_imports)
    resolve_component_imports(&mut document)?;

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

    // In prod mode, emit a strict CSP meta tag (script-src 'self' — no inline JS).
    let csp_meta = if config.mode == "prod" && config.csp {
        Some("default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self' data:".to_string())
    } else {
        None
    };
    let is_prod = config.mode == "prod";
    let options = codegen::html::HtmlPageOptions {
        lang: config.app_lang.clone(),
        title: config.app_title.clone(),
        extra_css_files,
        critical_css: None,
        csp_meta,
        prod: is_prod,
        // Enable source maps in dev mode (non-prod) for browser devtools debugging
        source_maps: !is_prod,
        // Pages reference a single shared /assets/webcore.js (cached once across
        // the whole site) instead of inlining the runtime in every page.
        inline_runtime: false,
    };

    // Generate CSS up front (theme + scoped component styles) so prod mode can
    // inline each page's critical CSS into its <head>.
    let combined_css = codegen::css::generate_combined_css(theme.as_ref(), &document);
    let processed_css = if config.mode == "prod" {
        css_processor::minify_css(&combined_css)?
    } else {
        css_processor::format_css(&combined_css)?
    };

    // Pre-minified CSS parts for critical-CSS assembly (prod only):
    // global (theme vars + base) + one entry per styled component.
    let critical_parts: Option<(String, BTreeMap<String, String>)> = if config.mode == "prod" {
        let global = css_processor::minify_css(&codegen::css::generate_global_css(theme.as_ref()))?;
        let mut per_component = BTreeMap::new();
        for (name, component) in &document.components {
            let scoped = codegen::css::generate_scoped_css(component);
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
        let used = codegen::html::collect_page_components(doc, page_name);
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
    // Union of every page's compiled expressions (id → closure). The shared
    // runtime file's `_e` map is built from this so one cached webcore.js
    // serves all pages. Page-prefixed ids keep them unique across pages.
    let mut all_exprs: Vec<(String, String)> = Vec::new();
    // Collect errors during page rendering to report all at once (error aggregation).
    let mut page_errors: Vec<error::CompileError> = Vec::new();

    // Collect initial state once for SSG pre-rendering
    let initial_state = ssg::build_initial_state(&document);
    let ssg_ctx = ssg::SsgContext {
        state: &initial_state,
        locales: &document.locales,
        locale: &config.locale,
    };

    // Return a document view scoped to the components available for `page_name`.
    // If the page file declared explicit imports, only those components are kept.
    // If no imports were declared, all components are available (v2 compat).
    let page_scoped_doc = |page_name: &str| -> ast::WebCoreDocument {
        if let Some(imports) = document.page_imports.get(page_name) {
            let mut scoped = document.clone();
            scoped.components.retain(|name, _| imports.contains(name));
            scoped
        } else {
            document.clone()
        }
    };

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
        // Build a document view restricted to the components this page imports.
        // Falls back to the full component pool for pages without import declarations.
        let page_doc = page_scoped_doc(page_name);
        let page_options = codegen::html::HtmlPageOptions {
            critical_css: critical_css_for(&page_doc, page_name),
            ..options.clone()
        };
        match codegen::html::generate_page(
            &page_doc,
            page_name,
            &page_options,
            Some(Path::new(".")),
            Some(&ssg_ctx),
        ) {
            Ok(html_result) => {
                all_handlers.extend(html_result.handlers);
                all_exprs.extend(html_result.compiled_exprs);
                // Write source map alongside the HTML when present (dev mode)
                if let Some(ref map_json) = html_result.source_map_json {
                    let map_path = dist_dir
                        .join(filename.trim_end_matches("index.html"))
                        .join(format!("{page_name}.js.map"));
                    if let Some(parent) = map_path.parent() {
                        let _ = fs::create_dir_all(parent);
                    }
                    let _ = fs::write(&map_path, map_json);
                }
                let final_html = if config.mode == "prod" {
                    codegen::html::minify_html(&html_result.html)
                } else {
                    html_result.html
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

            let page_options = codegen::html::HtmlPageOptions {
                critical_css: critical_css_for(&temp_doc, component_name),
                ..options.clone()
            };
            match codegen::html::generate_page(
                &temp_doc,
                component_name,
                &page_options,
                Some(Path::new(".")),
                Some(&ssg_ctx),
            ) {
                Ok(html_result) => {
                    all_handlers.extend(html_result.handlers);
                    all_exprs.extend(html_result.compiled_exprs);
                    let final_html = if config.mode == "prod" {
                        codegen::html::minify_html(&html_result.html)
                    } else {
                        html_result.html
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
        let page_options = codegen::html::HtmlPageOptions {
            critical_css: critical_css_for(&temp_doc, &component_name),
            ..options.clone()
        };

        println!(
            "🗂  Collection '{collection}': {} page(s) for {route}",
            entries.len()
        );
        for (rel_path, param, value) in entries {
            // Pre-render `{$route.<param>}` with this item's value
            let mut item_state = initial_state.clone();
            item_state.insert(format!("$route.{param}"), value.clone());
            let item_ssg_ctx = ssg::SsgContext {
                state: &item_state,
                locales: &document.locales,
                locale: &config.locale,
            };
            match codegen::html::generate_page(
                &temp_doc,
                &component_name,
                &page_options,
                Some(Path::new(".")),
                Some(&item_ssg_ctx),
            ) {
                Ok(html_result) => {
                    all_handlers.extend(html_result.handlers);
                    all_exprs.extend(html_result.compiled_exprs);
                    let final_html = if config.mode == "prod" {
                        codegen::html::minify_html(&html_result.html)
                    } else {
                        html_result.html
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

    let prod = config.mode == "prod";

    // Generate the single shared v3 runtime: same emitter as the inline path,
    // but fed the UNION of every page's compiled expressions and handlers so
    // one cached webcore.js serves the whole site. Page-prefixed expression ids
    // keep the `_e` map keys unique across pages.
    let runtime_js =
        codegen::js::generate_inline_js(&all_handlers, &all_exprs, &[], &document, prod).js;
    let final_js = if prod {
        codegen::js::minify_js(&runtime_js)
    } else {
        runtime_js
    };

    // Content-hash filename: webcore.<hash8>.js (cache-busting without query params)
    let (js_hash, js_sri) = assets::hash_asset(final_js.as_bytes(), prod);
    let js_filename = format!("webcore.{}.js", &js_hash[..8]);
    let js_path = assets_dir.join(&js_filename);
    fs::write(&js_path, &final_js).map_err(|e| error::CompileError::Io {
        path: js_path.clone(),
        source: e,
    })?;

    let (css_hash, css_sri) = assets::hash_asset(processed_css.as_bytes(), prod);

    assets::patch_asset_hashes(
        dist_dir,
        &js_filename,
        &css_hash,
        js_sri.as_deref(),
        css_sri.as_deref(),
    );

    // Copy public assets, fingerprinting images along the way
    let public_dir = Path::new("public");
    if public_dir.exists() {
        copy_dir_recursive(public_dir, &assets_dir, config.mode == "prod")?;
        let fingerprint_map = fingerprint_images(public_dir, &assets_dir)?;
        if !fingerprint_map.is_empty() {
            rewrite_asset_refs(dist_dir, &fingerprint_map);
        }
    }

    print_dist_tree(dist_dir, config.mode == "prod");
    print_bundle_analysis(&final_js);
    Ok(())
}

/// Watch mode: rebuild whenever source files change (no HTTP server).
pub(crate) fn watch_project() -> Result<(), String> {
    use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    println!("👁  Watch mode — rebuilding on file changes (Ctrl-C to stop)");

    // Initial build
    if let Err(e) = build_project() {
        eprintln!("Build error: {e}");
    }

    let dirty = Arc::new(Mutex::new(false));
    let dirty_watcher = dirty.clone();

    let mut watcher: RecommendedWatcher =
        notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
            if let Ok(event) = res {
                if matches!(
                    event.kind,
                    EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
                ) {
                    if let Ok(mut d) = dirty_watcher.lock() {
                        *d = true;
                    }
                }
            }
        })
        .map_err(|e| format!("Watcher error: {e}"))?;

    for dir in &["src", "theme.toml", "webc.toml", "locales"] {
        let p = std::path::Path::new(dir);
        if p.exists() {
            watcher
                .watch(p, RecursiveMode::Recursive)
                .map_err(|e| format!("Watch error for {dir}: {e}"))?;
        }
    }

    let mut last_build = Instant::now();
    loop {
        std::thread::sleep(Duration::from_millis(200));
        let is_dirty = dirty
            .lock()
            .map(|mut d| {
                if *d {
                    *d = false;
                    true
                } else {
                    false
                }
            })
            .unwrap_or(false);

        if is_dirty && last_build.elapsed() > Duration::from_millis(300) {
            last_build = Instant::now();
            println!("\n🔄 File changed — rebuilding...");
            if let Err(e) = build_project() {
                eprintln!("Build error: {e}");
            } else {
                println!("✅ Rebuild complete");
            }
        }
    }
}
