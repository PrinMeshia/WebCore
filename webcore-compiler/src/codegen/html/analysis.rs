use crate::core::ast::{AttributeValue, Element, Layout, WebCoreDocument};
use crate::core::error::CompileError;

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

pub(super) fn elements_need_js(elements: &[crate::core::ast::Element]) -> bool {
    use crate::core::ast::Element;
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

pub(super) fn document_needs_js(document: &WebCoreDocument, page_name: &str) -> bool {
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
            || elements_need_js(&comp.view)
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

pub(super) fn find_layout(document: &WebCoreDocument) -> Result<&Layout, CompileError> {
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
