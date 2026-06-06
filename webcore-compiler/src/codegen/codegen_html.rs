//! HTML Code Generator

use crate::ast::*;
use crate::codegen::codegen_css::generate_scope_id;

// Options passed from the build to influence the page shell
#[derive(Debug, Clone)]
pub struct HtmlPageOptions {
    pub lang: String,
    pub title: String,
    pub extra_css_files: Vec<String>,
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

/// Extract path from webcore_navigate(path) expression
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
                format!("/{}", path)
            };

            return Some(clean_path);
        }
    }
    None
}

fn find_layout(document: &WebCoreDocument) -> Result<&Layout, String> {
    // Prefer the layout declared in the app block, then fall back to MainLayout / default
    if let Some(name) = document.app.as_ref().and_then(|a| a.layout.as_deref()) {
        if let Some(layout) = document.layouts.get(name) {
            return Ok(layout);
        }
        return Err(format!(
            "No layout found: app declares '{}' but available layouts are: [{}]",
            name,
            document.layouts.keys().cloned().collect::<Vec<_>>().join(", ")
        ));
    }
    document
        .layouts
        .get("MainLayout")
        .or_else(|| document.layouts.get("default"))
        .ok_or_else(|| format!(
            "No layout found (tried MainLayout, default). Available: [{}]",
            document.layouts.keys().cloned().collect::<Vec<_>>().join(", ")
        ))
}

pub fn generate_hybrid_index(
    document: &WebCoreDocument,
    options: &HtmlPageOptions,
) -> Result<HtmlGenerationResult, String> {
    let layout = find_layout(document)?;

    // Generate HTML shell
    let mut html = String::new();
    html.push_str("<!DOCTYPE html>\n");
    html.push_str(&format!(
        "<html lang=\"{}\">\n<head>\n",
        html_escape(&options.lang)
    ));
    html.push_str("  <meta charset=\"UTF-8\">\n");
    html.push_str("  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n");
    html.push_str(&format!(
        "  <title>{}</title>\n",
        html_escape(&options.title)
    ));
    html.push_str("  <link rel=\"stylesheet\" href=\"/assets/theme.css\">\n");
    for css in &options.extra_css_files {
        html.push_str(&format!(
            "  <link rel=\"stylesheet\" href=\"/assets/{}\">\n",
            html_escape(css)
        ));
    }
    html.push_str("</head>\n<body>\n");

    // Generate layout shell (without page content, just the structure)
    let (layout_content, all_handlers) = generate_layout_shell(layout, document)?;
    html.push_str(&layout_content);

    // Content area will be added by the layout shell

    html.push_str("  <script src=\"/assets/webcore.js\"></script>\n");
    html.push_str("</body>\n</html>");

    Ok(HtmlGenerationResult {
        html,
        handlers: all_handlers,
    })
}

pub fn generate_spa_html(
    document: &WebCoreDocument,
    options: &HtmlPageOptions,
) -> Result<HtmlGenerationResult, String> {
    let layout = find_layout(document)?;

    // Generate HTML shell
    let mut html = String::new();
    html.push_str("<!DOCTYPE html>\n");
    html.push_str(&format!(
        "<html lang=\"{}\">\n<head>\n",
        html_escape(&options.lang)
    ));
    html.push_str("  <meta charset=\"UTF-8\">\n");
    html.push_str("  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n");
    html.push_str(&format!(
        "  <title>{}</title>\n",
        html_escape(&options.title)
    ));
    html.push_str("  <link rel=\"stylesheet\" href=\"/assets/theme.css\">\n");
    for css in &options.extra_css_files {
        html.push_str(&format!(
            "  <link rel=\"stylesheet\" href=\"/assets/{}\">\n",
            html_escape(css)
        ));
    }
    html.push_str("</head>\n<body>\n");

    // Generate layout shell (without page content, just the structure)
    let (layout_content, mut all_handlers) = generate_layout_shell(layout, document)?;
    html.push_str(&layout_content);

    // Add routing container
    html.push_str("  <div id=\"webcore-router\" style=\"display: none;\">\n");

    // Generate all pages as hidden divs
    for (page_name, page) in &document.pages {
        let (page_html, handlers) = generate_page_content(page, document)?;
        html.push_str(&format!(
            "    <div id=\"page-{}\" data-route=\"/{}\">\n",
            page_name.to_lowercase(),
            page_name.to_lowercase()
        ));
        html.push_str(&page_html);
        html.push_str("    </div>\n");
        all_handlers.extend(handlers);
    }

    // Generate all page components as hidden divs
    for (component_name, component) in &document.components {
        if component_name.ends_with("Page") {
            let (page_html, handlers) = generate_component_content(component, document)?;
            let route_name = component_name.replace("Page", "").to_lowercase();
            html.push_str(&format!(
                "    <div id=\"page-{}\" data-route=\"/{}\">\n",
                route_name, route_name
            ));
            html.push_str(&page_html);
            html.push_str("    </div>\n");
            all_handlers.extend(handlers);
        }
    }

    html.push_str("  </div>\n");

    html.push_str("  <script src=\"/assets/webcore.js\"></script>\n");
    html.push_str("</body>\n</html>");

    Ok(HtmlGenerationResult {
        html,
        handlers: all_handlers,
    })
}

