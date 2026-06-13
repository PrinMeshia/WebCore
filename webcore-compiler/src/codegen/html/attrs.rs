use crate::core::ast::{Attribute, AttributeValue};
use crate::codegen::attr_names;
use super::utils::html_escape;
use std::fmt::Write as _;
use super::HandlerMapping;

/// Expand `bind:attr={expr}` into a value/checked attr + event handler pair.
/// `bind:value={x}`   → `value={x}` + `on:input={x = event.target.value}`
/// `bind:checked={x}` → `checked={x}` + `on:change={x = event.target.checked}`
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
pub(super) fn handle_ref_attr(attr_name: &str) -> Option<String> {
    let ref_name = attr_name.strip_prefix("ref:")?;
    Some(format!(
        " {}=\"{}\"",
        attr_names::REF,
        html_escape(ref_name)
    ))
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
/// Returns the one or two data-* attribute strings, or `None` if not a validate: attr.
pub(super) fn handle_validation_attr(attr: &Attribute) -> Option<String> {
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

    // Split on `|` to extract the base event type and optional modifiers.
    // Supported modifiers: stop, prevent, once, self, debounce[=N]
    let parts: Vec<&str> = raw_event_type.split('|').collect();
    let base_event_type = parts[0];
    let mut debounce_ms: Option<u32> = None;
    let mut modifiers: Vec<&str> = Vec::new();
    for part in &parts[1..] {
        if part.starts_with("debounce") {
            let ms = part.strip_prefix("debounce=")
                .and_then(|s| s.parse().ok())
                .unwrap_or(300u32);
            debounce_ms = Some(ms);
        } else {
            modifiers.push(part);
        }
    }

    *counter += 1;
    let handler_id = format!("{prefix}btn{counter}");

    // Extract href from webcore_navigate() for links
    if is_link && expr.contains("webcore_navigate") {
        if let Some(path) = super::utils::extract_navigate_path(expr) {
            *resolved_href = Some(path);
        }
    }

    // HandlerMapping stores base event type for non-debounce, or "type|debounce=N" for debounce.
    // Non-debounce modifiers (stop, prevent, once, self) are encoded in the HTML attribute.
    let mapped_event_type = if let Some(ms) = debounce_ms {
        format!("{base_event_type}|debounce={ms}")
    } else {
        base_event_type.to_string()
    };
    handlers.push(HandlerMapping {
        id: handler_id.clone(),
        event_type: mapped_event_type,
        expression: expr.to_string(),
    });

    // Use data-webcore-e="<type>[|mod1|mod2]" for CSP-compatible event delegation.
    // Debounced handlers are wired via getElementById in JS (no data-webcore-e needed).
    let html_attr = if debounce_ms.is_some() {
        format!(" id=\"{handler_id}\"")
    } else if modifiers.is_empty() {
        format!(" id=\"{handler_id}\" data-webcore-e=\"{base_event_type}\"")
    } else {
        let mods_str = modifiers.join("|");
        format!(" id=\"{handler_id}\" data-webcore-e=\"{base_event_type}|{mods_str}\"")
    };
    Some(html_attr)
}
