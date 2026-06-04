mod ast;
mod parser;
pub mod codegen {
    pub mod codegen_css;
    pub mod codegen_html;
    pub mod codegen_js;
}
mod css_processor;
mod errors;
mod ssg;
#[cfg(test)]
mod tests;
mod theme;

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use qrcode::render::unicode;
use qrcode::QrCode;
use serde::Deserialize;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::net::{TcpListener, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tiny_http::{Request, Response, Server};
use tungstenite::{accept, Message};

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_help();
        return;
    }

    match args[1].as_str() {
        "new" => {
            let name = match args.get(2) {
                Some(n) => n.clone(),
                None => {
                    eprintln!("Usage: webc new <nom-du-projet>");
                    std::process::exit(1);
                }
            };
            if let Err(e) = new_project(&name) {
                eprintln!("Erreur : {}", e);
                std::process::exit(1);
            }
        }
        "build" => {
            if let Err(e) = build_project() {
                eprintln!("Build failed: {}", e);
                std::process::exit(1);
            }
        }
        "dev" => {
            let mut port: u16 = 3000;
            let mut host: Option<String> = None;
            let mut auto_open = false;
            // Args: dev [port] [--host 0.0.0.0] [--open]
            let mut i = 2;
            // Back-compat: if a bare number is provided, treat as port
            if let Some(arg) = args.get(i) {
                if let Ok(p) = arg.parse::<u16>() {
                    port = p;
                    i += 1;
                }
            }
            while i < args.len() {
                match args[i].as_str() {
                    "--host" => {
                        if let Some(h) = args.get(i + 1) {
                            host = Some(h.clone());
                        }
                        i += 2;
                    }
                    "--open" => {
                        auto_open = true;
                        i += 1;
                    }
                    _ => {
                        i += 1;
                    }
                }
            }
            if let Err(e) = dev_server_with_options(port, host, auto_open) {
                eprintln!("Dev server error: {}", e);
                std::process::exit(1);
            }
        }
        _ => {
            eprintln!("Commande inconnue : {}", args[1]);
            print_help();
            std::process::exit(1);
        }
    }
}

fn print_help() {
    println!("WebCore v{}", env!("CARGO_PKG_VERSION"));
    println!();
    println!("USAGE:");
    println!("  webc <commande> [options]");
    println!();
    println!("COMMANDES:");
    println!("  new <nom>    Créer un nouveau projet WebCore");
    println!("  build        Compiler le projet (dist/)");
    println!("  dev [port]   Démarrer le serveur de développement (défaut : 3000)");
    println!();
    println!("OPTIONS (dev) :");
    println!("  --host <ip>  Écouter sur une IP spécifique (ex: 0.0.0.0)");
    println!("  --open       Ouvrir le navigateur automatiquement");
}