pub fn generate_page_content_only(
    document: &WebCoreDocument,
    page_name: &str,
    options: &HtmlPageOptions,
) -> Result<HtmlGenerationResult, String> {
    // Find the page
    let page = document
        .pages
        .get(page_name)
        .ok_or_else(|| format!("Page '{}' not found", page_name))?;

    let layout = find_layout(document)?;

    // Generate HTML by combining layout and page content
    let mut html = String::new();
    html.push_str("<!DOCTYPE html>\n");
    html.push_str(&format!(
        "<html lang=\"{}\">\n<head>\n",
        html_escape(&options.lang)
    ));
    html.push_str("  <meta charset=\"UTF-8\">\n");
    html.push_str("  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n");
    html.push_str(&format!(
        "  <title>{}</title>\n",
        html_escape(&options.title)
    ));
    html.push_str("  <link rel=\"stylesheet\" href=\"/assets/theme.css\">\n");
    for css in &options.extra_css_files {
        html.push_str(&format!(
            "  <link rel=\"stylesheet\" href=\"/assets/{}\">\n",
            html_escape(css)
        ));
    }
    html.push_str("</head>\n<body>\n");

    // Generate layout content, replacing slots with page content
    let (layout_content, handlers) =
        generate_layout_with_page_and_components(layout, page, document)?;
    html.push_str(&layout_content);

    html.push_str("  <script src=\"/assets/webcore.js\"></script>\n");
    html.push_str("</body>\n</html>");

    Ok(HtmlGenerationResult { html, handlers })
}

pub fn generate_html(
    document: &WebCoreDocument,
    page_name: &str,
    options: &HtmlPageOptions,
) -> Result<HtmlGenerationResult, String> {
    // Find the page
    let page = document
        .pages
        .get(page_name)
        .ok_or_else(|| format!("Page '{}' not found", page_name))?;

    let layout = find_layout(document)?;

    // Generate HTML by combining layout and page content
    let mut html = String::new();
    html.push_str("<!DOCTYPE html>\n");
    html.push_str(&format!(
        "<html lang=\"{}\">\n<head>\n",
        html_escape(&options.lang)
    ));
    html.push_str("  <meta charset=\"UTF-8\">\n");
    html.push_str("  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n");
    html.push_str(&format!(
        "  <title>{}</title>\n",
        html_escape(&options.title)
    ));
    html.push_str("  <link rel=\"stylesheet\" href=\"/assets/theme.css\">\n");
    for css in &options.extra_css_files {
        html.push_str(&format!(
            "  <link rel=\"stylesheet\" href=\"/assets/{}\">\n",
            html_escape(css)
        ));
    }
    html.push_str("</head>\n<body>\n");

    // Generate layout content, replacing slots with page content
    let (layout_content, handlers) =
        generate_layout_with_page_and_components(layout, page, document)?;
    html.push_str(&layout_content);

    html.push_str("  <script src=\"/assets/webcore.js\"></script>\n");
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
                } else {
                    if let Some(content) = slot_map.get(name.as_str()) {
                        resolved.extend_from_slice(content);
                    }
                    // unnamed named slot with no provision → empty (no comment noise)
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
) -> Result<(String, Vec<HandlerMapping>), String> {
    let prefix = safe_id_prefix(&page.name);
    let (named_slots, default_content) = separate_slot_content(&page.content);
    let resolved = resolve_slots(&layout.content, &named_slots, &default_content);
    let mut counter = 0usize;
    generate_elements_with_scope_and_counter(&resolved, document, None, &mut counter, &prefix)
}

