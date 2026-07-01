//! Slot resolution: match page-provided slot content into the layout's
//! `slot` placeholders (named slots, default content, layout fallbacks).

use crate::core::ast::{Element, Layout, Page, WebCoreDocument};
use crate::core::error::CompileError;
use crate::core::ssg::SsgContext;
use std::path::Path;

use super::elements::generate_elements;
use super::utils::safe_id_prefix;
use super::{GenContext, HtmlGenerationResult};

/// Returns `true` if any element in the slice (or its descendants) is a `Slot`
/// or `SlotContent`. Used to short-circuit `resolve_slots` on subtrees that
/// need no substitution.
fn contains_slot(elements: &[Element]) -> bool {
    elements.iter().any(|e| match e {
        Element::Slot(..) | Element::SlotContent { .. } => true,
        Element::Tag { content, .. }
        | Element::Component { content, .. }
        | Element::For { content, .. }
        | Element::ErrorBlock { content, .. }
        | Element::Fragment { content, .. } => contains_slot(content),
        Element::If {
            then_branch,
            else_branch,
            ..
        } => contains_slot(then_branch) || else_branch.as_ref().is_some_and(|eb| contains_slot(eb)),
        _ => false,
    })
}

/// Recursively replace Slot placeholders in a layout tree with provided page content.
fn resolve_slots(
    elements: &[Element],
    slot_map: &std::collections::BTreeMap<String, Vec<Element>>,
    default_content: &[Element],
) -> Vec<Element> {
    if !contains_slot(elements) {
        return elements.to_vec();
    }
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
            Element::Fragment { content, span } => {
                resolved.push(Element::Fragment {
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
    std::collections::BTreeMap<String, Vec<Element>>,
    Vec<Element>,
) {
    let mut named: std::collections::BTreeMap<String, Vec<Element>> =
        std::collections::BTreeMap::new();
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

#[allow(clippy::too_many_arguments)]
pub(super) fn generate_layout_with_page_and_components(
    layout: &Layout,
    page: &Page,
    document: &WebCoreDocument,
    project_root: Option<&Path>,
    ssg: Option<&SsgContext>,
    compiled_vars: Option<&crate::codegen::js::js_events::CompiledVars>,
    has_route_params: bool,
    has_query_params: bool,
) -> Result<HtmlGenerationResult, CompileError> {
    let prefix = safe_id_prefix(&page.name);
    let (named_slots, default_content) = separate_slot_content(&page.content);
    let resolved = resolve_slots(&layout.content, &named_slots, &default_content);
    let mut ctx = GenContext {
        document,
        prefix: &prefix,
        project_root,
        ssg,
        counter: 0,
        compiled_vars,
        expr_map: vec![],
        expr_spans: vec![],
        expr_counter: 0,
        has_route_params,
        has_query_params,
    };
    let (html, handlers) = generate_elements(&resolved, &mut ctx, None)?;
    Ok(HtmlGenerationResult {
        html,
        handlers,
        compiled_exprs: ctx.expr_map,
        expr_spans: ctx.expr_spans,
        source_map_json: None,
    })
}
