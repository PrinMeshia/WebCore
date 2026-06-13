//! Attribute handlers for HTML code generation.
//!
//! Covers: bind: expansion, ref:, class:, style:, validate:, on: event attrs,
//! and the navigate-path helper used by event handling.

use crate::core::ast::{Attribute, AttributeValue};
use crate::codegen::attr_names;
use crate::core::utils::html_escape;
use std::fmt::Write as _;

use super::HandlerMapping;

/// Extract path from `webcore_navigate(path)` expression.
pub(super) fn extract_navigate_path(expr: &str) -> Option<String> {
    let expr = expr.trim();
    if let Some(start) = expr.find("webcore_navigate(") {
        let after_paren = &expr[start + 17..];
        if let Some(end) = after_paren.find(')') {
            let path = after_paren[..end].trim();
            let clean_path = if path == "root" {
                "/".to_string()
            } else if path.starts_with('"') && path.ends_with('"') {
                path[1..path.len() - 1].to_string()
            } else if path.starts_with('/') {
                path.to_string()
            } else {
                format!("/{path}")
            };
            return Some(clean_path);
        }
    }
    None
}

/// Expand `bind:attr={expr}` into a value/checked attr + event handler pair.
pub(super) fn expand_bind_attrs(attributes: &[Attribute]) -> Vec<Attribute> {
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

/// `ref:name=true` → `Some(" data-webcore-ref=\"name\"")`; returns `None` for other attrs.
pub(super) fn handle_ref_attr(attr_name: &str) -> Option<String> {
    let ref_name = attr_name.strip_prefix("ref:")?;
    Some(format!(" {}=\"{}\"", attr_names::REF, html_escape(ref_name)))
}

/// `class:name={expr}` → `Some(" data-webcore-class-name=\"expr\"")`; returns `None` otherwise.
pub(super) fn handle_class_binding(attr_name: &str, expr: &str) -> Option<String> {
    let class_name = attr_name.strip_prefix("class:")?;
    Some(format!(
        " {}{}=\"{}\"",
        attr_names::CLASS_PREFIX,
        class_name,
        html_escape(expr)
    ))
}

/// `style:prop={expr}` → `Some(" data-webcore-style-prop=\"expr\"")`; returns `None` otherwise.
pub(super) fn handle_style_binding(attr_name: &str, expr: &str) -> Option<String> {
    let prop_name = attr_name.strip_prefix("style:")?;
    Some(format!(
        " {}{}=\"{}\"",
        attr_names::STYLE_PREFIX,
        prop_name,
        html_escape(expr)
    ))
}

/// Emit HTML for a validate:* attribute.
pub(super) fn handle_validation_attr(attr: &Attribute) -> Option<String> {
    let validator = attr.name.strip_prefix("validate:")?;
    let mut out = String::new();
    match &attr.value {
        AttributeValue::String(v) => match validator {
            "minlength" | "maxlength" => {
                let (constraint, msg) = v.split_once(',').unwrap_or((v.as_str(), ""));
                write!(out, " data-webcore-validate-{}=\"{}\"",
                    validator, html_escape(constraint.trim())).unwrap();
                if !msg.is_empty() {
                    write!(out, " data-webcore-validate-{}-msg=\"{}\"",
                        validator, html_escape(msg.trim())).unwrap();
                }
            }
            "pattern" => {
                let (pat, msg) = v.split_once(',').unwrap_or((v.as_str(), ""));
                write!(out, " data-webcore-validate-pattern=\"{}\"", html_escape(pat.trim())).unwrap();
                if !msg.is_empty() {
                    write!(out, " data-webcore-validate-pattern-msg=\"{}\"", html_escape(msg.trim())).unwrap();
                }
            }
            _ => {
                write!(out, " data-webcore-validate-{}=\"{}\"", validator, html_escape(v)).unwrap();
            }
        },
        AttributeValue::Boolean(true) => {
            write!(out, " data-webcore-validate-{validator}=\"\"").unwrap();
        }
        _ => {}
    }
    Some(out)
}

/// Returns the HTML attribute string for an `on:event={expr}` attribute.
#[allow(clippy::too_many_arguments)]
pub(super) fn handle_event_attr(
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
