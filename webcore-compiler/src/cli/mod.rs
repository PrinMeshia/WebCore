//! CLI argument parsing and command dispatch.

pub(crate) mod assets;
pub(crate) mod build;
pub(crate) mod check;
pub(crate) mod config;
pub(crate) mod loader;
pub(crate) mod output;
pub(crate) mod serve;
use std::env;
use std::fs;
use std::path::Path;

pub(crate) fn run() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_help();
        return;
    }

    match args[1].as_str() {
        "new" => {
            let name = if let Some(n) = args.get(2) {
                n.clone()
            } else {
                eprintln!("Usage: webc new <nom-du-projet>");
                std::process::exit(1);
            };
            if let Err(e) = new_project(&name) {
                eprintln!("Erreur : {e}");
                std::process::exit(1);
            }
        }
        "build" => {
            if let Err(e) = build::build_project() {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        "dev" => {
            let mut port: u16 = 3000;
            let mut host: Option<String> = None;
            let mut auto_open = false;
            let mut i = 2;
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
            if let Err(e) = serve::serve_project(port, host, auto_open) {
                eprintln!("Dev server error: {e}");
                std::process::exit(1);
            }
        }
        "check" => match check::check_project() {
            Ok(()) => {}
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        },
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
    println!("  check        Valider le projet sans générer de fichiers");
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

/// Convert kebab-case or `snake_case` to `PascalCase` (e.g. "my-app" → "`MyApp`").
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
