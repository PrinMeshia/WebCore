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
    /// Absolute site URL (e.g. `https://example.com`), used to emit `sitemap.xml`
    /// and the `Sitemap:` line in `robots.txt`. No sitemap is generated when unset.
    pub(crate) url: Option<String>,
    /// When true (and mode="prod"), a strict `Content-Security-Policy` meta tag is emitted.
    pub(crate) csp: bool,
    /// `[app] view_transitions` — wrap SPA navigation in the View Transitions
    /// API for animated page changes (graceful no-op where unsupported).
    pub(crate) view_transitions: bool,
    /// Indent size for `webc fmt` (default: 4).
    pub(crate) fmt_indent: Option<usize>,
    /// PWA settings (`[pwa]` in webc.toml). When present, `webc build` emits a
    /// web app manifest + service worker and links them into every page.
    pub(crate) pwa: Option<Pwa>,
    /// When true (`[i18n] static = true`), `webc build` emits one static page
    /// per locale: the default locale at the root, every other locale under a
    /// `/{locale}/` prefix, plus `hreflang` alternate links. Off by default.
    pub(crate) i18n_static: bool,
    /// RSS/Atom feed settings (`[feed]` in webc.toml). When present (and a site
    /// `url` is set), `webc build` emits `dist/feed.xml` from a data collection.
    pub(crate) feed: Option<Feed>,
    /// Responsive-image settings (`[images]`). When `widths` is non-empty,
    /// `webc build` generates resized variants and `webc:img` emits a `srcset`.
    pub(crate) images: Images,
}

/// Responsive-image settings (`[images]` in webc.toml). Empty `widths` disables
/// the feature.
#[derive(Debug, Clone, Default)]
pub(crate) struct Images {
    /// Target widths in px (variants are only made for widths < the original).
    pub(crate) widths: Vec<u32>,
    /// The `sizes` attribute emitted alongside `srcset` (default `100vw`).
    pub(crate) sizes: String,
}

/// Resolved feed settings (opt-in via a `[feed]` section). Field names map the
/// collection's item fields to feed entries; sensible defaults are applied.
#[derive(Debug, Clone)]
pub(crate) struct Feed {
    /// Data-import name to draw items from (`import posts from "..."`).
    pub(crate) collection: String,
    /// Channel title (defaults to the app title) and description.
    pub(crate) title: String,
    pub(crate) description: String,
    /// Item field names.
    pub(crate) title_field: String,
    pub(crate) link_field: String,
    pub(crate) date_field: String,
    pub(crate) summary_field: String,
    /// Prepended to the `link_field` value to build the item URL.
    pub(crate) link_prefix: String,
    /// Max number of items (newest first). `None` = all.
    pub(crate) limit: Option<usize>,
}

/// Resolved Progressive-Web-App settings (opt-in via a `[pwa]` section).
#[derive(Debug, Clone)]
pub(crate) struct Pwa {
    pub(crate) name: String,
    pub(crate) short_name: String,
    pub(crate) theme_color: String,
    pub(crate) background_color: String,
    pub(crate) display: String,
}

#[derive(Debug, Deserialize)]
struct WebcToml {
    app: Option<AppSection>,
    fmt: Option<FmtSection>,
    pwa: Option<PwaSection>,
    i18n: Option<I18nSection>,
    feed: Option<FeedSection>,
    images: Option<ImagesSection>,
}

#[derive(Debug, Deserialize)]
struct ImagesSection {
    widths: Option<Vec<u32>>,
    sizes: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FeedSection {
    collection: Option<String>,
    title: Option<String>,
    description: Option<String>,
    title_field: Option<String>,
    link_field: Option<String>,
    date_field: Option<String>,
    summary_field: Option<String>,
    link_prefix: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct I18nSection {
    /// Opt-in: generate one static HTML page per locale (default at root,
    /// others under `/{locale}/`) with `hreflang` alternates.
    #[serde(rename = "static")]
    static_pages: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct PwaSection {
    name: Option<String>,
    short_name: Option<String>,
    theme_color: Option<String>,
    background_color: Option<String>,
    display: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AppSection {
    title: Option<String>,
    lang: Option<String>,
    locale: Option<String>,
    mode: Option<String>,
    url: Option<String>,
    csp: Option<bool>,
    view_transitions: Option<bool>,
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
    let url = parsed
        .app
        .as_ref()
        .and_then(|a| a.url.clone())
        .map(|u| u.trim_end_matches('/').to_string())
        .filter(|u| !u.is_empty());
    let fmt_indent = parsed.fmt.as_ref().and_then(|f| f.indent);
    let pwa = parsed.pwa.as_ref().map(|p| {
        let name = p.name.clone().unwrap_or_else(|| app_title.clone());
        Pwa {
            short_name: p.short_name.clone().unwrap_or_else(|| name.clone()),
            name,
            theme_color: p
                .theme_color
                .clone()
                .unwrap_or_else(|| "#000000".to_string()),
            background_color: p
                .background_color
                .clone()
                .unwrap_or_else(|| "#ffffff".to_string()),
            display: p
                .display
                .clone()
                .unwrap_or_else(|| "standalone".to_string()),
        }
    });

    let i18n_static = parsed
        .i18n
        .as_ref()
        .and_then(|i| i.static_pages)
        .unwrap_or(false);

    let feed = parsed.feed.as_ref().and_then(|f| {
        // A collection is required; without it there is nothing to feed from.
        let collection = f.collection.clone()?;
        Some(Feed {
            collection,
            title: f.title.clone().unwrap_or_else(|| app_title.clone()),
            description: f.description.clone().unwrap_or_default(),
            title_field: f.title_field.clone().unwrap_or_else(|| "title".to_string()),
            link_field: f.link_field.clone().unwrap_or_else(|| "url".to_string()),
            date_field: f.date_field.clone().unwrap_or_else(|| "date".to_string()),
            summary_field: f
                .summary_field
                .clone()
                .unwrap_or_else(|| "summary".to_string()),
            link_prefix: f.link_prefix.clone().unwrap_or_default(),
            limit: f.limit,
        })
    });

    let images = parsed
        .images
        .as_ref()
        .map(|im| Images {
            widths: {
                let mut w = im.widths.clone().unwrap_or_default();
                w.retain(|&x| x > 0);
                w.sort_unstable();
                w.dedup();
                w
            },
            sizes: im.sizes.clone().unwrap_or_else(|| "100vw".to_string()),
        })
        .unwrap_or_default();

    let view_transitions = parsed
        .app
        .as_ref()
        .and_then(|a| a.view_transitions)
        .unwrap_or(false);

    Ok(Config {
        app_title,
        app_lang,
        locale,
        mode,
        url,
        csp,
        view_transitions,
        fmt_indent,
        pwa,
        i18n_static,
        feed,
        images,
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
            url: None,
            csp: false,
            view_transitions: false,
            fmt_indent: None,
            pwa: None,
            i18n_static: false,
            feed: None,
            images: Images::default(),
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