fn generate_elements_with_scope_and_counter(
    elements: &[Element],
    document: &WebCoreDocument,
    scope_id: Option<&str>,
    counter: &mut usize,
    prefix: &str,
) -> Result<(String, Vec<HandlerMapping>), String> {
    let mut result = String::new();
    let mut all_handlers = Vec::new();

    for element in elements {
        let (element_html, handlers) =
            generate_element_with_scope(element, document, counter, scope_id, prefix)?;
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
                    value: AttributeValue::Expression(
                        format!("{} = {}", expr.trim(), prop),
                    ),
                    span: attr.span,
                });
            }
        } else {
            result.push(attr.clone());
        }
    }
    result
}

/// Generate HTML for a single `<tag>` element, including:
/// - CSS scope attribute (`data-v`)
/// - Static, boolean, and expression attributes
/// - Event handler attributes (`on:click` → `onclick="webcore_handle_click(...)"`)
/// - Validation data attributes (`validate:required` → `data-webcore-validate-required`)
/// - SPA-aware `<a href>` with `onclick="webcore_navigate(...)"` for internal links
fn generate_tag_element(
    name: &str,
    attributes: &[Attribute],
    content: &[Element],
    document: &WebCoreDocument,
    counter: &mut usize,
    scope_id: Option<&str>,
    prefix: &str,
) -> Result<(String, Vec<HandlerMapping>), String> {
    let mut result = String::new();
    let mut handlers = Vec::new();
    if name == "text" {
        let (content_html, content_handlers) = generate_elements_with_scope_and_counter(
            content, document, scope_id, counter, prefix,
        )?;
        return Ok((content_html, content_handlers));
    }

    // Expand bind:attr={expr} → attr={expr} + on:event={expr = event.target.value}
    let expanded = expand_bind_attrs(attributes);
    let attributes = &expanded;

    let mapped_name = if name == "link" { "a" } else { name };
    let is_link = mapped_name == "a";
    let mut resolved_href: Option<String> = None;
    result.push_str(&format!("<{}", mapped_name));

    // Add scope attribute for CSS scoping
    if let Some(sid) = scope_id {
        result.push_str(&format!(" data-v=\"{}\"", sid));
    }

    // Mark elements that have dynamic (expression) attribute bindings
    if attributes.iter().any(|a| {
        !a.name.starts_with("on:") && matches!(&a.value, AttributeValue::Expression(_))
    }) {
        result.push_str(" data-webcore-bound");
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
            result.push_str(&format!(" data-webcore-field=\"{}\"", html_escape(&field)));
        }
    }

    // Generate attributes
    for attr in attributes {
        // Skip validate:* here — converted below after the loop
        if attr.name.starts_with("validate:") {
            continue;
        }
        match &attr.value {
            AttributeValue::String(value) => {
                if is_link && attr.name == "to" {
                    resolved_href = Some(value.clone());
                } else {
                    result.push_str(&format!(" {}=\"{}\"", attr.name, html_escape(value)));
                }
            }
            AttributeValue::Boolean(true) => {
                result.push_str(&format!(" {}", attr.name));
            }
            AttributeValue::Boolean(false) => {}
            AttributeValue::Expression(expr) => {
                if attr.name.starts_with("on:") {
                    // Event handler: on:click={ count += 1 }
                    let event_type = attr.name.strip_prefix("on:").unwrap_or("click");
                    *counter += 1;
                    let handler_id = format!("{}btn{}", prefix, counter);

                    // Extract href from webcore_navigate() for links
                    if is_link && expr.contains("webcore_navigate") {
                        if let Some(path) = extract_navigate_path(expr) {
                            resolved_href = Some(path);
                        }
                    }

                    // Add handler to our collection
                    handlers.push(HandlerMapping {
                        id: handler_id.clone(),
                        event_type: event_type.to_string(),
                        expression: expr.clone(),
                    });

                    // Use native HTML5 event attributes with simple IDs
                    match event_type {
                        "click" => result.push_str(&format!(" id=\"{}\" onclick=\"webcore_handle_click('{}'); return false;\"", handler_id, handler_id)),
                        "submit" => result.push_str(&format!(" id=\"{}\" onsubmit=\"webcore_handle_submit('{}'); return false;\"", handler_id, handler_id)),
                        "change" => result.push_str(&format!(" id=\"{}\" onchange=\"webcore_handle_change('{}')\"", handler_id, handler_id)),
                        "input" => result.push_str(&format!(" id=\"{}\" oninput=\"webcore_handle_input('{}')\"", handler_id, handler_id)),
                        _ => result.push_str(&format!(" id=\"{}\" on{}=\"webcore_handle_event('{}', '{}')\"", handler_id, event_type, event_type, handler_id)),
                    }
                } else {
                    // Dynamic attribute: bound at runtime via bindAttrs()
                    result.push_str(&format!(
                        " data-webcore-attr-{}=\"{}\"",
                        attr.name,
                        html_escape(expr)
                    ));
                }
            }
        }
    }

    // Emit validate:* attrs as data-webcore-validate-* attributes
    for attr in attributes
        .iter()
        .filter(|a| a.name.starts_with("validate:"))
    {
        let validator = attr.name.strip_prefix("validate:").unwrap_or("");
        match &attr.value {
            AttributeValue::String(v) => match validator {
                "minlength" | "maxlength" => {
                    let (constraint, msg) = v.split_once(',').unwrap_or((v.as_str(), ""));
                    result.push_str(&format!(
                        " data-webcore-validate-{}=\"{}\"",
                        validator,
                        html_escape(constraint.trim())
                    ));
                    if !msg.is_empty() {
                        result.push_str(&format!(
                            " data-webcore-validate-{}-msg=\"{}\"",
                            validator,
                            html_escape(msg.trim())
                        ));
                    }
                }
                "pattern" => {
                    let (pat, msg) = v.split_once(',').unwrap_or((v.as_str(), ""));
                    result.push_str(&format!(
                        " data-webcore-validate-pattern=\"{}\"",
                        html_escape(pat.trim())
                    ));
                    if !msg.is_empty() {
                        result.push_str(&format!(
                            " data-webcore-validate-pattern-msg=\"{}\"",
                            html_escape(msg.trim())
                        ));
                    }
                }
                _ => {
                    result.push_str(&format!(
                        " data-webcore-validate-{}=\"{}\"",
                        validator,
                        html_escape(v)
                    ));
                }
            },
            AttributeValue::Boolean(true) => {
                result.push_str(&format!(" data-webcore-validate-{}=\"\"", validator));
            }
            _ => {}
        }
    }

    if is_link {
        if let Some(h) = resolved_href {
            let has_nav = document
                .app
                .as_ref()
                .map(|a| !a.routes.is_empty())
                .unwrap_or(false);
            // Internal paths get an onclick for SPA navigation so clicking
            // never triggers a full page reload.  href is kept as fallback.
            if has_nav && h.starts_with('/') {
                result.push_str(&format!(
                    " href=\"{}\" onclick=\"webcore_navigate('{}'); return false;\"",
                    html_escape(&h),
                    h
                ));
            } else {
                result.push_str(&format!(" href=\"{}\"", html_escape(&h)));
            }
        } else if !attributes.iter().any(|a| a.name == "href") {
            result.push_str(" href=\"#\"");
        }
    }

    result.push('>');
    let (content_html, content_handlers) = generate_elements_with_scope_and_counter(
        content, document, scope_id, counter, prefix,
    )?;
    result.push_str(&content_html);
    result.push_str(&format!("</{}>", mapped_name));
    handlers.extend(content_handlers);
    Ok((result, handlers))
}

