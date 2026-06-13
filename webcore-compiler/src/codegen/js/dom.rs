//! Document analysis helpers for JS code generation.
//!
//! Walks the `WebCore` AST to detect which runtime features are needed
//! (`RuntimeFeatures`), collects lifecycle hook bodies, and gathers
//! component-level event-listener registrations.

use crate::core::ast::{Element, AttributeValue, WebCoreDocument};

/// Component-level event listener: emitted by `on:eventName={expr}` on a component call.
pub struct EventListenerMapping {
    pub event_name: String,
    pub expression: String,
}

/// Features detected by walking the document AST — drives tree-shaking.
#[derive(Default)]
pub(super) struct RuntimeFeatures {
    pub has_interpolation: bool,
    pub has_if: bool,
    pub has_for: bool,
    pub has_dynamic_attrs: bool,
    pub has_validation: bool,
    pub has_navigation: bool,
    pub has_param_routes: bool,
    /// Any component has an `http { }` block
    pub has_http: bool,
    /// Any expression contains `$query.`
    pub has_query_params: bool,
    /// Any element attribute starts with `class:`
    pub has_class_binding: bool,
    /// Any event attribute name contains `|debounce`
    pub has_debounce: bool,
    /// Any element attribute starts with `ref:`
    pub has_refs: bool,
    /// Any element attribute starts with `style:`
    pub has_style_binding: bool,
    /// Any element has a `webc:transition` attribute
    pub has_transition: bool,
}

pub(super) fn detect_features_in_elements(elements: &[Element], f: &mut RuntimeFeatures) {
    for elem in elements {
        match elem {
            Element::Interpolation(expr, _) => {
                f.has_interpolation = true;
                if expr.contains("$query.") {
                    f.has_query_params = true;
                }
            }
            Element::For { content, iterable, .. } => {
                f.has_for = true;
                if iterable.contains("$query.") {
                    f.has_query_params = true;
                }
                detect_features_in_elements(content, f);
            }
            Element::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                f.has_if = true;
                if condition.contains("$query.") {
                    f.has_query_params = true;
                }
                detect_features_in_elements(then_branch, f);
                if let Some(eb) = else_branch {
                    detect_features_in_elements(eb, f);
                }
            }
            Element::Tag {
                name,
                attributes,
                content,
                ..
            } => {
                if name == "link" && attributes.iter().any(|a| a.name == "to") {
                    f.has_navigation = true;
                }
                for attr in attributes {
                    if attr.name.starts_with("validate:") {
                        f.has_validation = true;
                    }
                    if attr.name.starts_with("class:") {
                        f.has_class_binding = true;
                    }
                    if attr.name.contains("|debounce") {
                        f.has_debounce = true;
                    }
                    if attr.name.starts_with("ref:") {
                        f.has_refs = true;
                    }
                    if attr.name.starts_with("style:") {
                        f.has_style_binding = true;
                    }
                    if attr.name == "webc:transition" {
                        f.has_transition = true;
                    }
                    match &attr.value {
                        AttributeValue::Expression(expr) => {
                            if !attr.name.starts_with("on:")
                                && !attr.name.starts_with("class:")
                                && !attr.name.starts_with("ref:")
                                && !attr.name.starts_with("style:")
                                && attr.name != "webc:transition"
                            {
                                f.has_dynamic_attrs = true;
                            }
                            if expr.contains("webcore_navigate(") {
                                f.has_navigation = true;
                            }
                            if expr.contains("$query.") {
                                f.has_query_params = true;
                            }
                        }
                        AttributeValue::String(s) => {
                            if s.contains("$query.") {
                                f.has_query_params = true;
                            }
                        }
                        _ => {}
                    }
                }
                detect_features_in_elements(content, f);
            }
            Element::Component { content, .. } | Element::SlotContent { content, .. } => {
                detect_features_in_elements(content, f);
            }
            Element::ErrorBlock { content, .. } => {
                f.has_validation = true;
                detect_features_in_elements(content, f);
            }
            Element::Text(t, _) => {
                if t.contains("$query.") {
                    f.has_query_params = true;
                }
            }
            Element::Slot(..) => {}
        }
    }
}

