//! HTML Code Generator

#[cfg(test)]
use crate::ast::Component;
use crate::ast::{Attribute, AttributeValue, Element, Layout, Page, WebCoreDocument};
use crate::codegen::attr_names;
use crate::codegen::codegen_css::generate_scope_id;
use crate::error::CompileError;
use std::fmt::Write as _;
use std::path::Path;

// Options passed from the build to influence the page shell
#[derive(Debug, Clone)]
pub struct HtmlPageOptions {
    pub lang: String,
    pub title: String,
    pub extra_css_files: Vec<String>,
    /// When set (prod mode), this CSS is inlined in a `<style>` tag in `<head>`
    /// and the full stylesheet is loaded deferred (non render-blocking).
    pub critical_css: Option<String>,
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

/// Emit the standard HTML shell opening: `<!DOCTYPE html>…<head>…</head><body>\n`.
///
/// `title` and `css_href` are HTML-escaped by the caller or here as appropriate.
/// `needs_js` controls whether the preload hint for webcore.js is included.
/// When `critical_css` is set, it is inlined in a `<style>` tag and the full
/// stylesheet is loaded non-blocking (`media="print"` swap + `<noscript>` fallback).
fn emit_html_shell(
    title: &str,
    lang: &str,
    extra_css_files: &[String],
    needs_js: bool,
    critical_css: Option<&str>,
) -> String {
    let mut html = String::new();
    html.push_str("<!DOCTYPE html>\n");
    write!(html, "<html lang=\"{}\">\n<head>\n", html_escape(lang)).unwrap();
    html.push_str("  <meta charset=\"UTF-8\">\n");
    html.push_str("  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n");
    writeln!(html, "  <title>{}</title>", html_escape(title)).unwrap();
    if let Some(css) = critical_css {
        // Critical CSS: inline the page's own styles, defer the full stylesheet.
        writeln!(html, "  <style>{css}</style>").unwrap();
        html.push_str("  <link rel=\"stylesheet\" href=\"/assets/theme.css\" media=\"print\" onload=\"this.media='all'\">\n");
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
        .unwrap();
    }
    if needs_js {
        html.push_str("  <link rel=\"preload\" as=\"script\" href=\"/assets/webcore.js\">\n");
    }
    html.push_str("</head>\n<body>\n");
    html
}

/// Extract path from `webcore_navigate(path)` expression
fn extract_navigate_path(expr: &str) -> Option<String> {
    // Match webcore_navigate(/path) or webcore_navigate(root) or webcore_navigate("/path")
    let expr = expr.trim();

    if let Some(start) = expr.find("webcore_navigate(") {
        let after_paren = &expr[start + 17..]; // After "webcore_navigate("
        if let Some(end) = after_paren.find(')') {
            let path = after_paren[..end].trim();

            // Handle different path formats
            let clean_path = if path == "root" {
                "/".to_string()
            } else if path.starts_with('"') && path.ends_with('"') {
                // Quoted path: "/about"
                path[1..path.len() - 1].to_string()
            } else if path.starts_with('/') {
                // Unquoted path: /about
                path.to_string()
            } else {
                // Fallback
                format!("/{path}")
            };

            return Some(clean_path);
        }
    }
    None
}

fn find_layout(document: &WebCoreDocument) -> Result<&Layout, CompileError> {
    // Prefer the layout declared in the app block, then fall back to MainLayout / default
    if let Some(name) = document.app.as_ref().and_then(|a| a.layout.as_deref()) {
        if let Some(layout) = document.layouts.get(name) {
            return Ok(layout);
        }
        return Err(CompileError::MissingLayout {
            name: name.to_string(),
            available: document.layouts.keys().cloned().collect(),
        });
    }
    document
        .layouts
        .get("MainLayout")
        .or_else(|| document.layouts.get("default"))
        .ok_or_else(|| CompileError::MissingLayout {
            name: "MainLayout".to_string(),
            available: document.layouts.keys().cloned().collect(),
        })
}

/// Collect the names of all components reachable from `elements`,
/// following component references recursively (a component used by
/// another component is also collected).
fn collect_components_in(
    elements: &[Element],
    document: &WebCoreDocument,
    out: &mut std::collections::HashSet<String>,
) {
    for elem in elements {
        match elem {
            Element::Component { name, content, .. } => {
                if out.insert(name.clone()) {
                    if let Some(comp) = document.components.get(name) {
                        collect_components_in(&comp.view, document, out);
                    }
                }
                collect_components_in(content, document, out);
            }
            Element::Tag { content, .. }
            | Element::SlotContent { content, .. }
            | Element::For { content, .. }
            | Element::ErrorBlock { content, .. } => {
                collect_components_in(content, document, out);
            }
            Element::If {
                then_branch,
                else_branch,
                ..
            } => {
                collect_components_in(then_branch, document, out);
                if let Some(else_b) = else_branch {
                    collect_components_in(else_b, document, out);
                }
            }
            _ => {}
        }
    }
}

/// Return the set of component names used by `page_name` (page content +
/// layout), following nested component references. Used by the build to
/// assemble per-page critical CSS.
pub(crate) fn collect_page_components(
    document: &WebCoreDocument,
    page_name: &str,
) -> std::collections::HashSet<String> {
    let mut out = std::collections::HashSet::new();
    // A component rendered as a page (e.g. `PostPage`) carries its own style
    // block but never appears as an Element::Component — include it directly.
    if document.components.contains_key(page_name) {
        out.insert(page_name.to_string());
    }
    if let Some(page) = document.pages.get(page_name) {
        collect_components_in(&page.content, document, &mut out);
    }
    if let Ok(layout) = find_layout(document) {
        collect_components_in(&layout.content, document, &mut out);
    }
    out
}

fn elements_need_js(elements: &[crate::ast::Element]) -> bool {
    use crate::ast::Element;
    for elem in elements {
        match elem {
            Element::Interpolation(..) => return true,
            Element::If { .. } => return true,
            Element::For { .. } => return true,
            Element::Tag {
                attributes,
                content,
                ..
            } => {
                for attr in attributes {
                    if matches!(attr.value, AttributeValue::Expression(_)) {
                        return true;
                    }
                    if attr.name.starts_with("on:") {
                        return true;
                    }
                    if attr.name.starts_with("class:") {
                        return true;
                    }
                    if attr.name.starts_with("style:") {
                        return true;
                    }
                    if attr.name.starts_with("validate:") {
                        return true;
                    }
                    if attr.name.starts_with("ref:") {
                        return true;
                    }
                    if attr.name == "bind:value" || attr.name == "bind:checked" {
                        return true;
                    }
                }
                if elements_need_js(content) {
                    return true;
                }
            }
            Element::Component {
                attributes,
                content,
                ..
            } => {
                if elements_need_js(content) {
                    return true;
                }
                for attr in attributes {
                    if matches!(attr.value, AttributeValue::Expression(_)) {
                        return true;
                    }
                }
            }
            Element::SlotContent { content, .. } => {
                if elements_need_js(content) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

fn document_needs_js(document: &WebCoreDocument, page_name: &str) -> bool {
    if !document.store.is_empty() {
        return true;
    }
    if document
        .app
        .as_ref()
        .map(|a| !a.routes.is_empty())
        .unwrap_or(false)
    {
        return true;
    }
    for comp in document.components.values() {
        if !comp.state.is_empty()
            || !comp.computed.is_empty()
            || comp.http.is_some()
            || comp.mount_body.is_some()
            || comp.destroy_body.is_some()
        {
            return true;
        }
    }
    if let Some(page) = document.pages.get(page_name) {
        if elements_need_js(&page.content) {
            return true;
        }
    }
    false
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
        .unwrap();
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
            .unwrap();
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

pub(crate) fn generate_page_content_only(
    document: &WebCoreDocument,
    page_name: &str,
    options: &HtmlPageOptions,
) -> Result<HtmlGenerationResult, CompileError> {
    generate_page_content_only_with_root(document, page_name, options, None)
}

pub(crate) fn generate_page_content_only_with_root(
    document: &WebCoreDocument,
    page_name: &str,
    options: &HtmlPageOptions,
    project_root: Option<&Path>,
) -> Result<HtmlGenerationResult, CompileError> {
    // Find the page
    let page = document
        .pages
        .get(page_name)
        .ok_or_else(|| CompileError::MissingPage {
            name: page_name.to_string(),
        })?;

    let layout = find_layout(document)?;

    let needs_js = document_needs_js(document, page_name);
    let mut html = emit_html_shell(
        &options.title,
        &options.lang,
        &options.extra_css_files,
        needs_js,
        options.critical_css.as_deref(),
    );

    // Generate layout content, replacing slots with page content
    let (layout_content, handlers) =
        generate_layout_with_page_and_components(layout, page, document, project_root)?;
    html.push_str(&layout_content);

    if needs_js {
        html.push_str("  <script defer src=\"/assets/webcore.js\"></script>\n");
    }
    html.push_str("</body>\n</html>");

    Ok(HtmlGenerationResult { html, handlers })
}

#[cfg(test)]
pub(crate) fn generate_html(
    document: &WebCoreDocument,
    page_name: &str,
    options: &HtmlPageOptions,
) -> Result<HtmlGenerationResult, CompileError> {
    // Find the page
    let page = document
        .pages
        .get(page_name)
        .ok_or_else(|| CompileError::MissingPage {
            name: page_name.to_string(),
        })?;

    let layout = find_layout(document)?;

    let needs_js = document_needs_js(document, page_name);
    let mut html = emit_html_shell(
        &options.title,
        &options.lang,
        &options.extra_css_files,
        needs_js,
        options.critical_css.as_deref(),
    );

    // Generate layout content, replacing slots with page content
    let (layout_content, handlers) =
        generate_layout_with_page_and_components(layout, page, document, None)?;
    html.push_str(&layout_content);

    if needs_js {
        html.push_str("  <script defer src=\"/assets/webcore.js\"></script>\n");
    }
    html.push_str("</body>\n</html>");

    Ok(HtmlGenerationResult { html, handlers })
}

/// Recursively replace Slot placeholders in a layout tree with provided page content.
fn resolve_slots(
    elements: &[Element],
    slot_map: &std::collections::HashMap<String, Vec<Element>>,
    default_content: &[Element],
) -> Vec<Element> {
    let mut resolved = Vec::new();
    for element in elements {
        match element {
            Element::Slot(name, _) => {
                if name == "content" {
                    if let Some(content) = slot_map.get("content") {
                        resolved.extend_from_slice(content);
                    } else {
                        resolved.extend_from_slice(default_content);
                    }
                } else if let Some(content) = slot_map.get(name.as_str()) {
                    resolved.extend_from_slice(content);
                }
            }
            Element::SlotContent {
                name,
                content,
                span: _,
            } => {
                // SlotContent in a layout = slot position with default content
                if let Some(page_content) = slot_map.get(name.as_str()) {
                    // Page provided content → use it (override default)
                    resolved.extend_from_slice(page_content);
                } else if name == "content" {
                    // Default content slot → use page's non-slot elements
                    resolved.extend_from_slice(default_content);
                } else {
                    // Named slot with no page content → render layout's default
                    let resolved_defaults = resolve_slots(content, slot_map, default_content);
                    resolved.extend(resolved_defaults);
                }
            }
            Element::Tag {
                name,
                attributes,
                content,
                span,
            } => {
                resolved.push(Element::Tag {
                    name: name.clone(),
                    attributes: attributes.clone(),
                    content: resolve_slots(content, slot_map, default_content),
                    span: *span,
                });
            }
            Element::Component {
                name,
                attributes,
                content,
                span,
            } => {
                resolved.push(Element::Component {
                    name: name.clone(),
                    attributes: attributes.clone(),
                    content: resolve_slots(content, slot_map, default_content),
                    span: *span,
                });
            }
            Element::For {
                item,
                index,
                iterable,
                key,
                content,
                span,
            } => {
                resolved.push(Element::For {
                    item: item.clone(),
                    index: index.clone(),
                    iterable: iterable.clone(),
                    key: key.clone(),
                    content: resolve_slots(content, slot_map, default_content),
                    span: *span,
                });
            }
            Element::If {
                condition,
                then_branch,
                else_branch,
                span,
            } => {
                resolved.push(Element::If {
                    condition: condition.clone(),
                    then_branch: resolve_slots(then_branch, slot_map, default_content),
                    else_branch: else_branch
                        .as_ref()
                        .map(|eb| resolve_slots(eb, slot_map, default_content)),
                    span: *span,
                });
            }
            Element::ErrorBlock {
                field,
                content,
                span,
            } => {
                resolved.push(Element::ErrorBlock {
                    field: field.clone(),
                    content: resolve_slots(content, slot_map, default_content),
                    span: *span,
                });
            }
            _ => resolved.push(element.clone()),
        }
    }
    resolved
}

/// Split page content into named slot provisions and default (unnamed) content.
fn separate_slot_content(
    page_content: &[Element],
) -> (
    std::collections::HashMap<String, Vec<Element>>,
    Vec<Element>,
) {
    let mut named: std::collections::HashMap<String, Vec<Element>> =
        std::collections::HashMap::new();
    let mut default_content = Vec::new();

    for elem in page_content {
        if let Element::SlotContent { name, content, .. } = elem {
            named.insert(name.clone(), content.clone());
        } else {
            default_content.push(elem.clone());
        }
    }

    (named, default_content)
}

fn generate_layout_with_page_and_components(
    layout: &Layout,
    page: &Page,
    document: &WebCoreDocument,
    project_root: Option<&Path>,
) -> Result<(String, Vec<HandlerMapping>), CompileError> {
    let prefix = safe_id_prefix(&page.name);
    let (named_slots, default_content) = separate_slot_content(&page.content);
    let resolved = resolve_slots(&layout.content, &named_slots, &default_content);
    let mut counter = 0usize;
    generate_elements_with_scope_and_counter(
        &resolved,
        document,
        None,
        &mut counter,
        &prefix,
        project_root,
    )
}

fn generate_elements_with_scope_and_counter(
    elements: &[Element],
    document: &WebCoreDocument,
    scope_id: Option<&str>,
    counter: &mut usize,
    prefix: &str,
    project_root: Option<&Path>,
) -> Result<(String, Vec<HandlerMapping>), CompileError> {
    let mut result = String::new();
    let mut all_handlers = Vec::new();

    for element in elements {
        let (element_html, handlers) = generate_element_with_scope(
            element,
            document,
            counter,
            scope_id,
            prefix,
            project_root,
        )?;
        result.push_str(&element_html);
        result.push('\n');
        all_handlers.extend(handlers);
    }
    Ok((result, all_handlers))
}

/// Expand `bind:attr={expr}` into a value/checked attr + event handler pair.
/// `bind:value={x}`   → `value={x}` + `on:input={x = event.target.value}`
/// `bind:checked={x}` → `checked={x}` + `on:change={x = event.target.checked}`
fn expand_bind_attrs(attributes: &[Attribute]) -> Vec<Attribute> {
    if !attributes.iter().any(|a| a.name.starts_with("bind:")) {
        return attributes.to_vec();
    }
    let mut result = Vec::with_capacity(attributes.len() + 2);
    for attr in attributes {
        if let Some(target) = attr.name.strip_prefix("bind:") {
            if let AttributeValue::Expression(expr) = &attr.value {
                result.push(Attribute {
                    name: target.to_string(),
                    value: AttributeValue::Expression(expr.clone()),
                    span: attr.span,
                });
                let (event, prop) = if target == "checked" || target == "selected" {
                    ("on:change", "event.target.checked")
                } else {
                    ("on:input", "event.target.value")
                };
                result.push(Attribute {
                    name: event.to_string(),
                    value: AttributeValue::Expression(format!("{} = {}", expr.trim(), prop)),
                    span: attr.span,
                });
            }
        } else {
            result.push(attr.clone());
        }
    }
    result
}

// ── Attribute sub-handlers ────────────────────────────────────────────────────

/// `ref:name=true` → `Some(" data-webcore-ref=\"name\"")`; returns `None` for other attrs.
fn handle_ref_attr(attr_name: &str) -> Option<String> {
    let ref_name = attr_name.strip_prefix("ref:")?;
    Some(format!(
        " {}=\"{}\"",
        attr_names::REF,
        html_escape(ref_name)
    ))
}

/// `class:name={expr}` → `Some(" data-webcore-class-name=\"expr\"")`; returns `None` otherwise.
fn handle_class_binding(attr_name: &str, expr: &str) -> Option<String> {
    let class_name = attr_name.strip_prefix("class:")?;
    Some(format!(
        " {}{}=\"{}\"",
        attr_names::CLASS_PREFIX,
        class_name,
        html_escape(expr)
    ))
}

/// `style:prop={expr}` → `Some(" data-webcore-style-prop=\"expr\"")`; returns `None` otherwise.
fn handle_style_binding(attr_name: &str, expr: &str) -> Option<String> {
    let prop_name = attr_name.strip_prefix("style:")?;
    Some(format!(
        " {}{}=\"{}\"",
        attr_names::STYLE_PREFIX,
        prop_name,
        html_escape(expr)
    ))
}

/// Emit HTML for a validate:* attribute.
/// Returns the one or two data-* attribute strings, or `None` if not a validate: attr.
fn handle_validation_attr(attr: &Attribute) -> Option<String> {
    let validator = attr.name.strip_prefix("validate:")?;
    let mut out = String::new();
    match &attr.value {
        AttributeValue::String(v) => match validator {
            "minlength" | "maxlength" => {
                let (constraint, msg) = v.split_once(',').unwrap_or((v.as_str(), ""));
                write!(
                    out,
                    " data-webcore-validate-{}=\"{}\"",
                    validator,
                    html_escape(constraint.trim())
                )
                .unwrap();
                if !msg.is_empty() {
                    write!(
                        out,
                        " data-webcore-validate-{}-msg=\"{}\"",
                        validator,
                        html_escape(msg.trim())
                    )
                    .unwrap();
                }
            }
            "pattern" => {
                let (pat, msg) = v.split_once(',').unwrap_or((v.as_str(), ""));
                let pat = pat.trim();
                write!(
                    out,
                    " data-webcore-validate-pattern=\"{}\"",
                    html_escape(pat)
                )
                .unwrap();
                if !msg.is_empty() {
                    write!(
                        out,
                        " data-webcore-validate-pattern-msg=\"{}\"",
                        html_escape(msg.trim())
                    )
                    .unwrap();
                }
                // Compile-time ReDoS warning: nested quantifiers can cause catastrophic backtracking in browsers
                if pat.contains(")+") || pat.contains(")*") || pat.contains(")+?") {
                    eprintln!("warning[security]: validate:pattern=\"{pat}\" may contain nested quantifiers — potential ReDoS in browser");
                }
            }
            _ => {
                write!(
                    out,
                    " data-webcore-validate-{}=\"{}\"",
                    validator,
                    html_escape(v)
                )
                .unwrap();
            }
        },
        AttributeValue::Boolean(true) => {
            write!(out, " data-webcore-validate-{validator}=\"\"").unwrap();
        }
        _ => {}
    }
    Some(out)
}

/// Returns the HTML attribute string for an `on:event={expr}` attribute, and (via out-params)
/// the handler mapping to register and the resolved href for link navigation.
/// Returns `None` when `attr_name` does not start with `on:`.
#[allow(clippy::too_many_arguments)]
fn handle_event_attr(
    attr_name: &str,
    expr: &str,
    is_link: bool,
    prefix: &str,
    counter: &mut usize,
    handlers: &mut Vec<HandlerMapping>,
    resolved_href: &mut Option<String>,
) -> Option<String> {
    if !attr_name.starts_with("on:") {
        return None;
    }
    let raw_event_type = attr_name.strip_prefix("on:").unwrap_or("click");
    let (event_type, debounce_ms) = if let Some(pos) = raw_event_type.find("|debounce") {
        (&raw_event_type[..pos], Some(300u32))
    } else {
        (raw_event_type, None)
    };

    *counter += 1;
    let handler_id = format!("{prefix}btn{counter}");

    // Extract href from webcore_navigate() for links
    if is_link && expr.contains("webcore_navigate") {
        if let Some(path) = extract_navigate_path(expr) {
            *resolved_href = Some(path);
        }
    }

    let mapped_event_type = if let Some(ms) = debounce_ms {
        format!("{event_type}|debounce={ms}")
    } else {
        event_type.to_string()
    };
    handlers.push(HandlerMapping {
        id: handler_id.clone(),
        event_type: mapped_event_type,
        expression: expr.to_string(),
    });

    let html_attr = if debounce_ms.is_some() {
        format!(" id=\"{handler_id}\"")
    } else {
        match event_type {
            "click" => format!(" id=\"{handler_id}\" onclick=\"webcore_handle_click('{handler_id}'); return false;\""),
            "submit" => format!(" id=\"{handler_id}\" onsubmit=\"webcore_handle_submit('{handler_id}'); return false;\""),
            "change" => format!(" id=\"{handler_id}\" onchange=\"webcore_handle_change('{handler_id}')\""),
            "input" => format!(" id=\"{handler_id}\" oninput=\"webcore_handle_input('{handler_id}')\""),
            _ => format!(" id=\"{handler_id}\" on{event_type}=\"webcore_handle_event('{event_type}', '{handler_id}')\""),
        }
    };
    Some(html_attr)
}

/// Generate HTML for a single `<tag>` element, including:
/// - CSS scope attribute (`data-v`)
/// - Static, boolean, and expression attributes
/// - Event handler attributes (`on:click` → `onclick="webcore_handle_click(...)"`)
/// - Validation data attributes (`validate:required` → `data-webcore-validate-required`)
/// - SPA-aware `<a href>` with `onclick="webcore_navigate(...)"` for internal links
#[allow(clippy::too_many_arguments)]
fn generate_tag_element(
    name: &str,
    attributes: &[Attribute],
    content: &[Element],
    document: &WebCoreDocument,
    counter: &mut usize,
    scope_id: Option<&str>,
    prefix: &str,
    project_root: Option<&Path>,
) -> Result<(String, Vec<HandlerMapping>), CompileError> {
    let mut result = String::new();
    let mut handlers = Vec::new();
    if name == "text" {
        let (content_html, content_handlers) = generate_elements_with_scope_and_counter(
            content,
            document,
            scope_id,
            counter,
            prefix,
            project_root,
        )?;
        return Ok((content_html, content_handlers));
    }

    // Expand bind:attr={expr} → attr={expr} + on:event={expr = event.target.value}
    let expanded = expand_bind_attrs(attributes);
    let attributes = &expanded;

    let mapped_name = if name == "link" { "a" } else { name };
    let is_link = mapped_name == "a";
    let mut resolved_href: Option<String> = None;
    write!(result, "<{mapped_name}").unwrap();

    // Add scope attribute for CSS scoping
    if let Some(sid) = scope_id {
        write!(result, " {}=\"{}\"", attr_names::SCOPE, sid).unwrap();
    }

    // Mark elements that have dynamic (expression) attribute bindings via data-webcore-attr-*
    // (class: bindings use data-webcore-class-* and are NOT marked with data-webcore-bound)
    if attributes.iter().any(|a| {
        !a.name.starts_with("on:")
            && !a.name.starts_with("class:")
            && matches!(&a.value, AttributeValue::Expression(_))
    }) {
        write!(result, " {}", attr_names::BOUND).unwrap();
    }

    // Detect validate:* attributes and add data-webcore-field
    let has_validate = attributes.iter().any(|a| a.name.starts_with("validate:"));
    if has_validate {
        let field_name = attributes.iter().find(|a| a.name == "name").and_then(|a| {
            if let AttributeValue::String(v) = &a.value {
                Some(v.clone())
            } else {
                None
            }
        });
        if let Some(field) = field_name {
            write!(result, " data-webcore-field=\"{}\"", html_escape(&field)).unwrap();
        }
    }

    // Single-pass attribute scan — avoids multiple O(n) passes over the attribute list.
    struct TagScan {
        has_webc_img: bool,
        has_alt: bool,
        has_loading: bool,
        has_decoding: bool,
        src_value: Option<String>,
    }
    let scan = {
        let mut s = TagScan {
            has_webc_img: false,
            has_alt: false,
            has_loading: false,
            has_decoding: false,
            src_value: None,
        };
        for a in attributes {
            match a.name.as_str() {
                "webc:img" => s.has_webc_img = true,
                "alt" => s.has_alt = true,
                "loading" => s.has_loading = true,
                "decoding" => s.has_decoding = true,
                "src" => {
                    if let AttributeValue::String(v) = &a.value {
                        s.src_value = Some(v.clone());
                    }
                }
                _ => {}
            }
        }
        s
    };

    // webc:img — smart image defaults (Feature A)
    // Detect on the *original* tag name (before link→a mapping) AND mapped name
    let is_img = name == "img";
    let has_webc_img = is_img && scan.has_webc_img;
    if has_webc_img {
        // Emit a11y warning if alt is absent
        if !scan.has_alt {
            eprintln!("warning[a11y]: <img> with webc:img is missing alt attribute");
        }
        // Inject loading="lazy" if not already present
        if !scan.has_loading {
            result.push_str(" loading=\"lazy\"");
        }
        // Inject decoding="async" if not already present
        if !scan.has_decoding {
            result.push_str(" decoding=\"async\"");
        }
        // Read image dimensions at compile time
        if let Some(root) = project_root {
            if let Some(src) = &scan.src_value {
                let rel = src.trim_start_matches('/');
                let img_path = root.join("public").join(rel);
                if img_path.exists() {
                    if let Ok(sz) = imagesize::size(&img_path) {
                        write!(result, " width=\"{}\" height=\"{}\"", sz.width, sz.height).unwrap();
                    }
                }
            }
        }
    }

    // Generate attributes
    for attr in attributes {
        // Skip validate:* here — converted below after the loop
        if attr.name.starts_with("validate:") {
            continue;
        }
        // Skip webc:img — it's a compiler directive, not a real HTML attribute
        if attr.name == "webc:img" {
            continue;
        }
        // Skip on:event|debounce="N" modifier-only attributes (just a delay hint, no handler)
        if attr.name.starts_with("on:") && attr.name.contains("|debounce") {
            if let AttributeValue::String(_) = &attr.value {
                // This is purely a delay specifier — skip it (handled in JS codegen)
                continue;
            }
        }
        // ref:name=true → data-webcore-ref="name"
        if attr.name.starts_with("ref:") {
            if let Some(s) = handle_ref_attr(&attr.name) {
                result.push_str(&s);
            }
            continue;
        }
        // webc:transition="name" → data-webcore-transition="name"
        if attr.name == "webc:transition" {
            if let AttributeValue::String(value) = &attr.value {
                write!(
                    result,
                    " {}=\"{}\"",
                    attr_names::TRANSITION,
                    html_escape(value)
                )
                .unwrap();
            }
            continue;
        }
        // style:prop={expr} → data-webcore-style-prop="expr"
        if attr.name.starts_with("style:") {
            if let AttributeValue::Expression(expr) = &attr.value {
                if let Some(s) = handle_style_binding(&attr.name, expr) {
                    result.push_str(&s);
                }
            }
            continue;
        }
        match &attr.value {
            AttributeValue::String(value) => {
                if is_link && attr.name == "to" {
                    resolved_href = Some(value.clone());
                } else {
                    write!(result, " {}=\"{}\"", attr.name, html_escape(value)).unwrap();
                }
            }
            AttributeValue::Boolean(true) => {
                write!(result, " {}", attr.name).unwrap();
            }
            AttributeValue::Boolean(false) => {}
            AttributeValue::Expression(expr) => {
                if attr.name.starts_with("class:") {
                    // Conditional class binding: class:name={expr} → data-webcore-class-name="expr"
                    if let Some(s) = handle_class_binding(&attr.name, expr) {
                        result.push_str(&s);
                    }
                } else if attr.name.starts_with("on:") {
                    // Event handler: on:click={ count += 1 }
                    if let Some(s) = handle_event_attr(
                        &attr.name,
                        expr,
                        is_link,
                        prefix,
                        counter,
                        &mut handlers,
                        &mut resolved_href,
                    ) {
                        result.push_str(&s);
                    }
                } else {
                    // Dynamic attribute: bound at runtime via bindAttrs()
                    write!(
                        result,
                        " data-webcore-attr-{}=\"{}\"",
                        attr.name,
                        html_escape(expr)
                    )
                    .unwrap();
                }
            }
        }
    }

    // Emit validate:* attrs as data-webcore-validate-* attributes
    for attr in attributes
        .iter()
        .filter(|a| a.name.starts_with("validate:"))
    {
        if let Some(s) = handle_validation_attr(attr) {
            result.push_str(&s);
        }
    }

    if is_link {
        if let Some(h) = resolved_href {
            let has_nav = document.app.as_ref().is_some_and(|a| !a.routes.is_empty());
            // Internal paths get an onclick for SPA navigation so clicking
            // never triggers a full page reload.  href is kept as fallback.
            if has_nav && h.starts_with('/') {
                let js_safe_h = h.replace('\\', "\\\\").replace('\'', "\\'");
                write!(
                    result,
                    " href=\"{}\" onclick=\"webcore_navigate('{}'); return false;\"",
                    html_escape(&h),
                    js_safe_h
                )
                .unwrap();
            } else {
                write!(result, " href=\"{}\"", html_escape(&h)).unwrap();
            }
        } else if !attributes.iter().any(|a| a.name == "href") {
            result.push_str(" href=\"#\"");
        }
    }

    result.push('>');
    let (content_html, content_handlers) = generate_elements_with_scope_and_counter(
        content,
        document,
        scope_id,
        counter,
        prefix,
        project_root,
    )?;
    result.push_str(&content_html);
    write!(result, "</{mapped_name}>").unwrap();
    handlers.extend(content_handlers);
    Ok((result, handlers))
}

/// Render a component call site: resolve the component definition, substitute props,
/// apply the component's CSS scope, and recursively generate the view.
/// Falls back to rendering as a plain HTML element if the component is unknown.
#[allow(clippy::too_many_arguments)]
fn generate_component_element(
    name: &str,
    attributes: &[Attribute],
    content: &[Element],
    document: &WebCoreDocument,
    counter: &mut usize,
    scope_id: Option<&str>,
    prefix: &str,
    project_root: Option<&Path>,
) -> Result<(String, Vec<HandlerMapping>), CompileError> {
    // Find the component definition
    if let Some(component) = document.components.get(name) {
        // Validate props — warn on unknown prop names
        if !component.props.is_empty() {
            let prop_names: std::collections::HashSet<&str> =
                component.props.iter().map(|p| p.name.as_str()).collect();
            for attr in attributes {
                // Skip directive/event attributes
                if attr.name.starts_with("on:")
                    || attr.name.starts_with("class:")
                    || attr.name.starts_with("style:")
                    || attr.name.starts_with("ref:")
                    || attr.name.starts_with("webc:")
                    || attr.name.starts_with("bind:")
                {
                    continue;
                }
                if !prop_names.contains(attr.name.as_str()) {
                    eprintln!(
                        "warning[props]: component '{}' received unknown prop '{}'",
                        name, attr.name
                    );
                }
            }
        }

        // Collect static prop values
        let static_props: std::collections::HashMap<String, String> = attributes
            .iter()
            .filter_map(|a| {
                if let AttributeValue::String(v) = &a.value {
                    Some((a.name.clone(), v.clone()))
                } else {
                    None
                }
            })
            .collect();

        // Collect dynamic (expression) prop values — reactive props
        let dynamic_props: std::collections::HashMap<String, String> = attributes
            .iter()
            .filter_map(|a| {
                if let AttributeValue::Expression(e) = &a.value {
                    Some((a.name.clone(), e.clone()))
                } else {
                    None
                }
            })
            .collect();

        // Generate scope ID for this component's CSS — only if the component has styles.
        // Unstyled components do not need data-v attributes, which reduces HTML weight.
        let component_scope_str = if component.style.is_empty() {
            None
        } else {
            Some(generate_scope_id(name))
        };

        // Substitute props into the view
        let substituted;
        let view: &[Element] = if static_props.is_empty() && dynamic_props.is_empty() {
            &component.view
        } else {
            substituted = substitute_props(&component.view, &static_props, &dynamic_props);
            &substituted
        };

        generate_elements_with_scope_and_counter(
            view,
            document,
            component_scope_str.as_deref(),
            counter,
            prefix,
            project_root,
        )
    } else {
        // Component not found, generate as HTML element
        let mut result = String::new();
        write!(result, "<{name}").unwrap();

        // Add scope if we have one
        if let Some(sid) = scope_id {
            write!(result, " {}=\"{}\"", attr_names::SCOPE, sid).unwrap();
        }

        // Mark elements that have dynamic attribute bindings
        if attributes
            .iter()
            .any(|a| matches!(&a.value, AttributeValue::Expression(_)))
        {
            write!(result, " {}", attr_names::BOUND).unwrap();
        }

        // Generate attributes
        for attr in attributes {
            match &attr.value {
                AttributeValue::String(value) => {
                    write!(result, " {}=\"{}\"", attr.name, html_escape(value)).unwrap();
                }
                AttributeValue::Boolean(true) => {
                    write!(result, " {}", attr.name).unwrap();
                }
                AttributeValue::Boolean(false) => {}
                AttributeValue::Expression(expr) => {
                    write!(
                        result,
                        " data-webcore-attr-{}=\"{}\"",
                        attr.name,
                        html_escape(expr)
                    )
                    .unwrap();
                }
            }
        }

        result.push('>');
        let (content_html, content_handlers) = generate_elements_with_scope_and_counter(
            content,
            document,
            scope_id,
            counter,
            prefix,
            project_root,
        )?;
        result.push_str(&content_html);
        write!(result, "</{name}>").unwrap();
        Ok((result, content_handlers))
    }
}

fn generate_element_with_scope(
    element: &Element,
    document: &WebCoreDocument,
    counter: &mut usize,
    scope_id: Option<&str>,
    prefix: &str,
    project_root: Option<&Path>,
) -> Result<(String, Vec<HandlerMapping>), CompileError> {
    match element {
        Element::Text(text, _span) => Ok((html_escape(text), Vec::new())),
        Element::Tag {
            name,
            attributes,
            content,
            ..
        } => generate_tag_element(
            name,
            attributes,
            content,
            document,
            counter,
            scope_id,
            prefix,
            project_root,
        ),
        Element::Slot(name, _span) => Ok((format!("<!-- Slot: {name} -->"), Vec::new())),
        Element::SlotContent { content, .. } => {
            // SlotContent consumed by slot matching; render children as fallback
            generate_elements_with_scope_and_counter(
                content,
                document,
                scope_id,
                counter,
                prefix,
                project_root,
            )
        }
        Element::Component {
            name,
            attributes,
            content,
            ..
        } => generate_component_element(
            name,
            attributes,
            content,
            document,
            counter,
            scope_id,
            prefix,
            project_root,
        ),
        Element::Interpolation(expr, _span) => Ok((
            format!(
                "<span {}=\"{}\"></span>",
                attr_names::INTERPOLATION,
                html_escape(expr)
            ),
            Vec::new(),
        )),
        Element::ErrorBlock { field, content, .. } => {
            let mut html = format!(
                "<div {}=\"{}\" style=\"display:none\">\n",
                attr_names::ERROR,
                html_escape(field)
            );
            let (content_html, handlers) = generate_elements_with_scope_and_counter(
                content,
                document,
                scope_id,
                counter,
                prefix,
                project_root,
            )?;
            html.push_str(&content_html);
            html.push_str("</div>\n");
            Ok((html, handlers))
        }
        Element::For {
            item,
            index,
            iterable,
            key,
            content,
            ..
        } => render_for_element(
            item,
            index.as_deref(),
            iterable,
            key.as_deref(),
            content,
            document,
            counter,
            scope_id,
            prefix,
            project_root,
        ),
        Element::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => render_if_element(
            condition,
            then_branch,
            else_branch.as_deref(),
            document,
            counter,
            scope_id,
            prefix,
            project_root,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn render_for_element(
    item: &str,
    index: Option<&str>,
    iterable: &str,
    key: Option<&str>,
    content: &[Element],
    document: &WebCoreDocument,
    counter: &mut usize,
    scope_id: Option<&str>,
    prefix: &str,
    project_root: Option<&Path>,
) -> Result<(String, Vec<HandlerMapping>), CompileError> {
    // Detect range syntax: "0..5" or "N..M"
    let is_range = {
        let parts: Vec<&str> = iterable.splitn(2, "..").collect();
        parts.len() == 2
            && parts[0].trim().parse::<i64>().is_ok()
            && parts[1].trim().parse::<i64>().is_ok()
    };

    let mut open = format!(
        "<template {}=\"{}\" {}=\"{}\"",
        attr_names::FOR,
        item,
        attr_names::FOR_IN,
        iterable
    );
    if is_range {
        write!(
            open,
            " {}=\"{}\"",
            attr_names::FOR_RANGE,
            html_escape(iterable)
        )
        .unwrap();
    }
    if let Some(idx) = index {
        write!(open, " {}=\"{}\"", attr_names::FOR_INDEX, html_escape(idx)).unwrap();
    }
    if let Some(k) = key {
        write!(open, " {}=\"{}\"", attr_names::FOR_KEY, html_escape(k)).unwrap();
    }
    if let Some(sid) = scope_id {
        write!(open, " {}=\"{}\"", attr_names::SCOPE, sid).unwrap();
    }
    open.push('>');
    let (content_html, handlers) = generate_elements_with_scope_and_counter(
        content,
        document,
        scope_id,
        counter,
        prefix,
        project_root,
    )?;
    let result = format!(
        "{}\n{}</template>\n<div {}=\"{}\"></div>",
        open,
        content_html,
        attr_names::FOR_CONTAINER,
        iterable
    );
    Ok((result, handlers))
}

#[allow(clippy::too_many_arguments)]
fn render_if_element(
    condition: &str,
    then_branch: &[Element],
    else_branch: Option<&[Element]>,
    document: &WebCoreDocument,
    counter: &mut usize,
    scope_id: Option<&str>,
    prefix: &str,
    project_root: Option<&Path>,
) -> Result<(String, Vec<HandlerMapping>), CompileError> {
    let scope_attr = scope_id.map_or(String::new(), |sid| {
        format!(" {}=\"{}\"", attr_names::SCOPE, sid)
    });
    let mut result = format!(
        "<div {}=\"{}\"{}>\n",
        attr_names::IF,
        html_escape(condition),
        scope_attr
    );
    let mut all_handlers = Vec::new();

    let (then_html, then_handlers) = generate_elements_with_scope_and_counter(
        then_branch,
        document,
        scope_id,
        counter,
        prefix,
        project_root,
    )?;
    result.push_str(&then_html);
    result.push_str("</div>\n");
    all_handlers.extend(then_handlers);

    if let Some(else_content) = else_branch {
        writeln!(
            result,
            "<div {}=\"{}\"{}>",
            attr_names::IF_ELSE,
            html_escape(condition),
            scope_attr
        )
        .unwrap();
        let (else_html, else_handlers) = generate_elements_with_scope_and_counter(
            else_content,
            document,
            scope_id,
            counter,
            prefix,
            project_root,
        )?;
        result.push_str(&else_html);
        result.push_str("</div>\n");
        all_handlers.extend(else_handlers);
    }
    Ok((result, all_handlers))
}

fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

/// Substitute prop values into a component view before rendering.
///
/// Static props: `Interpolation(propName)` → `Text(value)`.
/// Dynamic props: `Interpolation(propName)` → `Interpolation(expr)` (stays reactive).
/// Word-boundary-aware identifier substitution within an expression string.
/// Replaces `name` with `replacement` only when it is not adjacent to `[a-zA-Z0-9_$]`.
fn replace_identifier(src: &str, name: &str, replacement: &str) -> String {
    let mut result = String::new();
    let mut remaining = src;
    loop {
        match remaining.find(name) {
            None => {
                result.push_str(remaining);
                break;
            }
            Some(pos) => {
                let before_ok = pos == 0
                    || remaining[..pos]
                        .chars()
                        .last()
                        .is_none_or(|c| !c.is_alphanumeric() && c != '_' && c != '$');
                let after_ok = remaining[pos + name.len()..]
                    .chars()
                    .next()
                    .is_none_or(|c| !c.is_alphanumeric() && c != '_' && c != '$');
                if before_ok && after_ok {
                    result.push_str(&remaining[..pos]);
                    result.push_str(replacement);
                    remaining = &remaining[pos + name.len()..];
                } else {
                    result.push_str(&remaining[..=pos]);
                    remaining = &remaining[pos + 1..];
                }
            }
        }
    }
    result
}

/// Substitutes prop identifiers into an expression string using the combined prop map.
///
/// The combined map encodes both static and dynamic props in one pass:
/// `(true, value)` = static prop (resolves to a literal string),
/// `(false, expr)` = dynamic prop (stays as a reactive expression).
///
/// Returns `Some((true, literal))` for a direct static match,
/// `Some((false, expr))` for a dynamic/compound substitution,
/// or `None` if the expression is unchanged.
fn substitute_in_expr_combined(
    trimmed: &str,
    combined: &std::collections::HashMap<&str, (bool, &str)>,
) -> Option<(bool, String)> {
    // Direct exact match — cheapest path
    if let Some(&(is_static, val)) = combined.get(trimmed) {
        return Some((is_static, val.to_string()));
    }
    // Compound expression: scan once for all identifier replacements
    let mut result = trimmed.to_string();
    for (&prop, &(is_static, val)) in combined {
        let replacement = if is_static {
            format!("({val})")
        } else {
            val.to_string()
        };
        result = replace_identifier(&result, prop, &replacement);
    }
    if result == trimmed {
        None
    } else {
        Some((false, result))
    }
}

/// Apply prop substitution to attribute values using the combined prop map.
fn substitute_props_in_attrs_combined(
    attributes: &[Attribute],
    combined: &std::collections::HashMap<&str, (bool, &str)>,
) -> Vec<Attribute> {
    attributes
        .iter()
        .map(|attr| {
            let value = match &attr.value {
                AttributeValue::Expression(expr) => {
                    match substitute_in_expr_combined(expr.trim(), combined) {
                        Some((true, val)) => AttributeValue::String(val),
                        Some((false, e)) => AttributeValue::Expression(e),
                        None => attr.value.clone(),
                    }
                }
                other => other.clone(),
            };
            Attribute {
                name: attr.name.clone(),
                value,
                span: attr.span,
            }
        })
        .collect()
}

/// Build a combined prop map once and recurse using it.
/// Avoids two separate O(n_props) iterations per expression for every element.
fn substitute_props(
    elements: &[Element],
    static_props: &std::collections::HashMap<String, String>,
    dynamic_props: &std::collections::HashMap<String, String>,
) -> Vec<Element> {
    // Build combined map once for the entire subtree traversal.
    let combined: std::collections::HashMap<&str, (bool, &str)> = static_props
        .iter()
        .map(|(k, v)| (k.as_str(), (true, v.as_str())))
        .chain(
            dynamic_props
                .iter()
                .map(|(k, v)| (k.as_str(), (false, v.as_str()))),
        )
        .collect();
    elements
        .iter()
        .map(|e| substitute_props_elem_combined(e, &combined))
        .collect()
}

fn substitute_props_elem_combined(
    element: &Element,
    combined: &std::collections::HashMap<&str, (bool, &str)>,
) -> Element {
    match element {
        Element::Interpolation(expr, span) => {
            match substitute_in_expr_combined(expr.trim(), combined) {
                Some((true, val)) => Element::Text(val, *span),
                Some((false, new_expr)) => Element::Interpolation(new_expr, *span),
                None => element.clone(),
            }
        }
        Element::Tag {
            name,
            attributes,
            content,
            span,
        } => Element::Tag {
            name: name.clone(),
            attributes: substitute_props_in_attrs_combined(attributes, combined),
            content: elements_combined(content, combined),
            span: *span,
        },
        Element::Component {
            name,
            attributes,
            content,
            span,
        } => Element::Component {
            name: name.clone(),
            attributes: substitute_props_in_attrs_combined(attributes, combined),
            content: elements_combined(content, combined),
            span: *span,
        },
        Element::For {
            item,
            index,
            iterable,
            key,
            content,
            span,
        } => Element::For {
            item: item.clone(),
            index: index.clone(),
            iterable: iterable.clone(),
            key: key.clone(),
            content: elements_combined(content, combined),
            span: *span,
        },
        Element::If {
            condition,
            then_branch,
            else_branch,
            span,
        } => Element::If {
            condition: condition.clone(),
            then_branch: elements_combined(then_branch, combined),
            else_branch: else_branch
                .as_ref()
                .map(|eb| elements_combined(eb, combined)),
            span: *span,
        },
        Element::ErrorBlock {
            field,
            content,
            span,
        } => Element::ErrorBlock {
            field: field.clone(),
            content: elements_combined(content, combined),
            span: *span,
        },
        Element::SlotContent {
            name,
            content,
            span,
        } => Element::SlotContent {
            name: name.clone(),
            content: elements_combined(content, combined),
            span: *span,
        },
        _ => element.clone(),
    }
}

fn elements_combined(
    elements: &[Element],
    combined: &std::collections::HashMap<&str, (bool, &str)>,
) -> Vec<Element> {
    elements
        .iter()
        .map(|e| substitute_props_elem_combined(e, combined))
        .collect()
}

/// Derive a safe HTML-id-compatible prefix from a name (lowercase alphanumeric, max 12 chars).
fn safe_id_prefix(name: &str) -> String {
    let s: String = name
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .take(12)
        .collect();
    if s.is_empty() {
        "p".to_string()
    } else {
        s
    }
}

// Helper functions for SPA generation (test-only)
#[cfg(test)]
fn generate_layout_shell(
    layout: &Layout,
    document: &WebCoreDocument,
    project_root: Option<&Path>,
) -> Result<(String, Vec<HandlerMapping>), CompileError> {
    let mut result = String::new();
    let mut all_handlers = Vec::new();
    let mut counter = 0usize;

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
                    write!(result, "  <div id=\"webcore-slot-{slot_name}\">\n    <!-- Named slot: {slot_name} -->\n  </div>\n").unwrap();
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
                let (element_html, handlers) = generate_element_with_scope(
                    element,
                    document,
                    &mut counter,
                    None,
                    "ly",
                    project_root,
                )?;
                result.push_str(&element_html);
                all_handlers.extend(handlers);
            }
            _ => {
                let (element_html, handlers) = generate_element_with_scope(
                    element,
                    document,
                    &mut counter,
                    None,
                    "ly",
                    project_root,
                )?;
                result.push_str(&element_html);
                result.push('\n');
                all_handlers.extend(handlers);
            }
        }
    }

    Ok((result, all_handlers))
}

#[cfg(test)]
fn generate_page_content(
    page: &Page,
    document: &WebCoreDocument,
    project_root: Option<&Path>,
) -> Result<(String, Vec<HandlerMapping>), CompileError> {
    let prefix = safe_id_prefix(&page.name);
    let mut result = String::new();
    let mut all_handlers = Vec::new();
    let mut counter = 0usize;

    for element in &page.content {
        let (html, handlers) = generate_element_with_scope(
            element,
            document,
            &mut counter,
            None,
            &prefix,
            project_root,
        )?;
        result.push_str(&html);
        result.push('\n');
        all_handlers.extend(handlers);
    }

    Ok((result, all_handlers))
}

#[cfg(test)]
fn generate_component_content(
    component: &Component,
    document: &WebCoreDocument,
    project_root: Option<&Path>,
) -> Result<(String, Vec<HandlerMapping>), CompileError> {
    let scope_id = generate_scope_id(&component.name);
    let prefix = safe_id_prefix(&component.name);
    let mut result = String::new();
    let mut all_handlers = Vec::new();
    let mut counter = 0usize;

    for element in &component.view {
        let (html, handlers) = generate_element_with_scope(
            element,
            document,
            &mut counter,
            Some(&scope_id),
            &prefix,
            project_root,
        )?;
        result.push_str(&html);
        result.push('\n');
        all_handlers.extend(handlers);
    }

    Ok((result, all_handlers))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Span;

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
            locales: std::collections::HashMap::new(),
            default_locale: String::new(),
            wasm_module: None,
            layouts: std::collections::HashMap::new(),
            pages: std::collections::HashMap::new(),
            components: std::collections::HashMap::new(),
            imports: vec![],
            data_imports: std::collections::HashMap::new(),
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
            locales: std::collections::HashMap::new(),
            default_locale: String::new(),
            wasm_module: None,
            layouts: std::collections::HashMap::new(),
            pages: std::collections::HashMap::new(),
            components: std::collections::HashMap::new(),
            imports: vec![],
            data_imports: std::collections::HashMap::new(),
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
            locales: std::collections::HashMap::new(),
            default_locale: String::new(),
            wasm_module: None,
            layouts: std::collections::HashMap::new(),
            pages: std::collections::HashMap::new(),
            components: std::collections::HashMap::new(),
            imports: vec![],
            data_imports: std::collections::HashMap::new(),
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
        };
        let res = generate_html(&doc, "test", &opts).expect("html ok");
        assert!(res.html.contains("onfoo=\"webcore_handle_event('foo',"));
    }
}

// ── HTML minifier (prod mode) ────────────────────────────────────────────────

/// Strip HTML comments and collapse inter-tag whitespace (prod mode).
pub(crate) fn minify_html(html: &str) -> String {
    let no_comments = strip_html_comments(html);
    collapse_whitespace_between_tags(&no_comments)
}

fn strip_html_comments(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let bytes = html.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i..].starts_with(b"<!--") {
            if let Some(end) = html[i..].find("-->") {
                i += end + 3;
            } else {
                result.push_str(&html[i..]);
                break;
            }
        } else {
            result.push(bytes[i] as char);
            i += 1;
        }
    }
    result
}

fn collapse_whitespace_between_tags(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let bytes = html.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'>' {
            result.push('>');
            i += 1;
            // Skip whitespace-only runs between > and <
            let start = i;
            while i < bytes.len()
                && (bytes[i] == b' ' || bytes[i] == b'\n' || bytes[i] == b'\t' || bytes[i] == b'\r')
            {
                i += 1;
            }
            if i < bytes.len() && bytes[i] == b'<' {
                // pure whitespace between > and < — discard
            } else {
                // contains non-whitespace — keep original
                result.push_str(&html[start..i]);
            }
        } else {
            result.push(bytes[i] as char);
            i += 1;
        }
    }
    result
}
