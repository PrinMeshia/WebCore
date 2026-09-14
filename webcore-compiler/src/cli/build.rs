//! Build pipeline: compile `.webc` sources into `dist/`.

use super::assets::{self, copy_dir_recursive, fingerprint_images, rewrite_asset_refs};
use super::config::{read_config, read_wasm_module_name};
use super::loader::{
    build_temp_doc_for_component, expand_collection, load_webc_document, resolve_component_imports,
    resolve_data_imports,
};
use super::output::{print_bundle_analysis, print_dist_tree};
use super::site_outputs::{
    render_feed, render_manifest, render_robots, render_sitemap, xml_escape, SERVICE_WORKER_JS,
};

use crate::codegen;
use crate::core::{ast, css_processor, error, ssg, theme};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

// ── Main build pipeline ──────────────────────────────────────────────────────

/// Compile the current project (reads from `src/`, writes to `dist/`).
///
/// `mode_override` forces the build mode regardless of `webc.toml`'s `mode`
/// (`webc build --prod` / `--dev`); pass `None` to honour the config.
pub(crate) fn build_project(mode_override: Option<&str>) -> Result<(), error::CompileErrors> {
    println!("🔨 Building WebCore project...");

    // Read project config
    let mut config = read_config()?;
    if let Some(m) = mode_override {
        config.mode = m.to_string();
    }
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
    document.view_transitions = config.view_transitions;

    // Resolve build-time data imports (JSON/TOML → document.data_imports)
    resolve_data_imports(&mut document)?;

    // Resolve component imports (.webc → document.components + document.page_imports)
    resolve_component_imports(&mut document)?;

    // Expand `@for item in <data_import>` loops into static markup (#72). Runs
    // after component imports so imported components' data loops expand too.
    crate::core::data_collections::expand(&mut document);

    // Scope each component's reactive state to a unique key namespace so
    // identically-named state in different components no longer collides.
    crate::core::scope::scope_component_state(&mut document);

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
        // Absolute-URL SEO (canonical set per page below) — only when configured.
        site_url: config.url.clone(),
        canonical: None,
        // PWA head tags — only when a [pwa] section is present.
        pwa: config.pwa.as_ref().map(|p| codegen::html::PwaHead {
            theme_color: p.theme_color.clone(),
            short_name: p.short_name.clone(),
            apple_icon: "/assets/apple-touch-icon.png".to_string(),
        }),
        // Per-page hreflang alternates are filled in the page loop when
        // localized static pages are enabled (`[i18n] static`).
        hreflang_alternates: Vec::new(),
    };

    // Generate CSS up front (theme + scoped component styles) so prod mode can
    // inline each page's critical CSS into its <head>.
    let (combined_css, css_line_maps) =
        codegen::css::generate_combined_css_mapped(theme.as_ref(), &document);
    // Dev source map (#54): serve the raw line-tracked CSS (still validated by
    // LightningCSS below) so `theme.css.map` maps each scoped rule back to its
    // `.webc`. Prod stays minified (single line) — no map, matching the JS
    // source-map policy.
    let css_source_map: Option<String> = if config.mode != "prod" {
        build_css_source_map(&css_line_maps, &document)
    } else {
        None
    };
    let mut processed_css = if config.mode == "prod" {
        css_processor::minify_css(&combined_css)?
    } else if css_source_map.is_some() {
        // Validate via LightningCSS (propagates CSS parse errors), then serve
        // the raw CSS the map was built against.
        css_processor::format_css(&combined_css)?;
        combined_css
    } else {
        css_processor::format_css(&combined_css)?
    };
    if css_source_map.is_some() {
        processed_css.push_str("\n/*# sourceMappingURL=theme.css.map */\n");
    }

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

    // Collect initial state once for SSG pre-rendering. Per-page/locale
    // `SsgContext` values are built on the fly in the render loops below.
    let initial_state = ssg::build_initial_state(&document);

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

    // ── SSG i18n (#46) ────────────────────────────────────────────────────────
    // Locales to statically render. The default locale is always first (it owns
    // the shared runtime's expr/handler collection); when `[i18n] static` is on
    // and extra locale catalogs exist, each is rendered under `/{locale}/`.
    let mut render_locales: Vec<String> = vec![config.locale.clone()];
    if config.i18n_static {
        for l in document.locales.keys() {
            if *l != config.locale {
                render_locales.push(l.clone());
            }
        }
    }
    let localized = render_locales.len() > 1;

    // Clean route for a built filename ("index.html" → "/", "x/index.html" → "/x/").
    let route_of = |filename: &str| -> String {
        if filename == "index.html" {
            "/".to_string()
        } else {
            format!("/{}", filename.trim_end_matches("index.html"))
        }
    };
    // Prefix a base filename / route with the locale (default locale stays at root).
    let loc_filename = |locale: &str, base: &str| -> String {
        if locale == config.locale {
            base.to_string()
        } else {
            format!("{locale}/{base}")
        }
    };
    let loc_route = |locale: &str, base_route: &str| -> String {
        if locale == config.locale {
            base_route.to_string()
        } else {
            format!("/{locale}{base_route}")
        }
    };
    // Absolute href when a site URL is configured, otherwise root-relative.
    let href_for = |route: &str| -> String {
        match &config.url {
            Some(base) => format!("{base}{route}"),
            None => route.to_string(),
        }
    };
    // hreflang alternates for a page (one per locale + x-default → default).
    // Empty for the 404 page (noindex) and when localization is off.
    let hreflang_for = |base_route: &str, base_filename: &str| -> Vec<(String, String)> {
        if !localized || base_filename.starts_with("404") {
            return Vec::new();
        }
        let mut v: Vec<(String, String)> = render_locales
            .iter()
            .map(|l| (l.clone(), href_for(&loc_route(l, base_route))))
            .collect();
        v.push(("x-default".to_string(), href_for(base_route)));
        v
    };
    // Self-referencing canonical for a localized filename (404 excluded).
    let canonical_for = |filename: &str| -> Option<String> {
        config
            .url
            .as_ref()
            .filter(|_| !filename.starts_with("404"))
            .map(|base| {
                if filename == "index.html" {
                    format!("{base}/")
                } else {
                    format!("{base}/{}", filename.trim_end_matches("index.html"))
                }
            })
    };

    // Generate HTML for each page — collect errors rather than stopping at the first
    let page_count = document.pages.len();
    for (idx, page_name) in document.pages.keys().enumerate() {
        println!("  [{}/{}] {page_name}", idx + 1, page_count);
        let base_filename = page_to_filename(page_name);
        let base_route = route_of(&base_filename);
        // Build a document view restricted to the components this page imports.
        // Falls back to the full component pool for pages without import declarations.
        let page_doc = page_scoped_doc(page_name);
        let critical = critical_css_for(&page_doc, page_name);
        let hreflang = hreflang_for(&base_route, &base_filename);

        for locale in &render_locales {
            let is_default = locale == &config.locale;
            // The 404 page is noindex — render it once, in the default locale only.
            if !is_default && base_filename.starts_with("404") {
                continue;
            }
            let filename = loc_filename(locale, &base_filename);
            let loc_ssg = ssg::SsgContext {
                state: &initial_state,
                locales: &document.locales,
                locale,
            };
            let page_options = codegen::html::HtmlPageOptions {
                critical_css: critical.clone(),
                canonical: canonical_for(&filename),
                // Non-default locale pages carry the locale as their `lang`.
                lang: if is_default {
                    config.app_lang.clone()
                } else {
                    locale.clone()
                },
                hreflang_alternates: hreflang.clone(),
                ..options.clone()
            };
            match codegen::html::generate_page(
                &page_doc,
                page_name,
                &page_options,
                Some(Path::new(".")),
                Some(&loc_ssg),
            ) {
                Ok(html_result) => {
                    // The compiled closures/handlers are locale-independent, so
                    // collect them once (from the default locale) for the single
                    // shared runtime — avoids duplicate `_e`/handler entries.
                    if is_default {
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
    }

    // Generate HTML for each component that looks like a page
    for (component_name, component) in &document.components {
        if component_name.ends_with("Page") {
            let base_filename = page_to_filename(component_name);
            let base_route = route_of(&base_filename);
            println!("📄 Generating: {component_name} → {base_filename}");
            let temp_page = ast::Page {
                name: component_name.clone(),
                head: None,
                content: component.view.clone(),
                span: component.span,
            };
            let temp_doc = build_temp_doc_for_component(&document, temp_page, component_name);
            let critical = critical_css_for(&temp_doc, component_name);
            let hreflang = hreflang_for(&base_route, &base_filename);

            for locale in &render_locales {
                let is_default = locale == &config.locale;
                if !is_default && base_filename.starts_with("404") {
                    continue;
                }
                let filename = loc_filename(locale, &base_filename);
                let loc_ssg = ssg::SsgContext {
                    state: &initial_state,
                    locales: &document.locales,
                    locale,
                };
                let page_options = codegen::html::HtmlPageOptions {
                    critical_css: critical.clone(),
                    canonical: canonical_for(&filename),
                    lang: if is_default {
                        config.app_lang.clone()
                    } else {
                        locale.clone()
                    },
                    hreflang_alternates: hreflang.clone(),
                    ..options.clone()
                };
                match codegen::html::generate_page(
                    &temp_doc,
                    component_name,
                    &page_options,
                    Some(Path::new(".")),
                    Some(&loc_ssg),
                ) {
                    Ok(html_result) => {
                        if is_default {
                            all_handlers.extend(html_result.handlers);
                            all_exprs.extend(html_result.compiled_exprs);
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
    // Dev CSS source map (#54): written next to theme.css; the stylesheet's
    // trailing `sourceMappingURL` comment points browsers at it.
    if let Some(ref map_json) = css_source_map {
        let map_path = assets_dir.join("theme.css.map");
        fs::write(&map_path, map_json).map_err(|e| error::CompileError::Io {
            path: map_path.clone(),
            source: e,
        })?;
    }

    let prod = config.mode == "prod";

    // Generate the single shared v3 runtime: same emitter as the inline path,
    // but fed the UNION of every page's compiled expressions and handlers so
    // one cached webcore.js serves the whole site. Page-prefixed expression ids
    // keep the `_e` map keys unique across pages.
    let mut runtime_js =
        codegen::js::generate_inline_js(&all_handlers, &all_exprs, &[], &document, prod).js;
    // PWA: register the service worker from the shared runtime (external script,
    // CSP-safe). Appended before hashing so it ships in webcore.<hash>.js.
    if config.pwa.is_some() {
        runtime_js.push_str(
            "\nif('serviceWorker' in navigator){addEventListener('load',function(){navigator.serviceWorker.register('/sw.js').catch(function(){})})}\n",
        );
    }
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
    let mut fingerprint_map: BTreeMap<String, String> = BTreeMap::new();
    if public_dir.exists() {
        copy_dir_recursive(public_dir, &assets_dir, config.mode == "prod")?;
        fingerprint_map = fingerprint_images(public_dir, &assets_dir)?;
        // Responsive images (#74): generate resized width variants for raster
        // sources when `[images] widths` is configured.
        if !config.images.widths.is_empty() {
            let n = assets::generate_image_variants(public_dir, &assets_dir, &config.images.widths);
            if n > 0 {
                println!("🖼  Responsive images: {n} variant(s) generated");
            }
        }
        // Responsive images (#74): generate resized width variants for raster
        // sources when `[images] widths` is configured.
        if !config.images.widths.is_empty() {
            let n = assets::generate_image_variants(public_dir, &assets_dir, &config.images.widths);
            if n > 0 {
                println!("🖼  Responsive images: {n} variant(s) generated");
            }
        }
    }
    // Turn every `webc:img` responsive marker into a `srcset` (or strip it when
    // the feature is off). Runs after all HTML is emitted, and **before** the
    // fingerprint rewrite below: the `srcset` it writes names the source image,
    // which exists in `dist/` only under its hashed name. Rewriting afterwards
    // is what makes those URLs resolve; generated variants carry no hash and
    // are left alone, since no key of the map matches them.
    assets::apply_responsive_srcset(dist_dir, &config.images.widths, &config.images.sizes);
    if !fingerprint_map.is_empty() {
        rewrite_asset_refs(dist_dir, &fingerprint_map);
    }

    // ── SEO root files: robots.txt, sitemap.xml, 404.html ────────────────────
    // Clean-URL path for every page ("index.html" → "/", "x/index.html" → "/x/").
    // With localized static pages, each non-404 route also gets its `/{locale}/`
    // variants so every language version is indexable.
    let mut routes: Vec<String> = Vec::new();
    for name in document.pages.keys() {
        let f = page_to_filename(name);
        let base_route = route_of(&f);
        routes.push(base_route.clone());
        if localized && !f.starts_with("404") {
            for locale in &render_locales {
                if locale != &config.locale {
                    routes.push(loc_route(locale, &base_route));
                }
            }
        }
    }
    routes.sort();
    routes.dedup();

    // robots.txt — always emitted; advertises the sitemap when a site URL is set.
    let robots_path = dist_dir.join("robots.txt");
    fs::write(&robots_path, render_robots(config.url.as_deref())).map_err(|e| {
        error::CompileError::Io {
            path: robots_path.clone(),
            source: e,
        }
    })?;

    // sitemap.xml — only when an absolute site URL is configured (locs must be absolute).
    if let Some(ref base) = config.url {
        let sitemap_path = dist_dir.join("sitemap.xml");
        fs::write(&sitemap_path, render_sitemap(base, &routes)).map_err(|e| {
            error::CompileError::Io {
                path: sitemap_path.clone(),
                source: e,
            }
        })?;
    }

    // feed.xml (RSS 2.0) — from a data collection, when `[feed]` is configured
    // and an absolute site URL is set (feed links must be absolute).
    if let (Some(feed), Some(base)) = (config.feed.as_ref(), config.url.as_ref()) {
        match document.data_imports.get(&feed.collection) {
            Some(json) => {
                if let Some(items) = crate::core::data_collections::item_array(json) {
                    let count = feed.limit.map_or(items.len(), |n| n.min(items.len()));
                    let feed_path = dist_dir.join("feed.xml");
                    fs::write(&feed_path, render_feed(feed, base, &items)).map_err(|e| {
                        error::CompileError::Io {
                            path: feed_path.clone(),
                            source: e,
                        }
                    })?;
                    // Auto-discovery link in every page's <head>.
                    let link = format!(
                        "  <link rel=\"alternate\" type=\"application/rss+xml\" title=\"{}\" href=\"/feed.xml\">\n",
                        xml_escape(&feed.title)
                    );
                    assets::inject_head_snippet(dist_dir, &link);
                    println!("📡 Feed: {count} item(s) → /feed.xml");
                } else {
                    eprintln!(
                        "warning[feed]: collection '{}' is not a JSON array — skipping feed.xml",
                        feed.collection
                    );
                }
            }
            None => eprintln!(
                "warning[feed]: no `import {} from \"...\"` found for [feed] collection — skipping feed.xml",
                feed.collection
            ),
        }
    }

    // 404.html — static hosts (GitHub Pages, Netlify, …) serve dist/404.html on a
    // missing route. Mirror the built `404` page there when the project defines one.
    let not_found = dist_dir.join("404").join("index.html");
    if not_found.exists() {
        let _ = fs::copy(&not_found, dist_dir.join("404.html"));
    }

    // ── PWA: manifest.webmanifest + sw.js at the site root ───────────────────
    if let Some(ref pwa) = config.pwa {
        // Manifest icon paths honour asset fingerprinting (prod).
        let icon = |name: &str| -> String {
            let hashed = fingerprint_map
                .get(name)
                .map(String::as_str)
                .unwrap_or(name);
            format!("/assets/{hashed}")
        };
        let manifest = render_manifest(
            pwa,
            &icon("icon-192.png"),
            &icon("icon-512.png"),
            &icon("icon-maskable.png"),
        );
        let manifest_path = dist_dir.join("manifest.webmanifest");
        fs::write(&manifest_path, manifest).map_err(|e| error::CompileError::Io {
            path: manifest_path.clone(),
            source: e,
        })?;

        let sw_path = dist_dir.join("sw.js");
        fs::write(&sw_path, SERVICE_WORKER_JS).map_err(|e| error::CompileError::Io {
            path: sw_path.clone(),
            source: e,
        })?;
    }

    // Resolve `{$build.*}` placeholders now that output sizes are known.
    substitute_build_vars(dist_dir, &document, &processed_css, &final_js);

    print_dist_tree(dist_dir, config.mode == "prod");
    print_bundle_analysis(&final_js);
    Ok(())
}

/// Length of `data` after gzip compression at the default level (~what a server
/// serves) — models the real over-the-wire size. Falls back to the raw length
/// if compression fails.
fn gzip_len(data: &[u8]) -> usize {
    use flate2::{write::GzEncoder, Compression};
    use std::io::Write;
    let mut enc = GzEncoder::new(Vec::new(), Compression::default());
    if enc.write_all(data).is_err() {
        return data.len();
    }
    enc.finish().map(|v| v.len()).unwrap_or(data.len())
}

/// Replace `{$build.<field>}` placeholders in every `dist/` HTML file with the
/// real build statistics — page/component/locale counts and output sizes, which
/// are only final once the JS/CSS have been generated. Unknown fields are left
/// as-is (so a typo is visible rather than silently blanked).
fn substitute_build_vars(dist_dir: &Path, document: &ast::WebCoreDocument, css: &str, js: &str) {
    let kb = |bytes: usize| format!("{:.1}", bytes as f64 / 1024.0);
    // Gzipped sizes model the real over-the-wire weight (the marketing figure),
    // not the minified byte count.
    let js_gz = gzip_len(js.as_bytes());
    let css_gz = gzip_len(css.as_bytes());
    let total_gz = gzip_len(&[js.as_bytes(), css.as_bytes()].concat());
    let vars: [(&str, String); 14] = [
        ("pages", document.pages.len().to_string()),
        ("components", document.components.len().to_string()),
        ("locales", document.locales.len().to_string()),
        ("jsBytes", js.len().to_string()),
        ("jsKb", kb(js.len())),
        ("cssBytes", css.len().to_string()),
        ("cssKb", kb(css.len())),
        // Total shipped weight (runtime JS + stylesheet).
        ("totalKb", kb(js.len() + css.len())),
        // Gzipped variants — the actual transferred size.
        ("jsGzipBytes", js_gz.to_string()),
        ("jsGzipKb", kb(js_gz)),
        ("cssGzipBytes", css_gz.to_string()),
        ("cssGzipKb", kb(css_gz)),
        ("totalGzipBytes", total_gz.to_string()),
        ("totalGzipKb", kb(total_gz)),
    ];
    let _ = crate::cli::loader::walk_files(dist_dir, |p| {
        if p.extension().and_then(|e| e.to_str()) != Some("html") {
            return Ok(());
        }
        let Ok(mut html) = fs::read_to_string(p) else {
            return Ok(());
        };
        if !html.contains('\u{2063}') {
            return Ok(());
        }
        for (field, value) in &vars {
            html = html.replace(&codegen::html::build_var_placeholder(field), value);
        }
        let _ = fs::write(p, html);
        Ok(())
    });
}

/// Build the dev CSS source map (#54): each generated-stylesheet line that
/// carries a component rule is mapped back to its `.webc` file + line. Sources
/// are labelled with a project-relative path and their content is embedded
/// (`sourcesContent`) so DevTools needs no extra fetch. Returns `None` when
/// there is nothing to map.
fn build_css_source_map(
    maps: &[codegen::css::CssLineMap],
    document: &ast::WebCoreDocument,
) -> Option<String> {
    if maps.is_empty() {
        return None;
    }
    let cwd = std::env::current_dir().ok();
    let mut sm = codegen::css_sourcemap::CssSourceMap::new("theme.css");
    for m in maps {
        let Some(path) = document.source_files.get(&m.component) else {
            continue;
        };
        let label = cwd
            .as_ref()
            .and_then(|c| path.strip_prefix(c).ok())
            .unwrap_or(path.as_path())
            .display()
            .to_string();
        let content = fs::read_to_string(path).unwrap_or_default();
        let idx = sm.add_source(&label, &content);
        // `src_line` is 1-indexed; source maps are 0-indexed.
        sm.add(m.out_line, idx, m.src_line.saturating_sub(1));
    }
    if sm.is_empty() {
        None
    } else {
        Some(sm.build())
    }
}

/// Watch mode: rebuild whenever source files change (no HTTP server).
pub(crate) fn watch_project() -> Result<(), String> {
    use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    println!("👁  Watch mode — rebuilding on file changes (Ctrl-C to stop)");

    // Initial build
    if let Err(e) = build_project(None) {
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
            if let Err(e) = build_project(None) {
                eprintln!("Build error: {e}");
            } else {
                println!("✅ Rebuild complete");
            }
        }
    }
}

#[cfg(test)]
mod seo_tests {
    use super::{render_manifest, render_robots, render_sitemap};
    use crate::cli::config::Pwa;

    #[test]
    fn manifest_has_required_fields_and_icons() {
        let pwa = Pwa {
            name: "My \"App\"".to_string(),
            short_name: "App".to_string(),
            theme_color: "#7C3AED".to_string(),
            background_color: "#05030F".to_string(),
            display: "standalone".to_string(),
        };
        let m = render_manifest(
            &pwa,
            "/assets/icon-192.png",
            "/assets/icon-512.png",
            "/assets/icon-maskable.png",
        );
        assert!(m.contains(r#""short_name": "App""#));
        assert!(m.contains(r#""start_url": "/""#));
        assert!(m.contains(r#""display": "standalone""#));
        assert!(m.contains(r##""theme_color": "#7C3AED""##));
        assert!(m.contains(r#""sizes": "192x192""#));
        assert!(m.contains(r#""purpose": "maskable""#));
        // Quotes in the name are JSON-escaped.
        assert!(
            m.contains(r#""name": "My \"App\"""#),
            "name not escaped:\n{m}"
        );
    }

    #[test]
    fn robots_without_url_has_no_sitemap_line() {
        let out = render_robots(None);
        assert!(out.contains("User-agent: *"));
        assert!(out.contains("Allow: /"));
        assert!(!out.contains("Sitemap:"));
    }

    #[test]
    fn robots_with_url_links_sitemap() {
        let out = render_robots(Some("https://example.com"));
        assert!(out.contains("Sitemap: https://example.com/sitemap.xml"));
    }

    #[test]
    fn sitemap_lists_routes_and_skips_404() {
        let routes = vec!["/".to_string(), "/skills/".to_string(), "/404/".to_string()];
        let out = render_sitemap("https://example.com", &routes);
        assert!(out.contains("<loc>https://example.com/</loc>"));
        assert!(out.contains("<loc>https://example.com/skills/</loc>"));
        assert!(!out.contains("/404"), "404 must not be advertised:\n{out}");
        assert!(out.trim_start().starts_with("<?xml"));
        assert!(out.contains("</urlset>"));
    }
}
