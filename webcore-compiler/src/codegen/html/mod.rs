//! HTML Code Generator

mod analysis;
mod attrs;
mod components;
mod elements;
mod minify;
mod props;
mod shell;
mod slots;
mod tags;
mod utils;

use crate::core::ast::WebCoreDocument;
use crate::core::error::CompileError;
use crate::core::ssg::SsgContext;
#[cfg(test)]
use std::fmt::Write as _;
use std::path::Path;

use analysis::{document_needs_js, find_layout};
use shell::emit_html_shell;
#[cfg(test)]
use shell::{generate_component_content, generate_layout_shell, generate_page_content};
use slots::generate_layout_with_page_and_components;

pub(crate) use analysis::collect_page_components;
pub(crate) use minify::minify_html;

// Options passed from the build to influence the page shell
#[derive(Debug, Clone)]
pub struct HtmlPageOptions {
    pub lang: String,
    pub title: String,
    pub extra_css_files: Vec<String>,
    /// When set (prod mode), this CSS is inlined in a `<style>` tag in `<head>`
    /// and the full stylesheet is loaded deferred (non render-blocking).
    pub critical_css: Option<String>,
    /// When set (prod mode with CSP enabled), emitted as a
    /// `<meta http-equiv="Content-Security-Policy" content="...">` tag.
    pub csp_meta: Option<String>,
}

/// Tracks a compiled event handler so the JS runtime can wire it up.
///
/// `id` is a page-scoped unique string (`<prefix>btn<n>`) used as the
/// `data-webcore-handler` HTML attribute value and as the key in the
/// generated `onclick`/`onsubmit`/etc. handler map.
#[derive(Debug, Clone)]
pub struct HandlerMapping {
    pub id: String,
    pub event_type: String,
    pub expression: String,
}

/// Output of generating HTML for one page.
pub struct HtmlGenerationResult {
    pub html: String,
    /// All event handlers collected while generating this page's HTML,
    /// to be emitted into `webcore.js` as `onclick`/`onsubmit` assignments.
    pub handlers: Vec<HandlerMapping>,
}

/// Shared, page-scoped generation state threaded through the HTML emitters.
///
/// Bundles what used to be five separate parameters (`document`, `counter`,
/// `prefix`, `project_root`, …) so element generators only take the varying
/// per-node arguments plus the current CSS `scope_id`.
pub(super) struct GenContext<'a> {
    pub document: &'a WebCoreDocument,
    /// Page-scoped id prefix for generated handler ids (`<prefix>btn<n>`).
    pub prefix: &'a str,
    /// Project root for compile-time asset lookups (`webc:img` dimensions).
    pub project_root: Option<&'a Path>,
    /// SSG pre-rendering context: when set, interpolation spans are filled
    /// with their initial value and `@if`/`@else` divs get an initial
    /// `display` style at emission time.
    pub ssg: Option<&'a SsgContext<'a>>,
    /// Monotonic counter for unique handler ids within the page.
    pub counter: usize,
}

#[cfg(test)]
pub(crate) fn generate_spa_html(
    document: &WebCoreDocument,
    options: &HtmlPageOptions,
) -> Result<HtmlGenerationResult, CompileError> {
    let layout = find_layout(document)?;

    // SPA always needs JS (routing)
    let needs_js = true;
    let mut html = emit_html_shell(
        &options.title,
        &options.lang,
        &options.extra_css_files,
        needs_js,
        options.critical_css.as_deref(),
        options.csp_meta.as_deref(),
    );

    // Generate layout shell (without page content, just the structure)
    let (layout_content, mut all_handlers) = generate_layout_shell(layout, document, None)?;
    html.push_str(&layout_content);

    // Add routing container
    html.push_str("  <div id=\"webcore-router\" style=\"display: none;\">\n");

    // Generate all pages as hidden divs
    for (page_name, page) in &document.pages {
        let (page_html, handlers) = generate_page_content(page, document, None)?;
        writeln!(
            html,
            "    <div id=\"page-{}\" data-route=\"/{}\">",
            page_name.to_lowercase(),
            page_name.to_lowercase()
        )
        .expect("write! to String is infallible");
        html.push_str(&page_html);
        html.push_str("    </div>\n");
        all_handlers.extend(handlers);
    }

    // Generate all page components as hidden divs
    for (component_name, component) in &document.components {
        if component_name.ends_with("Page") {
            let (page_html, handlers) = generate_component_content(component, document, None)?;
            let route_name = component_name.replace("Page", "").to_lowercase();
            writeln!(
                html,
                "    <div id=\"page-{route_name}\" data-route=\"/{route_name}\">"
            )
            .expect("write! to String is infallible");
            html.push_str(&page_html);
            html.push_str("    </div>\n");
            all_handlers.extend(handlers);
        }
    }

    html.push_str("  </div>\n");

    html.push_str("  <script defer src=\"/assets/webcore.js\"></script>\n");
    html.push_str("</body>\n</html>");

    Ok(HtmlGenerationResult {
        html,
        handlers: all_handlers,
    })
}

