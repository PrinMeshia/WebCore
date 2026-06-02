//! HTML Code Generator

use crate::ast::*;
use crate::codegen::codegen_css::generate_scope_id;

// Options passed from the build to influence the page shell
#[derive(Debug, Clone)]
pub struct HtmlPageOptions {
    pub lang: String,
    pub title: String,
}

#[derive(Debug, Clone)]
pub struct HandlerMapping {
    pub id: String,
    pub event_type: String,
    pub expression: String,
}

pub struct HtmlGenerationResult {
    pub html: String,
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

pub fn generate_hybrid_index(
    document: &WebCoreDocument,
    options: &HtmlPageOptions,
) -> Result<HtmlGenerationResult, String> {
    // Find the layout (try MainLayout first, then default)
    let layout = document
        .layouts
        .get("MainLayout")
        .or_else(|| document.layouts.get("default"))
        .ok_or_else(|| "No layout found (tried MainLayout and default)".to_string())?;

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
    html.push_str("  <link rel=\"stylesheet\" href=\"theme.css\">\n");
    html.push_str("</head>\n<body>\n");

    // Generate layout shell (without page content, just the structure)
    let (layout_content, all_handlers) = generate_layout_shell(layout, document)?;
    html.push_str(&layout_content);

    // Content area will be added by the layout shell

    html.push_str("  <script src=\"webcore.js\"></script>\n");
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
    // Find the layout (try MainLayout first, then default)
    let layout = document
        .layouts
        .get("MainLayout")
        .or_else(|| document.layouts.get("default"))
        .ok_or_else(|| "No layout found (tried MainLayout and default)".to_string())?;

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
    html.push_str("  <link rel=\"stylesheet\" href=\"theme.css\">\n");
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

    html.push_str("  <script src=\"webcore.js\"></script>\n");
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

    // Find the layout (try MainLayout first, then default)
    let layout = document
        .layouts
        .get("MainLayout")
        .or_else(|| document.layouts.get("default"))
        .ok_or_else(|| "No layout found (tried MainLayout and default)".to_string())?;

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
    html.push_str("  <link rel=\"stylesheet\" href=\"theme.css\">\n");
    html.push_str("</head>\n<body>\n");

    // Generate layout content, replacing slots with page content
    let (layout_content, handlers) =
        generate_layout_with_page_and_components(layout, page, document)?;
    html.push_str(&layout_content);

    html.push_str("  <script src=\"webcore.js\"></script>\n");
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

    // Find the layout (try MainLayout first, then default)
    let layout = document
        .layouts
        .get("MainLayout")
        .or_else(|| document.layouts.get("default"))
        .ok_or_else(|| "No layout found (tried MainLayout and default)".to_string())?;

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
    html.push_str("  <link rel=\"stylesheet\" href=\"theme.css\">\n");
    html.push_str("</head>\n<body>\n");

    // Generate layout content, replacing slots with page content
    let (layout_content, handlers) =
        generate_layout_with_page_and_components(layout, page, document)?;
    html.push_str(&layout_content);

    html.push_str("  <script src=\"webcore.js\"></script>\n");
    html.push_str("</body>\n</html>");

    Ok(HtmlGenerationResult { html, handlers })
}

fn generate_layout_with_page_and_components(
    layout: &Layout,
    page: &Page,
    document: &WebCoreDocument,
) -> Result<(String, Vec<HandlerMapping>), String> {
    let prefix = safe_id_prefix(&page.name);
    generate_elements_with_slot_replacement_and_components(
        &layout.content,
        &page.content,
        document,
        &prefix,
    )
}

fn generate_elements_with_slot_replacement_and_components(
    elements: &[Element],
    page_content: &[Element],
    document: &WebCoreDocument,
    prefix: &str,
) -> Result<(String, Vec<HandlerMapping>), String> {
    let mut result = String::new();
    let mut all_handlers = Vec::new();
    let mut counter = 0usize;

    for element in elements {
        match element {
            Element::Slot(slot_name, _span) => {
                if slot_name == "content" {
                    // Replace content slot with page content - use scope for page components
                    for page_elem in page_content {
                        let (html, handlers) = generate_element_with_scope(
                            page_elem,
                            document,
                            &mut counter,
                            None,
                            prefix,
                        )?;
                        result.push_str(&html);
                        result.push('\n');
                        all_handlers.extend(handlers);
                    }
                } else {
                    result.push_str(&format!("<!-- Slot: {} -->", slot_name));
                }
            }
            Element::Tag { name, content, .. } => {
                if name == "text" {
                    // Render only inner content
                    for inner in content {
                        let (html, handlers) = generate_element_with_scope(
                            inner,
                            document,
                            &mut counter,
                            None,
                            prefix,
                        )?;
                        result.push_str(&html);
                        all_handlers.extend(handlers);
                    }
                    continue;
                }

                // Check if this is a main tag with a slot inside
                if name == "main" && content.len() == 1 {
                    if let Element::Slot(slot_name, _) = &content[0] {
                        if slot_name == "content" {
                            // Replace main with slot with page content
                            result.push_str("<main>");
                            for page_elem in page_content {
                                let (html, handlers) = generate_element_with_scope(
                                    page_elem,
                                    document,
                                    &mut counter,
                                    None,
                                    prefix,
                                )?;
                                result.push_str(&html);
                                result.push('\n');
                                all_handlers.extend(handlers);
                            }
                            result.push_str("</main>");
                            continue;
                        }
                    }
                }

                // Use the proper element generation with handler support
                let (element_html, element_handlers) =
                    generate_element_with_scope(element, document, &mut counter, None, prefix)?;
                result.push_str(&element_html);
                all_handlers.extend(element_handlers);
            }
            Element::Component { name, .. } => {
                // Find the component definition
                if let Some(component) = document.components.get(name) {
                    // Replace component with its view content with scoping
                    let scope_id = generate_scope_id(name);
                    for view_elem in &component.view {
                        let (html, handlers) = generate_element_with_scope(
                            view_elem,
                            document,
                            &mut counter,
                            Some(&scope_id),
                            prefix,
                        )?;
                        result.push_str(&html);
                        result.push('\n');
                        all_handlers.extend(handlers);
                    }
                } else {
                    // Component not found, generate as custom element
                    let (element_html, element_handlers) =
                        generate_element_with_scope(element, document, &mut counter, None, prefix)?;
                    result.push_str(&element_html);
                    all_handlers.extend(element_handlers);
                }
            }
            _ => {
                let (element_html, element_handlers) =
                    generate_element_with_scope(element, document, &mut counter, None, prefix)?;
                result.push_str(&element_html);
                all_handlers.extend(element_handlers);
            }
        }
    }
    Ok((result, all_handlers))
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
        } => {
            let mut result = String::new();
            let mut handlers = Vec::new();
            if name == "text" {
                let (content_html, content_handlers) = generate_elements_with_scope_and_counter(
                    content, document, scope_id, counter, prefix,
                )?;
                return Ok((content_html, content_handlers));
            }

            let mapped_name = if name == "link" { "a" } else { name.as_str() };
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
                    result.push_str(&format!(" href=\"{}\"", html_escape(&h)));
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
        Element::Slot(name, _span) => Ok((format!("<!-- Slot: {} -->", name), Vec::new())),
        Element::Component {
            name,
            attributes,
            content,
            ..
        } => {
            // Find the component definition
            if let Some(component) = document.components.get(name) {
                // Collect static prop values (String attributes matching declared props)
                let prop_values: std::collections::HashMap<String, String> = attributes
                    .iter()
                    .filter_map(|a| {
                        if let AttributeValue::String(v) = &a.value {
                            Some((a.name.clone(), v.clone()))
                        } else {
                            None
                        }
                    })
                    .collect();

                // Generate scope ID for this component's CSS
                let component_scope = generate_scope_id(name);

                // Substitute props into the view (avoids clone when no props given)
                let substituted;
                let view: &[Element] = if prop_values.is_empty() {
                    &component.view
                } else {
                    substituted = substitute_props(&component.view, &prop_values);
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
            iterable,
            content,
            ..
        } => {
            let mut result = String::new();
            let mut open = format!(
                "<template data-webcore-for=\"{}\" data-webcore-in=\"{}\"",
                item, iterable
            );
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

/// Substitute static prop values into a component view before rendering.
///
/// Each `Interpolation(expr)` whose `expr` matches a prop name is replaced with
/// `Text(value)`. Other element types are recursed into.
fn substitute_props(
    elements: &[Element],
    props: &std::collections::HashMap<String, String>,
) -> Vec<Element> {
    elements
        .iter()
        .map(|e| substitute_props_elem(e, props))
        .collect()
}

fn substitute_props_elem(
    element: &Element,
    props: &std::collections::HashMap<String, String>,
) -> Element {
    match element {
        Element::Interpolation(expr, span) => {
            if let Some(val) = props.get(expr.trim()) {
                Element::Text(val.clone(), *span)
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
            attributes: attributes.clone(),
            content: substitute_props(content, props),
            span: *span,
        },
        Element::Component {
            name,
            attributes,
            content,
            span,
        } => Element::Component {
            name: name.clone(),
            attributes: attributes.clone(),
            content: substitute_props(content, props),
            span: *span,
        },
        Element::For {
            item,
            iterable,
            content,
            span,
        } => Element::For {
            item: item.clone(),
            iterable: iterable.clone(),
            content: substitute_props(content, props),
            span: *span,
        },
        Element::If {
            condition,
            then_branch,
            else_branch,
            span,
        } => Element::If {
            condition: condition.clone(),
            then_branch: substitute_props(then_branch, props),
            else_branch: else_branch.as_ref().map(|eb| substitute_props(eb, props)),
            span: *span,
        },
        Element::ErrorBlock {
            field,
            content,
            span,
        } => Element::ErrorBlock {
            field: field.clone(),
            content: substitute_props(content, props),
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
                    // Add content area for hybrid router
                    result.push_str("  <main id=\"webcore-content\">\n");
                    result.push_str(
                        "    <!-- Content will be loaded here by the hybrid router -->\n",
                    );
                    result.push_str("  </main>\n");
                } else {
                    result.push_str(&format!("<!-- Slot: {} -->\n", slot_name));
                }
            }
            Element::Tag { name, content, .. } => {
                if name == "main" && content.len() == 1 {
                    // Check if main contains only a slot
                    if let Element::Slot(slot_name, _) = &content[0] {
                        if slot_name == "content" {
                            // Replace main with slot with our content area
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
        };
        let res = generate_html(&doc, "test", &opts).expect("html ok");
        assert!(res.html.contains("onfoo=\"webcore_handle_event('foo',"));
    }
}