fn new_project(name: &str) -> Result<(), String> {
    // Validate name: alphanumeric + hyphens/underscores, non-empty
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Err(format!(
            "Nom invalide '{name}'. Utilisez uniquement des lettres, chiffres, - ou _."
        ));
    }

    let root = Path::new(name);
    if root.exists() {
        return Err(format!("Le répertoire '{name}' existe déjà."));
    }

    println!("🚀 Création du projet '{name}'...");

    // Create directory structure
    for dir in &[
        format!("{name}/src/layouts"),
        format!("{name}/src/pages"),
        format!("{name}/src/components"),
        format!("{name}/public"),
    ] {
        fs::create_dir_all(dir).map_err(|e| format!("Impossible de créer {dir} : {e}"))?;
    }

    // webc.toml
    fs::write(
        format!("{name}/webc.toml"),
        format!(
            r#"[app]
title = "{name}"
lang = "fr"
mode = "dev"
"#
        ),
    )
    .map_err(|e| format!("webc.toml : {e}"))?;

    // theme.toml
    fs::write(
        format!("{name}/theme.toml"),
        r##"[colors]
primary    = "#4F46E5"
background = "#FFFFFF"
text       = "#1F2937"
muted      = "#6B7280"

[fonts]
sans = "system-ui, -apple-system, sans-serif"

[spacing]
sm = "0.5rem"
md = "1rem"
lg = "2rem"
"##,
    )
    .map_err(|e| format!("theme.toml : {e}"))?;

    // src/app.webc
    fs::write(
        format!("{name}/src/app.webc"),
        format!(
            r#"app {name_pascal} {{
    theme: "default"
    layout: MainLayout
    routes {{
        "/": HomePage
    }}
}}
"#,
            name_pascal = pascal_case(name)
        ),
    )
    .map_err(|e| format!("src/app.webc : {e}"))?;

    // src/layouts/MainLayout.webc
    fs::write(
        format!("{name}/src/layouts/MainLayout.webc"),
        r#"layout MainLayout {
    header {
        nav {
            link to="/" { "Accueil" }
        }
    }
    main { slot content }
    footer {
        p "Propulsé par WebCore"
    }
}
"#,
    )
    .map_err(|e| format!("src/layouts/MainLayout.webc : {e}"))?;

    // src/pages/home.webc
    fs::write(
        format!("{name}/src/pages/home.webc"),
        format!(
            r#"page "home" {{
    h1 "Bienvenue dans {name} !"
    p "Votre projet WebCore est prêt. Modifiez ce fichier pour commencer."
    Counter {{}}
}}
"#
        ),
    )
    .map_err(|e| format!("src/pages/home.webc : {e}"))?;

    // src/components/Counter.webc
    fs::write(
        format!("{name}/src/components/Counter.webc"),
        r#"component Counter {
    state {
        count: Number = 0
    }

    view {
        div {
            p "Compteur : {count}"
            button on:click={count += 1} { "+" }
            button on:click={count = max(0, count - 1)} { "−" }
        }
    }

    style {
        div    { display: flex; align-items: center; gap: 1rem; margin-top: 1rem; }
        button { padding: 0.25rem 0.75rem; cursor: pointer; border-radius: 4px; }
    }
}
"#,
    )
    .map_err(|e| format!("src/components/Counter.webc : {e}"))?;

    // wasm/ — Rust → WebAssembly module scaffold
    fs::create_dir_all(format!("{name}/wasm/src")).map_err(|e| format!("wasm/src : {e}"))?;
    fs::write(
        format!("{name}/wasm/Cargo.toml"),
        format!(
            r#"[package]
name = "{name}-wasm"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
wasm-bindgen = "0.2"
"#
        ),
    )
    .map_err(|e| format!("wasm/Cargo.toml : {e}"))?;
    fs::write(
        format!("{name}/wasm/src/lib.rs"),
        r#"use wasm_bindgen::prelude::*;

/// Example function exported to JavaScript.
/// Call it in your .webc views with: {wasm.greet("Alice")}
#[wasm_bindgen]
pub fn greet(name: &str) -> String {
    format!("Hello, {}!", name)
}
"#,
    )
    .map_err(|e| format!("wasm/src/lib.rs : {e}"))?;

    // public/.gitkeep
    fs::write(format!("{name}/public/.gitkeep"), "")
        .map_err(|e| format!("public/.gitkeep : {e}"))?;

    println!("✅ Projet '{name}' créé avec succès !");
    println!();
    println!("  cd {name}");
    println!("  webc dev");
    println!();
    println!("Pour compiler le module WASM (optionnel) :");
    println!("  wasm-pack build wasm/ --target web --out-dir ../dist/wasm");

    Ok(())
}

/// Convert kebab-case or snake_case to PascalCase (e.g. "my-app" → "MyApp").
fn pascal_case(s: &str) -> String {
    s.split(['-', '_'])
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut chars = p.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect()
}