/// Generate one full page (shell + layout + content).
///
/// `ssg` enables compile-time pre-rendering of initial state values; pass
/// `None` to emit runtime-only bindings (used by most golden tests).
pub(crate) fn generate_page(
    document: &WebCoreDocument,
    page_name: &str,
    options: &HtmlPageOptions,
    project_root: Option<&Path>,
    ssg: Option<&SsgContext>,
) -> Result<HtmlGenerationResult, CompileError> {
    // Find the page
    let page = document
        .pages
        .get(page_name)
        .ok_or_else(|| CompileError::MissingPage {
            name: page_name.to_string(),
        })?;

    let layout = find_layout(document)?;

    // critical_css injects a deferred <link> whose media swap requires JS;
    // force needs_js=true so webcore.js is included even on otherwise static pages.
    let needs_js = document_needs_js(document, page_name) || options.critical_css.is_some();
    let mut html = emit_html_shell(
        &options.title,
        &options.lang,
        &options.extra_css_files,
        needs_js,
        options.critical_css.as_deref(),
        options.csp_meta.as_deref(),
    );

    // Generate layout content, replacing slots with page content
    let (layout_content, handlers) =
        generate_layout_with_page_and_components(layout, page, document, project_root, ssg)?;
    html.push_str(&layout_content);

    if needs_js {
        html.push_str("  <script defer src=\"/assets/webcore.js\"></script>\n");
    }
    html.push_str("</body>\n</html>");

    Ok(HtmlGenerationResult { html, handlers })
}

