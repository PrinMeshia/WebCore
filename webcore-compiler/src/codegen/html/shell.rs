//! HTML document shell: `<!DOCTYPE>`, `<head>` (CSS/JS links, critical CSS,
//! CSP meta) — plus test-only SPA layout helpers.

#[cfg(test)]
use crate::codegen::css::generate_scope_id;
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
pub(super) fn emit_html_shell(
    title: &str,
    lang: &str,
    extra_css_files: &[String],
    needs_js: bool,
    critical_css: Option<&str>,
    csp_meta: Option<&str>,
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
    writeln!(html, "  <title>{}</title>", html_escape(title))
        .expect("write! to String is infallible");
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
    if needs_js {
        html.push_str("  <link rel=\"preload\" as=\"script\" href=\"/assets/webcore.js\">\n");
    }
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
    };

    for element in &component.view {
        let (html, handlers) = generate_element(element, &mut ctx, Some(&scope_id))?;
        result.push_str(&html);
        result.push('\n');
        all_handlers.extend(handlers);
    }

    Ok((result, all_handlers))
}
