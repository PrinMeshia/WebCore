//! Component element rendering for HTML code generation.

use crate::core::ast::{Attribute, AttributeValue, Element, WebCoreDocument};
use crate::codegen::attr_names;
use crate::core::utils::html_escape;
use std::fmt::Write as _;
use std::path::Path;

use crate::codegen::css::generate_scope_id;
use super::{generate_elements_with_scope_and_counter, HandlerMapping};
use super::props::substitute_props;
use crate::core::error::CompileError;

/// Render a component call site: resolve the component definition, substitute props,
/// apply the component's CSS scope, and recursively generate the view.
/// Falls back to rendering as a plain HTML element if the component is unknown.
#[allow(clippy::too_many_arguments)]
pub(super) fn generate_component_element(
    name: &str,
    attributes: &[Attribute],
    content: &[Element],
    document: &WebCoreDocument,
    counter: &mut usize,
    scope_id: Option<&str>,
    prefix: &str,
    project_root: Option<&Path>,
) -> Result<(String, Vec<HandlerMapping>), CompileError> {
    if let Some(component) = document.components.get(name) {
        // Warn on unknown props at compile time (skip built-in directive prefixes)
        if !component.props.is_empty() {
            let declared: std::collections::HashSet<&str> =
                component.props.iter().map(|p| p.name.as_str()).collect();
            for attr in attributes {
                if attr.name.starts_with("on:")
                    || attr.name.starts_with("webc:")
                    || attr.name.starts_with("class:")
                    || attr.name.starts_with("style:")
                    || attr.name.starts_with("ref:")
                    || attr.name.starts_with("bind:")
                    || attr.name.starts_with("client:")
                {
                    continue;
                }
                if !declared.contains(attr.name.as_str()) {
                    eprintln!(
                        "warning[props]: component '{}' received unknown prop '{}' — it will be ignored",
                        name, attr.name
                    );
                }
            }
        }

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

        let component_scope = generate_scope_id(name);

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
            project_root,
        )
    } else {
        let mut result = String::new();
        write!(result, "<{name}").unwrap();

        if let Some(sid) = scope_id {
            write!(result, " {}=\"{}\"", attr_names::SCOPE, sid).unwrap();
        }

        if attributes
            .iter()
            .any(|a| matches!(&a.value, AttributeValue::Expression(_)))
        {
            write!(result, " {}", attr_names::BOUND).unwrap();
        }

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
                    write!(result, " data-webcore-attr-{}=\"{}\"", attr.name, html_escape(expr)).unwrap();
                }
            }
        }

        result.push('>');
        let (content_html, content_handlers) = generate_elements_with_scope_and_counter(
            content, document, scope_id, counter, prefix, project_root,
        )?;
        result.push_str(&content_html);
        write!(result, "</{name}>").unwrap();
        Ok((result, content_handlers))
    }
}