pub(super) fn detect_features(document: &WebCoreDocument) -> RuntimeFeatures {
    let mut f = RuntimeFeatures::default();
    if let Some(app) = &document.app {
        if !app.routes.is_empty() {
            f.has_navigation = true;
        }
        if app.routes.keys().any(|path| path.contains(':')) {
            f.has_param_routes = true;
        }
    }
    for page in document.pages.values() {
        detect_features_in_elements(&page.content, &mut f);
    }
    for component in document.components.values() {
        if component.http.is_some() {
            f.has_http = true;
        }
        detect_features_in_elements(&component.view, &mut f);
    }
    for layout in document.layouts.values() {
        detect_features_in_elements(&layout.content, &mut f);
    }
    f
}

/// Collect on:mount bodies from all components (raw JS to run at `DOMContentLoaded`).
pub(super) fn collect_on_mount_bodies(document: &WebCoreDocument) -> Vec<String> {
    document
        .components
        .values()
        .filter_map(|c| c.mount_body.as_ref())
        .filter(|b| !b.trim().is_empty())
        .cloned()
        .collect()
}

/// Collect on:destroy bodies from all components.
pub(super) fn collect_on_destroy_bodies(document: &WebCoreDocument) -> Vec<String> {
    document
        .components
        .values()
        .filter_map(|c| c.destroy_body.as_ref())
        .filter(|b| !b.trim().is_empty())
        .cloned()
        .collect()
}

/// Build the semicolon-separated sequence of rebind calls for a given feature set.
pub(super) fn rebind_seq(f: &RuntimeFeatures, needs_bind: bool) -> String {
    let mut parts: Vec<&str> = Vec::new();
    if needs_bind {
        parts.push("bind()");
    }
    if f.has_if {
        parts.push("bindIf()");
    }
    if f.has_for {
        parts.push("bindFor()");
    }
    if f.has_dynamic_attrs || f.has_style_binding {
        parts.push("bindAttrs()");
    }
    if f.has_class_binding {
        parts.push("bindClassBindings()");
    }
    if f.has_validation {
        parts.push("bindValidation()");
    }
    parts.join(";")
}

/// Walk elements collecting on:eventName={expr} attrs on component calls.
fn collect_event_listeners_from_elements(
    elements: &[Element],
    out: &mut Vec<EventListenerMapping>,
) {
    for elem in elements {
        match elem {
            Element::Component {
                attributes,
                content,
                ..
            } => {
                for attr in attributes {
                    if let Some(event_name) = attr.name.strip_prefix("on:") {
                        if let AttributeValue::Expression(expr) = &attr.value {
                            out.push(EventListenerMapping {
                                event_name: event_name.to_string(),
                                expression: expr.clone(),
                            });
                        }
                    }
                }
                collect_event_listeners_from_elements(content, out);
            }
            Element::Tag { content, .. } | Element::For { content, .. } => collect_event_listeners_from_elements(content, out),
            Element::If {
                then_branch,
                else_branch,
                ..
            } => {
                collect_event_listeners_from_elements(then_branch, out);
                if let Some(eb) = else_branch {
                    collect_event_listeners_from_elements(eb, out);
                }
            }
            Element::ErrorBlock { content, .. } | Element::SlotContent { content, .. } => {
                collect_event_listeners_from_elements(content, out);
            }
            _ => {}
        }
    }
}

/// Collect component-level event listeners from the full document.
pub(super) fn collect_component_event_listeners(document: &WebCoreDocument) -> Vec<EventListenerMapping> {
    let mut out = Vec::new();
    for page in document.pages.values() {
        collect_event_listeners_from_elements(&page.content, &mut out);
    }
    for component in document.components.values() {
        collect_event_listeners_from_elements(&component.view, &mut out);
    }
    for layout in document.layouts.values() {
        collect_event_listeners_from_elements(&layout.content, &mut out);
    }
    out
}