/// Test-only alias of [`generate_page`] without root/SSG, kept for the golden tests.
#[cfg(test)]
pub(crate) fn generate_html(
    document: &WebCoreDocument,
    page_name: &str,
    options: &HtmlPageOptions,
) -> Result<HtmlGenerationResult, CompileError> {
    generate_page(document, page_name, options, None, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ast::{Attribute, AttributeValue, Element, Layout, Page, Span};

    fn make_button_page(page_name: &str) -> Page {
        Page {
            name: page_name.to_string(),
            head: None,
            content: vec![Element::Tag {
                name: "button".to_string(),
                attributes: vec![Attribute {
                    name: "on:click".to_string(),
                    value: AttributeValue::Expression("count += 1".to_string()),
                    span: Span::default(),
                }],
                content: vec![],
                span: Span::default(),
            }],
            span: Span::default(),
        }
    }

    fn make_doc_with_pages(pages: Vec<(&str, Page)>) -> WebCoreDocument {
        let mut doc = WebCoreDocument {
            app: None,
            store: vec![],
            locales: std::collections::BTreeMap::new(),
            default_locale: String::new(),
            wasm_module: None,
            layouts: std::collections::BTreeMap::new(),
            pages: std::collections::BTreeMap::new(),
            components: std::collections::BTreeMap::new(),
            imports: vec![],
            data_imports: std::collections::BTreeMap::new(),
        };
        doc.layouts.insert(
            "MainLayout".to_string(),
            Layout {
                name: "MainLayout".to_string(),
                content: vec![Element::Slot("content".to_string(), Span::default())],
                span: Span::default(),
            },
        );
        for (name, page) in pages {
            doc.pages.insert(name.to_string(), page);
        }
        doc
    }

    #[test]
    fn page_handlers_do_not_collide() {
        let home = make_button_page("home");
        let about = make_button_page("about");
        let doc = make_doc_with_pages(vec![("home", home), ("about", about)]);

        let opts = HtmlPageOptions {
            lang: "en".to_string(),
            title: "t".to_string(),
            extra_css_files: vec![],
            critical_css: None,
            csp_meta: None,
        };
        let res = generate_spa_html(&doc, &opts).expect("spa ok");

        let ids: Vec<&str> = res.handlers.iter().map(|h| h.id.as_str()).collect();
        // All handler IDs must be unique
        let unique: std::collections::HashSet<&str> = ids.iter().copied().collect();
        assert_eq!(ids.len(), unique.len(), "duplicate handler IDs: {:?}", ids);
        // IDs must carry the page prefix
        assert!(
            res.handlers.iter().any(|h| h.id.starts_with("home")),
            "expected a 'home' prefixed handler, got {:?}",
            ids
        );
        assert!(
            res.handlers.iter().any(|h| h.id.starts_with("about")),
            "expected an 'about' prefixed handler, got {:?}",
            ids
        );
    }

    #[test]
    fn dynamic_attr_emits_data_webcore_attr() {
        let mut doc = WebCoreDocument {
            app: None,
            store: vec![],
            locales: std::collections::BTreeMap::new(),
            default_locale: String::new(),
            wasm_module: None,
            layouts: std::collections::BTreeMap::new(),
            pages: std::collections::BTreeMap::new(),
            components: std::collections::BTreeMap::new(),
            imports: vec![],
            data_imports: std::collections::BTreeMap::new(),
        };
        doc.layouts.insert(
            "MainLayout".to_string(),
            Layout {
                name: "MainLayout".to_string(),
                content: vec![Element::Slot("content".to_string(), Span::default())],
                span: Span::default(),
            },
        );
        doc.pages.insert(
            "test".to_string(),
            Page {
                name: "test".to_string(),
                head: None,
                content: vec![Element::Tag {
                    name: "div".to_string(),
                    attributes: vec![Attribute {
                        name: "class".to_string(),
                        value: AttributeValue::Expression("dynamicClass".to_string()),
                        span: Span::default(),
                    }],
                    content: vec![],
                    span: Span::default(),
                }],
                span: Span::default(),
            },
        );
        let opts = HtmlPageOptions {
            lang: "fr".to_string(),
            title: "t".to_string(),
            extra_css_files: vec![],
            critical_css: None,
            csp_meta: None,
        };
        let res = generate_html(&doc, "test", &opts).expect("html ok");
        assert!(
            res.html.contains("data-webcore-bound"),
            "marker attribute missing"
        );
        assert!(
            res.html
                .contains("data-webcore-attr-class=\"dynamicClass\""),
            "binding attribute missing"
        );
        assert!(
            !res.html.contains("class=\"{}\""),
            "broken placeholder still present"
        );
    }

    #[test]
    fn event_fallback_uses_on_event_attribute() {
        // Build minimal doc with a button using an unknown event
        let mut doc = WebCoreDocument {
            app: None,
            store: vec![],
            locales: std::collections::BTreeMap::new(),
            default_locale: String::new(),
            wasm_module: None,
            layouts: std::collections::BTreeMap::new(),
            pages: std::collections::BTreeMap::new(),
            components: std::collections::BTreeMap::new(),
            imports: vec![],
            data_imports: std::collections::BTreeMap::new(),
        };
        doc.layouts.insert(
            "MainLayout".to_string(),
            Layout {
                name: "MainLayout".to_string(),
                content: vec![Element::Slot("content".to_string(), Span::default())],
                span: Span::default(),
            },
        );
        doc.pages.insert(
            "test".to_string(),
            Page {
                name: "test".to_string(),
                head: None,
                content: vec![Element::Tag {
                    name: "button".to_string(),
                    attributes: vec![Attribute {
                        name: "on:foo".to_string(),
                        value: AttributeValue::Expression("count += 1".to_string()),
                        span: Span::default(),
                    }],
                    content: vec![],
                    span: Span::default(),
                }],
                span: Span::default(),
            },
        );

        let opts = HtmlPageOptions {
            lang: "fr".to_string(),
            title: "t".to_string(),
            extra_css_files: vec![],
            critical_css: None,
            csp_meta: None,
        };
        let res = generate_html(&doc, "test", &opts).expect("html ok");
        assert!(res.html.contains("data-webcore-e=\"foo\""));
    }
}