/// Render a component call site: resolve the component definition, substitute props,
/// apply the component's CSS scope, and recursively generate the view.
/// Falls back to rendering as a plain HTML element if the component is unknown.
fn generate_component_element(
    name: &str,
    attributes: &[Attribute],
    content: &[Element],
    document: &WebCoreDocument,
    counter: &mut usize,
    scope_id: Option<&str>,
    prefix: &str,
) -> Result<(String, Vec<HandlerMapping>), String> {
    // Find the component definition
    if let Some(component) = document.components.get(name) {
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

        // Generate scope ID for this component's CSS
        let component_scope = generate_scope_id(name);

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
            Some(&component_scope),
            counter,
            prefix,
        )
    } else {
        // Component not found, generate as HTML element
        let mut result = String::new();
        result.push_str(&format!("<{}", name));

        // Add scope if we have one
        if let Some(sid) = scope_id {
            result.push_str(&format!(" data-v=\"{}\"", sid));
        }

        // Mark elements that have dynamic attribute bindings
        if attributes
            .iter()
            .any(|a| matches!(&a.value, AttributeValue::Expression(_)))
        {
            result.push_str(" data-webcore-bound");
        }

        // Generate attributes
        for attr in attributes {
            match &attr.value {
                AttributeValue::String(value) => {
                    result.push_str(&format!(" {}=\"{}\"", attr.name, html_escape(value)));
                }
                AttributeValue::Boolean(true) => {
                    result.push_str(&format!(" {}", attr.name));
                }
                AttributeValue::Boolean(false) => {}
                AttributeValue::Expression(expr) => {
                    result.push_str(&format!(
                        " data-webcore-attr-{}=\"{}\"",
                        attr.name,
                        html_escape(expr)
                    ));
                }
            }
        }

        result.push('>');
        let (content_html, content_handlers) = generate_elements_with_scope_and_counter(
            content, document, scope_id, counter, prefix,
        )?;
        result.push_str(&content_html);
        result.push_str(&format!("</{}>", name));
        Ok((result, content_handlers))
    }
}

