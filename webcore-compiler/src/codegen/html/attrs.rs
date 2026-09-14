use super::utils::html_escape;
use super::HandlerMapping;
use crate::codegen::attr_names;
use crate::core::ast::{Attribute, AttributeValue, Span};
use std::fmt::Write as _;

/// Base event type of an `on:` attribute name: `on:input|debounce=300` → `input`.
fn base_event(attr_name: &str) -> &str {
    attr_name
        .strip_prefix("on:")
        .unwrap_or(attr_name)
        .split('|')
        .next()
        .unwrap_or("")
}

/// Expand `bind:attr={expr}` into a value/checked attr + event handler pair.
/// `bind:value={x}`   → `value={x}` + `on:input={x = event.target.value}`
/// `bind:checked={x}` → `checked={x}` + `on:change={x = event.target.checked}`
///
/// When the element already declares a handler for the same event
/// (`bind:value={msg} on:input={count = event.target.value.length}`), the state
/// assignment is **prepended** to that handler instead of emitting a second
/// `on:input`: event delegation keys handlers by element id + event type, so two
/// handlers for one event on one element would silently drop one of them.
/// Debounced handlers are left alone — they are wired directly on the element
/// and merging would delay the state update too.
pub(super) fn expand_bind_attrs(attributes: &[Attribute]) -> Vec<Attribute> {
    if !attributes.iter().any(|a| a.name.starts_with("bind:")) {
        return attributes.to_vec();
    }
    let mut result: Vec<Attribute> = Vec::with_capacity(attributes.len() + 2);
    // (event attr name, assignment) pairs still to place, in source order.
    let mut pending: Vec<(&'static str, String, Span)> = Vec::new();
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
                pending.push((event, format!("{} = {}", expr.trim(), prop), attr.span));
            }
        } else {
            result.push(attr.clone());
        }
    }

    for (event, assignment, span) in pending {
        let existing = result.iter_mut().find(|a| {
            a.name.starts_with("on:")
                && !a.name.contains("|debounce")
                && base_event(&a.name) == base_event(event)
                && matches!(a.value, AttributeValue::Expression(_))
        });
        match existing {
            Some(attr) => {
                if let AttributeValue::Expression(expr) = &mut attr.value {
                    *expr = format!("{assignment}; {}", expr.trim());
                }
            }
            None => result.push(Attribute {
                name: event.to_string(),
                value: AttributeValue::Expression(assignment),
                span,
            }),
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

/// `class:name={expr}` → `Some(" data-webcore-class-name=\"id\"")`; returns `None` otherwise.
/// In v3, `id` is a compiled expression ID from `ctx.register_expr(expr, span)`.
pub(super) fn handle_class_binding(
    attr_name: &str,
    expr: &str,
    span: Span,
    ctx: &mut super::GenContext<'_>,
) -> Option<String> {
    let class_name = attr_name.strip_prefix("class:")?;
    let id = ctx.register_expr(expr, span);
    Some(format!(
        " {}{}=\"{}\"",
        attr_names::CLASS_PREFIX,
        class_name,
        html_escape(&id)
    ))
}

/// `style:prop={expr}` → `Some(" data-webcore-style-prop=\"id\"")`; returns `None` otherwise.
/// In v3, `id` is a compiled expression ID from `ctx.register_expr(expr, span)`.
pub(super) fn handle_style_binding(
    attr_name: &str,
    expr: &str,
    span: Span,
    ctx: &mut super::GenContext<'_>,
) -> Option<String> {
    let prop_name = attr_name.strip_prefix("style:")?;
    let id = ctx.register_expr(expr, span);
    Some(format!(
        " {}{}=\"{}\"",
        attr_names::STYLE_PREFIX,
        prop_name,
        html_escape(&id)
    ))
}

/// Emit HTML for a validate:* attribute.
/// Returns the one or two data-* attribute strings, or `None` if not a validate: attr.
pub(super) fn handle_validation_attr(
    attr: &Attribute,
    ssg: Option<&crate::core::ssg::SsgContext>,
) -> Option<String> {
    let validator = attr.name.strip_prefix("validate:")?;
    let mut out = String::new();
    match &attr.value {
        AttributeValue::String(raw) => {
            // Resolve `{t(…)}` / interpolations for the active locale and
            // un-escape string literals (`\"` → `"`). Without an SSG context,
            // escapes are still undone and interpolations collapse to empty.
            let v = match ssg {
                Some(s) => s.resolve_interpolated(raw),
                None => crate::core::ssg::resolve_interpolated_static(raw),
            };
            let v = &v;
            match validator {
                "minlength" | "maxlength" => {
                    let (constraint, msg) = v.split_once(',').unwrap_or((v.as_str(), ""));
                    write!(
                        out,
                        " data-webcore-validate-{}=\"{}\"",
                        validator,
                        html_escape(constraint.trim())
                    )
                    .expect("write! to String is infallible");
                    if !msg.is_empty() {
                        write!(
                            out,
                            " data-webcore-validate-{}-msg=\"{}\"",
                            validator,
                            html_escape(msg.trim())
                        )
                        .expect("write! to String is infallible");
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
                    .expect("write! to String is infallible");
                    if !msg.is_empty() {
                        write!(
                            out,
                            " data-webcore-validate-pattern-msg=\"{}\"",
                            html_escape(msg.trim())
                        )
                        .expect("write! to String is infallible");
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
                    .expect("write! to String is infallible");
                }
            }
        }
        AttributeValue::Boolean(true) => {
            write!(out, " data-webcore-validate-{validator}=\"\"")
                .expect("write! to String is infallible");
        }
        _ => {}
    }
    Some(out)
}

/// Register an `on:event={expr}` attribute against `element_id` and return the
/// `data-webcore-e` descriptor for it (`click`, `click|stop|prevent`, …), or
/// `None` for debounced handlers (wired directly by element id in JS).
///
/// Handlers are keyed `<element id>@<event type>` so one element can carry
/// several distinct events; the element itself gets a single `id`.
/// Returns `None` when `attr_name` does not start with `on:`.
#[allow(clippy::too_many_arguments)]
pub(super) fn handle_event_attr(
    attr_name: &str,
    expr: &str,
    is_link: bool,
    element_id: &str,
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
            let ms = part
                .strip_prefix("debounce=")
                .and_then(|s| s.parse().ok())
                .unwrap_or(300u32);
            debounce_ms = Some(ms);
        } else {
            modifiers.push(part);
        }
    }

    // Extract href from webcore_navigate() for links
    if is_link && expr.contains("webcore_navigate") {
        if let Some(path) = super::utils::extract_navigate_path(expr) {
            *resolved_href = Some(path);
        }
    }

    // Debounced handlers keep the bare element id: the JS wires them with
    // getElementById + addEventListener instead of going through `H`.
    if let Some(ms) = debounce_ms {
        handlers.push(HandlerMapping {
            id: element_id.to_string(),
            event_type: format!("{base_event_type}|debounce={ms}"),
            expression: expr.to_string(),
        });
        return None;
    }

    handlers.push(HandlerMapping {
        id: format!("{element_id}@{base_event_type}"),
        event_type: base_event_type.to_string(),
        expression: expr.to_string(),
    });

    // data-webcore-e descriptor: "<type>[|mod1|mod2]" (CSP-safe delegation).
    if modifiers.is_empty() {
        Some(base_event_type.to_string())
    } else {
        Some(format!("{base_event_type}|{}", modifiers.join("|")))
    }
}
