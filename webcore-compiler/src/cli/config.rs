//! Project configuration: read and parse `webc.toml`.

use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug)]
pub(crate) struct Config {
    pub(crate) app_title: String,
    pub(crate) app_lang: String,
    pub(crate) locale: String,
    pub(crate) mode: String,
    /// When true (and mode="prod"), a strict `Content-Security-Policy` meta tag is emitted.
    pub(crate) csp: bool,
    /// Indent size for `webc fmt` (default: 4).
    pub(crate) fmt_indent: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct WebcToml {
    app: Option<AppSection>,
    fmt: Option<FmtSection>,
}

#[derive(Debug, Deserialize)]
struct AppSection {
    title: Option<String>,
    lang: Option<String>,
    locale: Option<String>,
    mode: Option<String>,
    csp: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct FmtSection {
    indent: Option<usize>,
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

    let csp = parsed.app.as_ref().and_then(|a| a.csp).unwrap_or(false);
    let fmt_indent = parsed.fmt.as_ref().and_then(|f| f.indent);

    Ok(Config {
        app_title,
        app_lang,
        locale,
        mode,
        csp,
        fmt_indent,
    })
}

/// Load config, returning defaults if webc.toml is missing.
pub(crate) fn load_config() -> Result<Config, String> {
    if !Path::new("webc.toml").exists() {
        return Ok(Config {
            app_title: "WebCore App".to_string(),
            app_lang: "fr".to_string(),
            locale: "fr".to_string(),
            mode: "dev".to_string(),
            csp: false,
            fmt_indent: None,
        });
    }
    read_config()
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
