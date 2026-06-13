//! Component call-site rendering: prop validation and substitution
//! (static, dynamic, defaults), CSS scope assignment, recursive view
//! generation — with a plain-HTML fallback for unknown components.

use crate::codegen::attr_names;
use crate::codegen::css::generate_scope_id;
use crate::core::ast::{Attribute, AttributeValue, Element};
use crate::core::error::CompileError;
use std::fmt::Write as _;

use super::elements::generate_elements;
use super::props::substitute_props;
use super::utils::{is_void_element, push_close_tag, push_plain_attributes};
use super::{GenContext, HandlerMapping};

/// Render a component call site: resolve the component definition, substitute props,
/// apply the component's CSS scope, and recursively generate the view.
/// Falls back to rendering as a plain HTML element if the component is unknown.
pub(super) fn generate_component_element(
    name: &str,
    attributes: &[Attribute],
    content: &[Element],
    ctx: &mut GenContext,
    scope_id: Option<&str>,
) -> Result<(String, Vec<HandlerMapping>), CompileError> {
    // Find the component definition
    if let Some(component) = ctx.document.components.get(name) {
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

        // Collect static prop values — may be extended below with default values
        let static_props: std::collections::BTreeMap<String, String> = attributes
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
        let dynamic_props: std::collections::BTreeMap<String, String> = attributes
            .iter()
            .filter_map(|a| {
                if let AttributeValue::Expression(e) = &a.value {
                    Some((a.name.clone(), e.clone()))
                } else {
                    None
                }
            })
            .collect();

        // Inject default prop values for props not supplied by the caller
        let mut static_props = static_props;
        for prop in &component.props {
            if let Some(ref default_val) = prop.default_value {
                if !static_props.contains_key(&prop.name) && !dynamic_props.contains_key(&prop.name)
                {
                    static_props.insert(prop.name.clone(), default_val.clone());
                }
            }
        }

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

        generate_elements(view, ctx, component_scope_str.as_deref())
    } else {
        // Component not found, generate as HTML element
        let mut result = String::new();
        write!(result, "<{name}").expect("write! to String is infallible");

        // Add scope if we have one
        if let Some(sid) = scope_id {
            write!(result, " {}=\"{}\"", attr_names::SCOPE, sid)
                .expect("write! to String is infallible");
        }

        // Mark elements that have dynamic attribute bindings
        if attributes
            .iter()
            .any(|a| matches!(&a.value, AttributeValue::Expression(_)))
        {
            write!(result, " {}", attr_names::BOUND).expect("write! to String is infallible");
        }

        // Generate attributes
        push_plain_attributes(&mut result, attributes);

        result.push('>');
        if is_void_element(name) && content.is_empty() {
            return Ok((result, Vec::new()));
        }
        let (content_html, content_handlers) = generate_elements(content, ctx, scope_id)?;
        result.push_str(content_html.trim_end_matches('\n'));
        push_close_tag(&mut result, name);
        Ok((result, content_handlers))
    }
}