fn generate_element_with_scope(
    element: &Element,
    document: &WebCoreDocument,
    counter: &mut usize,
    scope_id: Option<&str>,
    prefix: &str,
) -> Result<(String, Vec<HandlerMapping>), String> {
    match element {
        Element::Text(text, _span) => Ok((html_escape(text), Vec::new())),
        Element::Tag {
            name,
            attributes,
            content,
            ..
        } =>
            generate_tag_element(name, attributes, content, document, counter, scope_id, prefix),
        Element::Slot(name, _span) => Ok((format!("<!-- Slot: {} -->", name), Vec::new())),
        Element::SlotContent { content, .. } => {
            // SlotContent consumed by slot matching; render children as fallback
            generate_elements_with_scope_and_counter(content, document, scope_id, counter, prefix)
        }
        Element::Component {
            name,
            attributes,
            content,
            ..
        } =>
            generate_component_element(name, attributes, content, document, counter, scope_id, prefix),
        Element::Interpolation(expr, _span) => Ok((
            format!(
                "<span data-webcore-interpolation=\"{}\"></span>",
                html_escape(expr)
            ),
            Vec::new(),
        )),
        Element::ErrorBlock { field, content, .. } => {
            let mut html = format!(
                "<div data-webcore-error=\"{}\" style=\"display:none\">\n",
                html_escape(field)
            );
            let (content_html, handlers) = generate_elements_with_scope_and_counter(
                content, document, scope_id, counter, prefix,
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
        } => {
            let mut result = String::new();
            let mut open = format!(
                "<template data-webcore-for=\"{}\" data-webcore-in=\"{}\"",
                item, iterable
            );
            if let Some(idx) = index {
                open.push_str(&format!(" data-webcore-for-index=\"{}\"", html_escape(idx)));
            }
            if let Some(k) = key {
                open.push_str(&format!(" data-webcore-for-key=\"{}\"", html_escape(k)));
            }
            if let Some(sid) = scope_id {
                open.push_str(&format!(" data-v=\"{}\"", sid));
            }
            open.push('>');
            result.push_str(&open);
            result.push('\n');
            let (content_html, handlers) = generate_elements_with_scope_and_counter(
                content, document, scope_id, counter, prefix,
            )?;
            result.push_str(&content_html);
            result.push_str("</template>\n");
            result.push_str(&format!(
                "<div data-webcore-for-container=\"{}\"></div>",
                iterable
            ));
            Ok((result, handlers))
        }
        Element::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            let mut result = String::new();
            let mut all_handlers = Vec::new();

            result.push_str(&format!(
                "<div data-webcore-if=\"{}\"",
                html_escape(condition)
            ));
            if let Some(sid) = scope_id {
                result.push_str(&format!(" data-v=\"{}\"", sid));
            }
            result.push_str(">\n");
            let (then_html, then_handlers) = generate_elements_with_scope_and_counter(
                then_branch,
                document,
                scope_id,
                counter,
                prefix,
            )?;
            result.push_str(&then_html);
            result.push_str("</div>\n");
            all_handlers.extend(then_handlers);

            if let Some(else_content) = else_branch {
                result.push_str(&format!(
                    "<div data-webcore-else=\"{}\"",
                    html_escape(condition)
                ));
                if let Some(sid) = scope_id {
                    result.push_str(&format!(" data-v=\"{}\"", sid));
                }
                result.push_str(">\n");
                let (else_html, else_handlers) = generate_elements_with_scope_and_counter(
                    else_content,
                    document,
                    scope_id,
                    counter,
                    prefix,
                )?;
                result.push_str(&else_html);
                result.push_str("</div>\n");
                all_handlers.extend(else_handlers);
            }
            Ok((result, all_handlers))
        }
    }
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
                    result.push_str(&remaining[..pos + 1]);
                    remaining = &remaining[pos + 1..];
                }
            }
        }
    }
    result
}

