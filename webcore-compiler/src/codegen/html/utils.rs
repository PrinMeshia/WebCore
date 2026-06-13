use crate::core::ast::{Attribute, AttributeValue};
use std::fmt::Write as _;

/// HTML void elements: they cannot have children and must not get a closing tag.
/// <https://html.spec.whatwg.org/multipage/syntax.html#void-elements>
const VOID_ELEMENTS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "source", "track",
    "wbr",
];

/// True if `name` is an HTML void element (no closing tag allowed).
pub(super) fn is_void_element(name: &str) -> bool {
    VOID_ELEMENTS.contains(&name)
}

/// Append the closing tag for `name`.
///
/// Single emission point for closing tags: void elements must never reach
/// this function (callers skip them after checking [`is_void_element`]).
pub(super) fn push_close_tag(result: &mut String, name: &str) {
    debug_assert!(
        !is_void_element(name),
        "void element <{name}> must not get a closing tag"
    );
    write!(result, "</{name}>").expect("write! to String is infallible");
}

/// Append attributes that need no special handling: static strings, boolean
/// flags, and dynamic expressions (emitted as `data-webcore-attr-*` for
/// `bindAttrs`). Event/class/style/validation attributes are handled by the
/// dedicated `attrs` module and must be filtered out by the caller.
pub(super) fn push_plain_attributes(result: &mut String, attributes: &[Attribute]) {
    for attr in attributes {
        match &attr.value {
            AttributeValue::String(value) => {
                write!(result, " {}=\"{}\"", attr.name, html_escape(value))
                    .expect("write! to String is infallible");
            }
            AttributeValue::Boolean(true) => {
                write!(result, " {}", attr.name).expect("write! to String is infallible");
            }
            AttributeValue::Boolean(false) => {}
            AttributeValue::Expression(expr) => {
                write!(
                    result,
                    " data-webcore-attr-{}=\"{}\"",
                    attr.name,
                    html_escape(expr)
                )
                .expect("write! to String is infallible");
            }
        }
    }
}

/// HTML-escape a string (& < > " ').
pub(super) fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

/// Derive a safe HTML-id-compatible prefix from a name (lowercase alphanumeric, max 12 chars).
pub(super) fn safe_id_prefix(name: &str) -> String {
    let s: String = name
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .take(12)
        .collect();
    if s.is_empty() {
        "p".to_string()
    } else {
        s
    }
}

/// Extract path from `webcore_navigate(path)` expression
pub(super) fn extract_navigate_path(expr: &str) -> Option<String> {
    // Match webcore_navigate(/path) or webcore_navigate(root) or webcore_navigate("/path")
    let expr = expr.trim();

    if let Some(start) = expr.find("webcore_navigate(") {
        let after_paren = &expr[start + 17..]; // After "webcore_navigate("
        if let Some(end) = after_paren.find(')') {
            let path = after_paren[..end].trim();

            // Handle different path formats
            let clean_path = if path == "root" {
                "/".to_string()
            } else if path.starts_with('"') && path.ends_with('"') {
                // Quoted path: "/about"
                path[1..path.len() - 1].to_string()
            } else if path.starts_with('/') {
                // Unquoted path: /about
                path.to_string()
            } else {
                // Fallback
                format!("/{path}")
            };

            return Some(clean_path);
        }
    }
    None
}
