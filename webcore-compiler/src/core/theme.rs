//! Theme system for `WebCore`

use std::collections::BTreeMap;
use std::fs;

#[derive(Debug, Clone)]
pub struct Theme {
    #[allow(dead_code)]
    pub name: String,
    pub colors: BTreeMap<String, String>,
    pub fonts: BTreeMap<String, String>,
    pub spacing: BTreeMap<String, String>,
    pub radius: BTreeMap<String, String>,
    pub breakpoints: BTreeMap<String, String>,
}

#[derive(Debug, serde::Deserialize)]
struct ThemeConfig {
    theme: ThemeData,
}

#[derive(Debug, serde::Deserialize)]
struct ThemeData {
    name: String,
    #[serde(default)]
    colors: BTreeMap<String, String>,
    #[serde(default)]
    fonts: BTreeMap<String, String>,
    #[serde(default)]
    spacing: BTreeMap<String, String>,
    #[serde(default)]
    radius: BTreeMap<String, String>,
    #[serde(default)]
    breakpoints: BTreeMap<String, String>,
}

pub(crate) fn load_theme(theme_path: &str) -> Result<Theme, String> {
    let content = fs::read_to_string(theme_path)
        .map_err(|e| format!("Failed to read theme file {theme_path}: {e}"))?;

    let config: ThemeConfig = toml::from_str(&content)
        .map_err(|e| format!("Failed to parse theme file {theme_path}: {e}"))?;

    Ok(Theme {
        name: config.theme.name,
        colors: config.theme.colors,
        fonts: config.theme.fonts,
        spacing: config.theme.spacing,
        radius: config.theme.radius,
        breakpoints: config.theme.breakpoints,
    })
}