/// Apply prop substitution to attribute values.
fn substitute_props_in_attrs(
    attributes: &[Attribute],
    static_props: &std::collections::HashMap<String, String>,
    dynamic_props: &std::collections::HashMap<String, String>,
) -> Vec<Attribute> {
    attributes
        .iter()
        .map(|attr| {
            let value = match &attr.value {
                AttributeValue::Expression(expr) => {
                    let trimmed = expr.trim();
                    if let Some(val) = static_props.get(trimmed) {
                        AttributeValue::String(val.clone())
                    } else if let Some(dyn_expr) = dynamic_props.get(trimmed) {
                        AttributeValue::Expression(dyn_expr.clone())
                    } else {
                        let mut result = trimmed.to_string();
                        for (prop, val) in static_props {
                            result =
                                replace_identifier(&result, prop, &format!("({})", val));
                        }
                        for (prop, dyn_expr) in dynamic_props {
                            result = replace_identifier(&result, prop, dyn_expr);
                        }
                        AttributeValue::Expression(result)
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

fn substitute_props(
    elements: &[Element],
    static_props: &std::collections::HashMap<String, String>,
    dynamic_props: &std::collections::HashMap<String, String>,
) -> Vec<Element> {
    elements
        .iter()
        .map(|e| substitute_props_elem(e, static_props, dynamic_props))
        .collect()
}

fn substitute_props_elem(
    element: &Element,
    static_props: &std::collections::HashMap<String, String>,
    dynamic_props: &std::collections::HashMap<String, String>,
) -> Element {
    match element {
        Element::Interpolation(expr, span) => {
            let trimmed = expr.trim();
            // 1. Exact match on static prop → inline text
            if let Some(val) = static_props.get(trimmed) {
                return Element::Text(val.clone(), *span);
            }
            // 2. Exact match on dynamic prop → pass-through as reactive interpolation
            if let Some(dyn_expr) = dynamic_props.get(trimmed) {
                return Element::Interpolation(dyn_expr.clone(), *span);
            }
            // 3. Partial substitution in compound expressions: {step + 1}, {count + step}
            let mut result = trimmed.to_string();
            for (prop, val) in static_props {
                result = replace_identifier(&result, prop, &format!("({})", val));
            }
            for (prop, dyn_expr) in dynamic_props {
                result = replace_identifier(&result, prop, dyn_expr);
            }
            if result != trimmed {
                Element::Interpolation(result, *span)
            } else {
                element.clone()
            }
        }
        Element::Tag {
            name,
            attributes,
            content,
            span,
        } => Element::Tag {
            name: name.clone(),
            attributes: substitute_props_in_attrs(attributes, static_props, dynamic_props),
            content: substitute_props(content, static_props, dynamic_props),
            span: *span,
        },
        Element::Component {
            name,
            attributes,
            content,
            span,
        } => Element::Component {
            name: name.clone(),
            attributes: substitute_props_in_attrs(attributes, static_props, dynamic_props),
            content: substitute_props(content, static_props, dynamic_props),
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
            content: substitute_props(content, static_props, dynamic_props),
            span: *span,
        },
        Element::If {
            condition,
            then_branch,
            else_branch,
            span,
        } => Element::If {
            condition: condition.clone(),
            then_branch: substitute_props(then_branch, static_props, dynamic_props),
            else_branch: else_branch
                .as_ref()
                .map(|eb| substitute_props(eb, static_props, dynamic_props)),
            span: *span,
        },
        Element::ErrorBlock {
            field,
            content,
            span,
        } => Element::ErrorBlock {
            field: field.clone(),
            content: substitute_props(content, static_props, dynamic_props),
            span: *span,
        },
        Element::SlotContent {
            name,
            content,
            span,
        } => Element::SlotContent {
            name: name.clone(),
            content: substitute_props(content, static_props, dynamic_props),
            span: *span,
        },
        _ => element.clone(),
    }
}

/// Derive a safe HTML-id-compatible prefix from a name (lowercase alphanumeric, max 12 chars).
fn safe_id_prefix(name: &str) -> String {
    let s: String = name
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .take(12)
        .collect();
    if s.is_empty() {
        "p".to_string()
    } else {
        s
    }
}

// Helper functions for SPA generation
fn generate_layout_shell(
    layout: &Layout,
    document: &WebCoreDocument,
) -> Result<(String, Vec<HandlerMapping>), String> {
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
                    result.push_str(&format!(
                        "  <div id=\"webcore-slot-{}\">\n    <!-- Named slot: {} -->\n  </div>\n",
                        slot_name, slot_name
                    ));
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
                let (element_html, handlers) =
                    generate_element_with_scope(element, document, &mut counter, None, "ly")?;
                result.push_str(&element_html);
                all_handlers.extend(handlers);
            }
            _ => {
                let (element_html, handlers) =
                    generate_element_with_scope(element, document, &mut counter, None, "ly")?;
                result.push_str(&element_html);
                result.push('\n');
                all_handlers.extend(handlers);
            }
        }
    }

    Ok((result, all_handlers))
}

fn generate_page_content(
    page: &Page,
    document: &WebCoreDocument,
) -> Result<(String, Vec<HandlerMapping>), String> {
    let prefix = safe_id_prefix(&page.name);
    let mut result = String::new();
    let mut all_handlers = Vec::new();
    let mut counter = 0usize;

    for element in &page.content {
        let (html, handlers) =
            generate_element_with_scope(element, document, &mut counter, None, &prefix)?;
        result.push_str(&html);
        result.push('\n');
        all_handlers.extend(handlers);
    }

    Ok((result, all_handlers))
}

fn generate_component_content(
    component: &Component,
    document: &WebCoreDocument,
) -> Result<(String, Vec<HandlerMapping>), String> {
    let scope_id = generate_scope_id(&component.name);
    let prefix = safe_id_prefix(&component.name);
    let mut result = String::new();
    let mut all_handlers = Vec::new();
    let mut counter = 0usize;

    for element in &component.view {
        let (html, handlers) =
            generate_element_with_scope(element, document, &mut counter, Some(&scope_id), &prefix)?;
        result.push_str(&html);
        result.push('\n');
        all_handlers.extend(handlers);
    }

    Ok((result, all_handlers))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_button_page(page_name: &str) -> Page {
        Page {
            name: page_name.to_string(),
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
        };
        let res = generate_html(&doc, "test", &opts).expect("html ok");
        assert!(res.html.contains("onfoo=\"webcore_handle_event('foo',"));
    }
}
