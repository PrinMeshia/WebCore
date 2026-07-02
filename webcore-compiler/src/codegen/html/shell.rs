//! HTML document shell: `<!DOCTYPE>`, `<head>` (CSS/JS links, critical CSS,
//! CSP meta) — plus test-only SPA layout helpers.

#[cfg(test)]
use crate::codegen::css::generate_scope_id;
use crate::core::ast::HeadBlock;
#[cfg(test)]
use crate::core::ast::{Component, Element, Layout, Page, WebCoreDocument};
#[cfg(test)]
use crate::core::error::CompileError;
use std::fmt::Write as _;
#[cfg(test)]
use std::path::Path;

#[cfg(test)]
use super::elements::generate_element;
use super::utils::html_escape;
#[cfg(test)]
use super::utils::safe_id_prefix;
#[cfg(test)]
use super::GenContext;
#[cfg(test)]
use super::HandlerMapping;

/// Emit the standard HTML shell opening: `<!DOCTYPE html>…<head>…</head><body>\n`.
///
/// `title` and `css_href` are HTML-escaped by the caller or here as appropriate.
/// `needs_js` controls whether the preload hint for webcore.js is included.
/// When `critical_css` is set, it is inlined in a `<style>` tag and the full
/// stylesheet is loaded non-blocking (`media="print"` swap + `<noscript>` fallback).
/// `head` carries the page's `head { }` block: its `title` overrides the global
/// one, its `meta` entries and `favicon` are emitted into `<head>`.
#[allow(clippy::too_many_arguments)]
pub(super) fn emit_html_shell(
    title: &str,
    lang: &str,
    extra_css_files: &[String],
    needs_js: bool,
    critical_css: Option<&str>,
    csp_meta: Option<&str>,
    head: Option<&HeadBlock>,
    site_url: Option<&str>,
    canonical: Option<&str>,
    pwa: Option<&super::PwaHead>,
) -> String {
    let mut html = String::new();
    html.push_str("<!DOCTYPE html>\n");
    write!(html, "<html lang=\"{}\">\n<head>\n", html_escape(lang))
        .expect("write! to String is infallible");
    html.push_str("  <meta charset=\"UTF-8\">\n");
    html.push_str("  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n");
    if let Some(csp) = csp_meta {
        writeln!(
            html,
            "  <meta http-equiv=\"Content-Security-Policy\" content=\"{}\">",
            html_escape(csp)
        )
        .expect("write! to String is infallible");
    }
    // The page's head{} title overrides the global title when present.
    let effective_title = head.and_then(|h| h.title.as_deref()).unwrap_or(title);
    writeln!(html, "  <title>{}</title>", html_escape(effective_title))
        .expect("write! to String is infallible");
    // Canonical URL (site_url + route) → <link rel="canonical"> + og:url.
    if let Some(url) = canonical {
        let esc = html_escape(url);
        writeln!(html, "  <link rel=\"canonical\" href=\"{esc}\">")
            .expect("write! to String is infallible");
        writeln!(html, "  <meta property=\"og:url\" content=\"{esc}\">")
            .expect("write! to String is infallible");
    }
    if let Some(h) = head {
        for (key, value) in &h.metas {
            // OpenGraph/Facebook tags use `property`; everything else uses `name`.
            let attr = if key.starts_with("og:") {
                "property"
            } else {
                "name"
            };
            // Social image tags must be absolute URLs for crawlers to fetch them:
            // rewrite root-relative `og:image` / `twitter:image` using site_url.
            let content = match site_url {
                Some(base) if key.contains("image") && value.starts_with('/') => {
                    format!("{base}{value}")
                }
                _ => value.clone(),
            };
            writeln!(
                html,
                "  <meta {}=\"{}\" content=\"{}\">",
                attr,
                html_escape(key),
                html_escape(&content)
            )
            .expect("write! to String is infallible");
        }
        if let Some(icon) = &h.favicon {
            writeln!(html, "  <link rel=\"icon\" href=\"{}\">", html_escape(icon))
                .expect("write! to String is infallible");
        }
    }
    // PWA: manifest link, theme color and Apple web-app meta/icon tags.
    if let Some(p) = pwa {
        writeln!(
            html,
            "  <link rel=\"manifest\" href=\"/manifest.webmanifest\">\n  \
             <meta name=\"theme-color\" content=\"{tc}\">\n  \
             <meta name=\"mobile-web-app-capable\" content=\"yes\">\n  \
             <meta name=\"apple-mobile-web-app-capable\" content=\"yes\">\n  \
             <meta name=\"apple-mobile-web-app-status-bar-style\" content=\"black-translucent\">\n  \
             <meta name=\"apple-mobile-web-app-title\" content=\"{sn}\">\n  \
             <link rel=\"apple-touch-icon\" href=\"{icon}\">",
            tc = html_escape(&p.theme_color),
            sn = html_escape(&p.short_name),
            icon = html_escape(&p.apple_icon),
        )
        .expect("write! to String is infallible");
    }
    if let Some(css) = critical_css {
        // Critical CSS: inline the page's own styles, defer the full stylesheet.
        // `data-webcore-defer` is swapped to media="all" by DOMContentLoaded (CSP-safe).
        // Escape </style> sequences so injected CSS can't break out of the tag.
        let safe_css = css.replace("</", "<\\/");
        writeln!(html, "  <style>{safe_css}</style>").expect("write! to String is infallible");
        html.push_str("  <link rel=\"stylesheet\" href=\"/assets/theme.css\" media=\"print\" data-webcore-defer>\n");
        html.push_str(
            "  <noscript><link rel=\"stylesheet\" href=\"/assets/theme.css\"></noscript>\n",
        );
    } else {
        html.push_str("  <link rel=\"stylesheet\" href=\"/assets/theme.css\">\n");
    }
    for css in extra_css_files {
        writeln!(
            html,
            "  <link rel=\"stylesheet\" href=\"/assets/{}\">",
            html_escape(css)
        )
        .expect("write! to String is infallible");
    }
    // v3: JS is inlined per-page, no preload hint needed
    let _ = needs_js; // consumed below in generate_page for inline <script>
    html.push_str("</head>\n<body>\n");
    html
}