fn build_project() -> Result<(), String> {
    println!("🔨 Building WebCore project...");

    // Read project config
    let config = read_config()?;
    println!("📁 Project: {}", config.app_title);

    // Create dist directory
    let dist_dir = Path::new("dist");
    if dist_dir.exists() {
        fs::remove_dir_all(dist_dir).map_err(|e| format!("Failed to clean dist: {}", e))?;
    }
    fs::create_dir_all(dist_dir).map_err(|e| format!("Failed to create dist: {}", e))?;

    // Load theme
    let theme = if Path::new("theme.toml").exists() {
        println!("🎨 Loading theme...");
        Some(theme::load_theme("theme.toml")?)
    } else {
        println!("⚠️  No theme.toml found, using default theme");
        None
    };

    // Load and parse all WebCore files
    let mut document = ast::WebCoreDocument {
        app: None,
        store: Vec::new(),
        locales: HashMap::new(),
        default_locale: config.locale.clone(),
        wasm_module: None,
        layouts: HashMap::new(),
        pages: HashMap::new(),
        components: HashMap::new(),
    };

    // Load app.webc first
    let app_path = Path::new("src/app.webc");
    if app_path.exists() {
        let content =
            fs::read_to_string(app_path).map_err(|e| format!("Failed to read app.webc: {}", e))?;
        let parsed =
            parser::parse_webc(&content).map_err(|e| format!("Parse error in app.webc:\n{e}"))?;
        document.app = parsed.app;
        document.store.extend(parsed.store);
    }

    // Load layouts
    let layouts_dir = Path::new("src/layouts");
    if layouts_dir.exists() {
        for entry in
            fs::read_dir(layouts_dir).map_err(|e| format!("Failed to read layouts: {}", e))?
        {
            let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("webc") {
                let content = fs::read_to_string(&path)
                    .map_err(|e| format!("Failed to read {:?}: {}", path, e))?;
                let parsed = parser::parse_webc(&content)
                    .map_err(|e| format!("Parse error in {}:\n{}", path.display(), e))?;

                // Merge layouts
                for (name, layout) in parsed.layouts {
                    document.layouts.insert(name, layout);
                }
            }
        }
    }

    // Load components
    let components_dir = Path::new("src/components");
    if components_dir.exists() {
        for entry in
            fs::read_dir(components_dir).map_err(|e| format!("Failed to read components: {}", e))?
        {
            let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("webc") {
                let content = fs::read_to_string(&path)
                    .map_err(|e| format!("Failed to read {:?}: {}", path, e))?;
                let parsed = parser::parse_webc(&content)
                    .map_err(|e| format!("Parse error in {}:\n{}", path.display(), e))?;

                // Merge components
                for (name, component) in parsed.components {
                    document.components.insert(name, component);
                }
            }
        }
    }

    // Load pages
    let pages_dir = Path::new("src/pages");
    if pages_dir.exists() {
        for entry in fs::read_dir(pages_dir).map_err(|e| format!("Failed to read pages: {}", e))? {
            let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("webc") {
                let content = fs::read_to_string(&path)
                    .map_err(|e| format!("Failed to read {:?}: {}", path, e))?;
                let parsed = parser::parse_webc(&content)
                    .map_err(|e| format!("Parse error in {}:\n{}", path.display(), e))?;

                // Merge pages and components
                for (name, page) in parsed.pages {
                    document.pages.insert(name, page);
                }
                for (name, component) in parsed.components {
                    document.components.insert(name, component);
                }
            }
        }
    }

    // Load locale files from locales/ directory (flat TOML: key = "value")
    let locales_dir = Path::new("locales");
    if locales_dir.exists() {
        for entry in
            fs::read_dir(locales_dir).map_err(|e| format!("Failed to read locales/: {}", e))?
        {
            let entry = entry.map_err(|e| format!("Failed to read locale entry: {}", e))?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("toml") {
                if let Some(code) = path.file_stem().and_then(|s| s.to_str()) {
                    let content = fs::read_to_string(&path)
                        .map_err(|e| format!("Failed to read {:?}: {}", path, e))?;
                    let entries: HashMap<String, String> = toml::from_str(&content)
                        .map_err(|e| format!("Failed to parse locale {:?}: {}", path, e))?;
                    document.locales.insert(code.to_string(), entries);
                    println!("🌍 Loaded locale: {}", code);
                }
            }
        }
    }

    // Detect and compile WASM module (wasm/Cargo.toml → dist/wasm/)
    let wasm_cargo = Path::new("wasm/Cargo.toml");
    if wasm_cargo.exists() {
        match read_wasm_module_name(wasm_cargo) {
            Ok(module_name) => {
                println!("🦀 WASM module detected: {}", module_name);
                document.wasm_module = Some(module_name.clone());

                let wasm_out = dist_dir.join("wasm");
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
                        println!("✅ WASM compiled: dist/wasm/{}.js", module_name);
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
                println!("⚠️  Could not read wasm/Cargo.toml: {}", e);
            }
        }
    }

    // Generate hybrid routing: separate HTML files + main index with router
    println!("🌐 Generating hybrid routing system...");
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
    };

    // Generate separate HTML files for each page/component
    let mut all_handlers = Vec::new();

    // Collect initial state once for SSG pre-rendering
    let initial_state = ssg::build_initial_state(&document);

    // Helper to convert page name to route-based filename
    fn page_to_filename(name: &str) -> String {
        let route = name
            .to_lowercase()
            .replace("page", "")
            .replace("home", "index");
        if route.is_empty() || route == "index" {
            "index.html".to_string()
        } else {
            format!("{}.html", route)
        }
    }

    // Generate HTML for each page
    for page_name in document.pages.keys() {
        let filename = page_to_filename(page_name);
        println!("📄 Generating: {} → {}", page_name, filename);
        let html_result =
            codegen::codegen_html::generate_page_content_only(&document, page_name, &options)?;
        all_handlers.extend(html_result.handlers);
        let ssg_html = ssg::apply_ssg_with_locales(
            &html_result.html,
            &initial_state,
            &document.locales,
            &config.locale,
        );
        let output_path = dist_dir.join(&filename);
        fs::write(&output_path, ssg_html)
            .map_err(|e| format!("Failed to write {:?}: {}", output_path, e))?;
    }

    // Generate HTML for each component that looks like a page
    for (component_name, component) in &document.components {
        if component_name.ends_with("Page") {
            let filename = page_to_filename(component_name);
            println!("📄 Generating: {} → {}", component_name, filename);
            // Create a temporary page from the component
            let temp_page = ast::Page {
                name: component_name.clone(),
                content: component.view.clone(),
                span: component.span,
            };
            let mut temp_doc = document.clone();
            temp_doc.pages.insert(component_name.clone(), temp_page);

            let html_result = codegen::codegen_html::generate_page_content_only(
                &temp_doc,
                component_name,
                &options,
            )?;
            all_handlers.extend(html_result.handlers);
            let ssg_html = ssg::apply_ssg_with_locales(
                &html_result.html,
                &initial_state,
                &document.locales,
                &config.locale,
            );
            let output_path = dist_dir.join(&filename);
            fs::write(&output_path, ssg_html)
                .map_err(|e| format!("Failed to write {:?}: {}", output_path, e))?;
        }
    }

    println!("✅ Generated static pages with clean URLs");

    // SPA generation handles all pages, no need for default page logic

    // Generate CSS (theme + scoped component styles)
    println!("🎨 Generating CSS (theme + scoped styles)...");

    // Generate combined CSS: theme variables + scoped component styles
    let combined_css = codegen::codegen_css::generate_combined_css(theme.as_ref(), &document);

    // Post-process CSS with LightningCSS
    let processed_css = if config.mode == "prod" {
        println!("🔧 Minifying CSS with LightningCSS...");
        css_processor::minify_css(&combined_css)?
    } else {
        css_processor::format_css(&combined_css)?
    };

    let css_path = dist_dir.join("theme.css");
    fs::write(&css_path, &processed_css)
        .map_err(|e| format!("Failed to write theme.css: {}", e))?;

    // Generate WebCore runtime JS with compiled handlers and state variables
    let runtime_js = codegen::codegen_js::generate_runtime_js(&all_handlers, &document);
    let final_js = if config.mode == "prod" {
        println!("🔧 Minifying JS...");
        codegen::codegen_js::minify_js(&runtime_js)
    } else {
        runtime_js
    };

    let js_path = dist_dir.join("webcore.js");
    fs::write(&js_path, &final_js).map_err(|e| format!("Failed to write webcore.js: {}", e))?;

    // Add a content-hash query param to <script src="webcore.js"> in every HTML file
    // so browsers never serve a stale cached runtime after a rebuild.
    let js_hash = {
        let mut h: u32 = 2_166_136_261;
        for b in final_js.bytes() {
            h ^= b as u32;
            h = h.wrapping_mul(16_777_619);
        }
        format!("{:08x}", h)
    };
    if let Ok(entries) = fs::read_dir(dist_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) == Some("html") {
                if let Ok(html) = fs::read_to_string(&p) {
                    let patched = html.replace(
                        r#"src="webcore.js""#,
                        &format!(r#"src="webcore.js?v={}""#, js_hash),
                    );
                    if patched != html {
                        let _ = fs::write(&p, patched);
                    }
                }
            }
        }
    }

    // Copy public assets
    let public_dir = Path::new("public");
    if public_dir.exists() {
        println!("📁 Copying public assets...");
        copy_dir_recursive(public_dir, dist_dir)?;
    }

    // SPA generation already handles index.html, no need for additional generation

    println!("✅ Build completed successfully!");
    Ok(())
}

