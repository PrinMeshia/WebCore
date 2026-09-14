//! Document analysis helpers for JS code generation.
//!
//! Walks the `WebCore` AST to detect which runtime features are needed
//! (`RuntimeFeatures`), collects lifecycle hook bodies, and gathers
//! component-level event-listener registrations.

use crate::core::ast::{AttributeValue, Element, WebCoreDocument};

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
    /// Any element uses `@defer { }` (lazy render after DOMContentLoaded)
    pub has_defer: bool,
    /// Any element has a spread attribute `...obj`
    pub has_spread: bool,
    /// Any component instance carries a `client:idle` / `client:visible`
    /// partial-hydration directive (islands, #50).
    pub has_islands: bool,
    /// Any component declares a `computed { }` var. Set by the JS generator
    /// (not by `detect_features`) once the computed table is built.
    pub has_computed: bool,
}

pub(super) fn detect_features_in_elements(elements: &[Element], f: &mut RuntimeFeatures) {
    crate::core::ast::walk_elements(elements, &mut |elem| match elem {
        Element::Interpolation(expr, _) => {
            f.has_interpolation = true;
            if expr.contains("$query.") {
                f.has_query_params = true;
            }
        }
        Element::For { iterable, .. } => {
            f.has_for = true;
            if iterable.contains("$query.") {
                f.has_query_params = true;
            }
        }
        Element::If { condition, .. } => {
            f.has_if = true;
            if condition.contains("$query.") {
                f.has_query_params = true;
            }
        }
        Element::Tag {
            name, attributes, ..
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
                    AttributeValue::Spread(_) => {
                        f.has_spread = true;
                        f.has_dynamic_attrs = true;
                    }
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
                    AttributeValue::String(s) if s.contains("$query.") => {
                        f.has_query_params = true;
                    }
                    _ => {}
                }
            }
        }
        Element::Defer { .. } => f.has_defer = true,
        Element::Component { attributes, .. } => {
            if crate::core::ast::island_strategy(attributes).is_some() {
                f.has_islands = true;
            }
        }
        Element::ErrorBlock { .. } => f.has_validation = true,
        Element::Text(t, _) => {
            if t.contains("$query.") {
                f.has_query_params = true;
            }
        }
        // Slot: no runtime impact. Markdown: inlined as static HTML at build.
        Element::Slot(..) | Element::SlotContent { .. } | Element::Fragment { .. } => {}
        Element::Markdown(..) => {}
    });
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

/// Collect `(component name, on:mount body)` pairs (raw JS). Components with an
/// empty body are skipped. The name lets the bootstrap defer island components'
/// mount to hydration time (#50).
pub(super) fn collect_on_mount_bodies(document: &WebCoreDocument) -> Vec<(String, String)> {
    document
        .components
        .iter()
        .filter_map(|(name, c)| {
            c.mount_body
                .as_ref()
                .filter(|b| !b.trim().is_empty())
                .map(|b| (name.clone(), b.clone()))
        })
        .collect()
}

/// Component names to defer `on:mount` for (islands, #50): those used
/// **exclusively** as `client="idle"` / `client="visible"` islands across the
/// whole (shared-runtime) document. A component that also appears eagerly
/// anywhere keeps its mount at load — deferring it would drop the mount of its
/// eager instances (the runtime is shared across all pages).
pub(super) fn collect_deferred_mount_components(
    document: &WebCoreDocument,
) -> std::collections::BTreeSet<String> {
    use std::collections::BTreeSet;
    fn walk(elements: &[Element], island: &mut BTreeSet<String>, eager: &mut BTreeSet<String>) {
        for el in elements {
            match el {
                Element::Component {
                    name,
                    attributes,
                    content,
                    ..
                } => {
                    if crate::core::ast::island_strategy(attributes).is_some() {
                        island.insert(name.clone());
                    } else {
                        eager.insert(name.clone());
                    }
                    walk(content, island, eager);
                }
                Element::Tag { content, .. }
                | Element::For { content, .. }
                | Element::SlotContent { content, .. }
                | Element::ErrorBlock { content, .. }
                | Element::Fragment { content, .. }
                | Element::Defer { content, .. } => walk(content, island, eager),
                Element::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    walk(then_branch, island, eager);
                    if let Some(eb) = else_branch {
                        walk(eb, island, eager);
                    }
                }
                _ => {}
            }
        }
    }
    let (mut island, mut eager) = (BTreeSet::new(), BTreeSet::new());
    for page in document.pages.values() {
        walk(&page.content, &mut island, &mut eager);
    }
    for layout in document.layouts.values() {
        walk(&layout.content, &mut island, &mut eager);
    }
    for comp in document.components.values() {
        walk(&comp.view, &mut island, &mut eager);
    }
    island.difference(&eager).cloned().collect()
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

/// Build the rebind call sequence for v3.
///
/// v3 bind functions read the module-scoped `_e` expression map via closure
/// (not as a parameter), so user-authored `bind()` calls inside `on:mount`
/// blocks work identically to the framework's own rebind calls.
pub(super) fn rebind_seq_v3(f: &RuntimeFeatures, needs_bind: bool) -> String {
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
    if f.has_defer {
        parts.push("bindDefer()");
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
            Element::Tag { content, .. } | Element::For { content, .. } => {
                collect_event_listeners_from_elements(content, out)
            }
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
            Element::ErrorBlock { content, .. }
            | Element::SlotContent { content, .. }
            | Element::Fragment { content, .. }
            | Element::Defer { content, .. } => {
                collect_event_listeners_from_elements(content, out);
            }
            _ => {}
        }
    }
}

/// Collect component-level event listeners from the full document.
pub(super) fn collect_component_event_listeners(
    document: &WebCoreDocument,
) -> Vec<EventListenerMapping> {
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
