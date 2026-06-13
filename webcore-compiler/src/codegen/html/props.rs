use crate::core::ast::{Attribute, AttributeValue, Element};

/// Word-boundary-aware identifier substitution within an expression string.
/// Replaces `name` with `replacement` only when it is not adjacent to `[a-zA-Z0-9_$]`.
pub(super) fn replace_identifier(src: &str, name: &str, replacement: &str) -> String {
    if !src.contains(name) {
        return src.to_string();
    }
    let mut result = String::with_capacity(src.len() + 16);
    let mut remaining = src;
    loop {
        match remaining.find(name) {
            None => {
                result.push_str(remaining);
                break;
            }
            Some(pos) => {
                let before_ok = pos == 0
                    || remaining[..pos]
                        .chars()
                        .last()
                        .is_none_or(|c| !c.is_alphanumeric() && c != '_' && c != '$');
                let after_ok = remaining[pos + name.len()..]
                    .chars()
                    .next()
                    .is_none_or(|c| !c.is_alphanumeric() && c != '_' && c != '$');
                if before_ok && after_ok {
                    result.push_str(&remaining[..pos]);
                    result.push_str(replacement);
                    remaining = &remaining[pos + name.len()..];
                } else {
                    result.push_str(&remaining[..=pos]);
                    remaining = &remaining[pos + 1..];
                }
            }
        }
    }
    result
}

/// Substitutes prop identifiers into an expression string using the combined prop map.
///
/// The combined map encodes both static and dynamic props in one pass:
/// `(true, value)` = static prop (resolves to a literal string),
/// `(false, expr)` = dynamic prop (stays as a reactive expression).
///
/// Returns `Some((true, literal))` for a direct static match,
/// `Some((false, expr))` for a dynamic/compound substitution,
/// or `None` if the expression is unchanged.
pub(super) fn substitute_in_expr_combined(
    trimmed: &str,
    combined: &std::collections::HashMap<&str, (bool, &str)>,
) -> Option<(bool, String)> {
    // Direct exact match — cheapest path
    if let Some(&(is_static, val)) = combined.get(trimmed) {
        return Some((is_static, val.to_string()));
    }
    // Fast pre-check: skip String allocation when no prop name appears in expression
    if !combined.keys().any(|&k| trimmed.contains(k)) {
        return None;
    }
    // Compound expression: scan once for all identifier replacements
    let mut result = trimmed.to_string();
    for (&prop, &(is_static, val)) in combined {
        let replacement = if is_static {
            format!("({val})")
        } else {
            val.to_string()
        };
        result = replace_identifier(&result, prop, &replacement);
    }
    if result == trimmed {
        None
    } else {
        Some((false, result))
    }
}

/// Apply prop substitution to attribute values using the combined prop map.
pub(super) fn substitute_props_in_attrs_combined(
    attributes: &[Attribute],
    combined: &std::collections::HashMap<&str, (bool, &str)>,
) -> Vec<Attribute> {
    attributes
        .iter()
        .map(|attr| {
            let value = match &attr.value {
                AttributeValue::Expression(expr) => {
                    match substitute_in_expr_combined(expr.trim(), combined) {
                        Some((true, val)) => AttributeValue::String(val),
                        Some((false, e)) => AttributeValue::Expression(e),
                        None => attr.value.clone(),
                    }
                }
                other => other.clone(),
            };
            Attribute {
                name: attr.name.clone(),
                value,
                span: attr.span,
            }
        })
        .collect()
}

/// Build a combined prop map once and recurse using it.
/// Avoids two separate O(n_props) iterations per expression for every element.
pub(super) fn substitute_props(
    elements: &[Element],
    static_props: &std::collections::HashMap<String, String>,
    dynamic_props: &std::collections::HashMap<String, String>,
) -> Vec<Element> {
    // Build combined map once for the entire subtree traversal.
    let combined: std::collections::HashMap<&str, (bool, &str)> = static_props
        .iter()
        .map(|(k, v)| (k.as_str(), (true, v.as_str())))
        .chain(
            dynamic_props
                .iter()
                .map(|(k, v)| (k.as_str(), (false, v.as_str()))),
        )
        .collect();
    elements
        .iter()
        .map(|e| substitute_props_elem_combined(e, &combined))
        .collect()
}

pub(super) fn substitute_props_elem_combined(
    element: &Element,
    combined: &std::collections::HashMap<&str, (bool, &str)>,
) -> Element {
    match element {
        Element::Interpolation(expr, span) => {
            match substitute_in_expr_combined(expr.trim(), combined) {
                Some((true, val)) => Element::Text(val, *span),
                Some((false, new_expr)) => Element::Interpolation(new_expr, *span),
                None => element.clone(),
            }
        }
        Element::Tag {
            name,
            attributes,
            content,
            span,
        } => Element::Tag {
            name: name.clone(),
            attributes: substitute_props_in_attrs_combined(attributes, combined),
            content: elements_combined(content, combined),
            span: *span,
        },
        Element::Component {
            name,
            attributes,
            content,
            span,
        } => Element::Component {
            name: name.clone(),
            attributes: substitute_props_in_attrs_combined(attributes, combined),
            content: elements_combined(content, combined),
            span: *span,
        },
        Element::For {
            item,
            index,
            iterable,
            key,
            content,
            span,
        } => Element::For {
            item: item.clone(),
            index: index.clone(),
            iterable: iterable.clone(),
            key: key.clone(),
            content: elements_combined(content, combined),
            span: *span,
        },
        Element::If {
            condition,
            then_branch,
            else_branch,
            span,
        } => Element::If {
            condition: condition.clone(),
            then_branch: elements_combined(then_branch, combined),
            else_branch: else_branch
                .as_ref()
                .map(|eb| elements_combined(eb, combined)),
            span: *span,
        },
        Element::ErrorBlock {
            field,
            content,
            span,
        } => Element::ErrorBlock {
            field: field.clone(),
            content: elements_combined(content, combined),
            span: *span,
        },
        Element::SlotContent {
            name,
            content,
            span,
        } => Element::SlotContent {
            name: name.clone(),
            content: elements_combined(content, combined),
            span: *span,
        },
        _ => element.clone(),
    }
}

pub(super) fn elements_combined(
    elements: &[Element],
    combined: &std::collections::HashMap<&str, (bool, &str)>,
) -> Vec<Element> {
    elements
        .iter()
        .map(|e| substitute_props_elem_combined(e, combined))
        .collect()
}