fn read_config() -> Result<Config, String> {
    let config_path = Path::new("webc.toml");
    if !config_path.exists() {
        return Err("webc.toml not found".to_string());
    }

    let content =
        fs::read_to_string(config_path).map_err(|e| format!("Failed to read webc.toml: {}", e))?;

    let parsed: WebcToml =
        toml::from_str(&content).map_err(|e| format!("Failed to parse webc.toml: {}", e))?;
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

#[derive(Debug)]
struct Config {
    app_title: String,
    app_lang: String,
    locale: String,
    mode: String,
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

/// Read the `[package] name` from a `Cargo.toml` file and return its snake_case form.
fn read_wasm_module_name(path: &Path) -> Result<String, String> {
    let content =
        fs::read_to_string(path).map_err(|e| format!("Failed to read {:?}: {}", path, e))?;
    let parsed: WasmCargoToml =
        toml::from_str(&content).map_err(|e| format!("Failed to parse {:?}: {}", path, e))?;
    Ok(parsed.package.name.replace('-', "_"))
}

#[derive(Debug, Deserialize)]
struct WasmCargoToml {
    package: WasmPackage,
}

#[derive(Debug, Deserialize)]
struct WasmPackage {
    name: String,
}

type WsClients = Arc<Mutex<Vec<tungstenite::WebSocket<std::net::TcpStream>>>>;

fn dev_server_with_options(port: u16, host: Option<String>, auto_open: bool) -> Result<(), String> {
    // initial build
    build_project()?;

    // Shared list of connected WebSocket clients
    let ws_clients: WsClients = Arc::new(Mutex::new(Vec::new()));

    // start file watcher
    let rebuild_flag = Arc::new(Mutex::new(false));
    let flag_clone = rebuild_flag.clone();

    let mut watcher: RecommendedWatcher = notify::recommended_watcher(
        move |res: std::result::Result<notify::Event, notify::Error>| {
            if let Ok(event) = res {
                match event.kind {
                    EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_) => {
                        if let Ok(mut f) = flag_clone.lock() {
                            *f = true;
                        }
                    }
                    _ => {}
                }
            }
        },
    )
    .map_err(|e| format!("watcher error: {}", e))?;

    watcher
        .watch(Path::new("src"), RecursiveMode::Recursive)
        .map_err(|e| format!("watch error: {}", e))?;
    if Path::new("theme.toml").exists() {
        watcher
            .watch(Path::new("theme.toml"), RecursiveMode::NonRecursive)
            .map_err(|e| format!("watch error: {}", e))?;
    }
    watcher
        .watch(Path::new("webc.toml"), RecursiveMode::NonRecursive)
        .map_err(|e| format!("watch error: {}", e))?;

    // start HTTP server with port auto-increment if in use
    let (server, bound_port) = bind_server_with_fallback(port, 50)?;
    let ws_port = bound_port + 1;

    // start WebSocket server for hot reload
    let ws_listener = TcpListener::bind(format!("0.0.0.0:{}", ws_port))
        .map_err(|e| format!("WS bind error: {}", e))?;

    let local_host = match host.as_deref() {
        Some("0.0.0.0") => "localhost".to_string(),
        Some(h) => h.to_string(),
        None => "localhost".to_string(),
    };
    println!("🚀 Dev server running at:");
    println!("  Local:   http://{}:{}", local_host, bound_port);
    println!("  HMR:     WebSocket on ws://{}:{}", local_host, ws_port);
    let network_ip = match host.as_deref() {
        Some("0.0.0.0") | None => get_primary_ipv4(),
        Some(h) => Some(h.to_string()),
    };
    let mut qr_url: Option<String> = None;
    if let Some(ip) = network_ip.clone() {
        if ip != "127.0.0.1" && ip != "localhost" && ip != "0.0.0.0" {
            let url = format!("http://{}:{}", ip, bound_port);
            println!("  Network: {}", url);
            qr_url = Some(url);
        }
    }

    // auto-open browser
    if auto_open {
        let open_url = format!("http://{}:{}", local_host, bound_port);
        let _ = open::that_detached(open_url);
    }

    // print QR code for network URL if available
    if let Some(url) = qr_url {
        if let Ok(code) = QrCode::new(url.as_bytes()) {
            println!("\n  Scan QR (Network):");
            let qr = code.render::<unicode::Dense1x2>().quiet_zone(true).build();
            println!("{}", qr);
        }
    }

    // spawn WebSocket accept loop — adds each new client to the shared list
    let ws_clients_for_accept = ws_clients.clone();
    thread::spawn(move || {
        for stream in ws_listener.incoming() {
            match stream {
                Ok(tcp) => {
                    // non-blocking so broadcast doesn't stall on a slow client
                    let _ = tcp.set_nonblocking(false);
                    match accept(tcp) {
                        Ok(ws) => {
                            if let Ok(mut list) = ws_clients_for_accept.lock() {
                                list.push(ws);
                            }
                        }
                        Err(e) => eprintln!("WS handshake error: {}", e),
                    }
                }
                Err(e) => eprintln!("WS accept error: {}", e),
            }
        }
    });

    // spawn rebuild loop — after each successful rebuild, broadcast "reload"
    let ws_clients_for_rebuild = ws_clients.clone();
    thread::spawn(move || loop {
        thread::sleep(Duration::from_millis(200));
        let mut do_rebuild = false;
        if let Ok(mut f) = rebuild_flag.lock() {
            if *f {
                do_rebuild = true;
                *f = false;
            }
        }
        if do_rebuild {
            println!("♻️  Rebuilding...");
            match build_project() {
                Ok(_) => {
                    println!("🔄 HMR: broadcasting reload to connected clients");
                    if let Ok(mut clients) = ws_clients_for_rebuild.lock() {
                        clients.retain_mut(|ws| ws.send(Message::Text("reload".into())).is_ok());
                    }
                }
                Err(e) => eprintln!("Rebuild failed: {}", e),
            }
        }
    });

    // serve loop
    for request in server.incoming_requests() {
        if let Err(e) = handle_request(request, ws_port) {
            eprintln!("request error: {}", e);
        }
    }

    Ok(())
}

/// WebSocket HMR client script injected into HTML pages in dev mode
fn get_ws_hmr_script(ws_port: u16) -> String {
    format!(
        r#"<script>
(function(){{
  var wsPort={};
  function connect(){{
    var ws=new WebSocket('ws://'+location.hostname+':'+wsPort);
    ws.onmessage=function(e){{if(e.data==='reload'){{location.reload();}}}};
    ws.onclose=function(){{setTimeout(connect,1000);}};
    ws.onerror=function(){{ws.close();}};
  }}
  connect();
}})();
</script>"#,
        ws_port
    )
}

fn handle_request(request: Request, ws_port: u16) -> Result<(), String> {
    let url = request.url();
    // Strip query string (?v=... cache-busting, etc.) before resolving to a file path
    let url = url.split('?').next().unwrap_or(url);

    // Clean URL routing: /about → dist/about.html
    let file_path = if url == "/" {
        PathBuf::from("dist/index.html")
    } else if url.contains('.') {
        // Has extension, serve as-is
        PathBuf::from(format!("dist{}", url))
    } else {
        // Clean URL: try .html extension
        let html_path = PathBuf::from(format!("dist{}.html", url));
        if html_path.exists() {
            html_path
        } else {
            // Fallback to index.html for SPA routing
            PathBuf::from("dist/index.html")
        }
    };

    match fs::read(&file_path) {
        Ok(mut bytes) => {
            let content_type = match file_path.extension().and_then(|e| e.to_str()).unwrap_or("") {
                "html" => {
                    // Inject WebSocket HMR script
                    if let Ok(html) = String::from_utf8(bytes.clone()) {
                        let hmr_script = get_ws_hmr_script(ws_port);
                        let injected_html =
                            html.replace("</body>", &format!("{}</body>", hmr_script));
                        bytes = injected_html.into_bytes();
                    }
                    "text/html; charset=utf-8"
                }
                "css" => "text/css; charset=utf-8",
                "js" => "application/javascript; charset=utf-8",
                "png" => "image/png",
                "jpg" | "jpeg" => "image/jpeg",
                "svg" => "image/svg+xml",
                "ico" => "image/x-icon",
                "woff" | "woff2" => "font/woff2",
                "json" => "application/json",
                _ => "application/octet-stream",
            };
            let response = Response::from_data(bytes)
                .with_header(
                    tiny_http::Header::from_bytes(&b"Content-Type"[..], content_type.as_bytes())
                        .unwrap(),
                )
                .with_header(
                    tiny_http::Header::from_bytes(&b"Cache-Control"[..], b"no-cache").unwrap(),
                );
            request
                .respond(response)
                .map_err(|e| format!("respond error: {}", e))
        }
        Err(_) => {
            let response = Response::from_string("Not Found").with_status_code(404);
            request
                .respond(response)
                .map_err(|e| format!("respond error: {}", e))
        }
    }
}

fn bind_server_with_fallback(start_port: u16, max_tries: u16) -> Result<(Server, u16), String> {
    let mut port = start_port;
    for _ in 0..max_tries {
        match Server::http(("0.0.0.0", port)) {
            Ok(server) => return Ok((server, port)),
            Err(e) => {
                // Try to detect port-in-use and fallback to next port
                let is_in_use = e
                    .as_ref()
                    .downcast_ref::<std::io::Error>()
                    .map(|ioe| ioe.kind() == std::io::ErrorKind::AddrInUse)
                    .unwrap_or(false);
                if is_in_use {
                    port = port.saturating_add(1);
                    continue;
                }
                return Err(format!("server error: {}", e));
            }
        }
    }
    Err(format!(
        "no free port in range {}..{}",
        start_port,
        start_port.saturating_add(max_tries)
    ))
}

fn get_primary_ipv4() -> Option<String> {
    // Determine the primary outbound IP by opening a UDP socket
    if let Ok(socket) = UdpSocket::bind(("0.0.0.0", 0)) {
        if socket.connect(("8.8.8.8", 80)).is_ok() {
            if let Ok(addr) = socket.local_addr() {
                if let std::net::IpAddr::V4(ipv4) = addr.ip() {
                    // Skip loopback just in case
                    if !ipv4.is_loopback() {
                        return Some(ipv4.to_string());
                    }
                }
            }
        }
    }
    None
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    if src.is_dir() {
        fs::create_dir_all(dst).map_err(|e| format!("Failed to create dir {:?}: {}", dst, e))?;
        for entry in
            fs::read_dir(src).map_err(|e| format!("Failed to read dir {:?}: {}", src, e))?
        {
            let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());
            copy_dir_recursive(&src_path, &dst_path)?;
        }
    } else {
        fs::copy(src, dst).map_err(|e| format!("Failed to copy {:?} to {:?}: {}", src, dst, e))?;
    }
    Ok(())
}
