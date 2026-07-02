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
            AttributeValue::Spread(_) => {
                // Spread is handled by tags.rs / bindAttrs; skip here
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

/// Derive a safe prefix from a name (lowercase alphanumeric, max 12 chars).
///
/// The prefix opens every generated element id (`{prefix}e0`, `{prefix}btn1`, …),
/// and those ids are emitted as **unquoted JS object keys** in the handler/expr
/// maps. A key must therefore be a valid JS identifier start: a name like `404`
/// would otherwise yield `404e0:` — a syntax error. When the sanitized prefix
/// starts with a digit we prepend `p` (e.g. `404` → `p404`).
pub(super) fn safe_id_prefix(name: &str) -> String {
    let s: String = name
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .take(12)
        .collect();
    match s.chars().next() {
        None => "p".to_string(),
        Some(c) if c.is_ascii_digit() => format!("p{s}"),
        _ => s,
    }
}

#[cfg(test)]
mod prefix_tests {
    use super::safe_id_prefix;

    #[test]
    fn strips_non_alphanumeric_and_lowercases() {
        assert_eq!(safe_id_prefix("HomePage"), "homepage");
        assert_eq!(safe_id_prefix("my page!"), "mypage");
    }

    #[test]
    fn empty_falls_back_to_p() {
        assert_eq!(safe_id_prefix(""), "p");
        assert_eq!(safe_id_prefix("---"), "p");
    }

    #[test]
    fn digit_leading_names_are_valid_js_identifiers() {
        // "404" as an unquoted object key (404e0:) is a JS syntax error.
        assert_eq!(safe_id_prefix("404"), "p404");
        assert_eq!(safe_id_prefix("2fa"), "p2fa");
        assert!(!safe_id_prefix("404")
            .chars()
            .next()
            .unwrap()
            .is_ascii_digit());
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