// Helper functions for SPA generation (test-only)
#[cfg(test)]
pub(super) fn generate_layout_shell(
    layout: &Layout,
    document: &WebCoreDocument,
    project_root: Option<&Path>,
) -> Result<(String, Vec<HandlerMapping>), CompileError> {
    let mut result = String::new();
    let mut all_handlers = Vec::new();
    let mut ctx = GenContext {
        document,
        prefix: "ly",
        project_root,
        ssg: None,
        counter: 0,
        compiled_vars: None,
        expr_map: vec![],
        expr_spans: vec![],
        expr_counter: 0,
        has_route_params: false,
        has_query_params: false,
    };

    for element in &layout.content {
        match element {
            Element::Slot(slot_name, _span) => {
                if slot_name == "content" {
                    result.push_str("  <main id=\"webcore-content\">\n");
                    result.push_str(
                        "    <!-- Content will be loaded here by the hybrid router -->\n",
                    );
                    result.push_str("  </main>\n");
                } else {
                    write!(result, "  <div id=\"webcore-slot-{slot_name}\">\n    <!-- Named slot: {slot_name} -->\n  </div>\n").expect("write! to String is infallible");
                }
            }
            Element::Tag { name, content, .. } => {
                if name == "main" && content.len() == 1 {
                    if let Element::Slot(slot_name, _) = &content[0] {
                        if slot_name == "content" {
                            result.push_str("  <main id=\"webcore-content\">\n");
                            result.push_str(
                                "    <!-- Content will be loaded here by the hybrid router -->\n",
                            );
                            result.push_str("  </main>\n");
                            continue;
                        }
                    }
                }
                let (element_html, handlers) = generate_element(element, &mut ctx, None)?;
                result.push_str(&element_html);
                all_handlers.extend(handlers);
            }
            _ => {
                let (element_html, handlers) = generate_element(element, &mut ctx, None)?;
                result.push_str(&element_html);
                result.push('\n');
                all_handlers.extend(handlers);
            }
        }
    }

    Ok((result, all_handlers))
}

#[cfg(test)]
pub(super) fn generate_page_content(
    page: &Page,
    document: &WebCoreDocument,
    project_root: Option<&Path>,
) -> Result<(String, Vec<HandlerMapping>), CompileError> {
    let prefix = safe_id_prefix(&page.name);
    let mut result = String::new();
    let mut all_handlers = Vec::new();
    let mut ctx = GenContext {
        document,
        prefix: &prefix,
        project_root,
        ssg: None,
        counter: 0,
        compiled_vars: None,
        expr_map: vec![],
        expr_spans: vec![],
        expr_counter: 0,
        has_route_params: false,
        has_query_params: false,
    };

    for element in &page.content {
        let (html, handlers) = generate_element(element, &mut ctx, None)?;
        result.push_str(&html);
        result.push('\n');
        all_handlers.extend(handlers);
    }

    Ok((result, all_handlers))
}

#[cfg(test)]
pub(super) fn generate_component_content(
    component: &Component,
    document: &WebCoreDocument,
    project_root: Option<&Path>,
) -> Result<(String, Vec<HandlerMapping>), CompileError> {
    let scope_id = generate_scope_id(&component.name);
    let prefix = safe_id_prefix(&component.name);
    let mut result = String::new();
    let mut all_handlers = Vec::new();
    let mut ctx = GenContext {
        document,
        prefix: &prefix,
        project_root,
        ssg: None,
        counter: 0,
        compiled_vars: None,
        expr_map: vec![],
        expr_spans: vec![],
        expr_counter: 0,
        has_route_params: false,
        has_query_params: false,
    };

    for element in &component.view {
        let (html, handlers) = generate_element(element, &mut ctx, Some(&scope_id))?;
        result.push_str(&html);
        result.push('\n');
        all_handlers.extend(handlers);
    }

    Ok((result, all_handlers))
}
