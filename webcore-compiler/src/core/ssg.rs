//! Static Site Generation: pre-render initial state values into HTML.
//!
//! Fills `<span data-webcore-interpolation="expr">` with the initial computed
//! value and sets correct `display` style on `@if`/`@else` divs based on
//! initial state — eliminating flash-of-wrong-content on first load.
//!
//! The JS runtime (`bindIf`, `bind`) overwrites these values reactively after
//! `DOMContentLoaded`, so SSG is fully compatible with the existing runtime.

use crate::core::ast::WebCoreDocument;
use regex::Regex;
use std::collections::HashMap;
use std::fmt::Write as _;

/// Collect the initial default value of every state/store variable.
///
/// Store vars are inserted first; component-state vars use `entry().or_insert`
/// so the first-seen value wins when multiple components declare the same name.
pub(crate) fn build_initial_state(document: &WebCoreDocument) -> HashMap<String, String> {
    let mut state: HashMap<String, String> = HashMap::new();

    for var in &document.store {
        if let Some(val) = &var.default_value {
            state.entry(var.name.clone()).or_insert_with(|| val.clone());
        }
    }

    for component in document.components.values() {
        for var in &component.state {
            if let Some(val) = &var.default_value {
                state.entry(var.name.clone()).or_insert_with(|| val.clone());
            }
        }
    }

    // Data imports are available as initial state for SSG
    for (name, json) in &document.data_imports {
        state.entry(name.clone()).or_insert_with(|| json.trim().to_string());
    }

    state
}

use crate::core::utils::{html_escape, html_unescape};



/// Resolve a single token (variable name or numeric literal) to f64.
fn resolve_number(token: &str, state: &HashMap<String, String>) -> Option<f64> {
    let token = token.trim();
    if let Ok(n) = token.parse::<f64>() {
        return Some(n);
    }
    if let Some(val) = state.get(token) {
        if let Ok(n) = val.parse::<f64>() {
            return Some(n);
        }
    }
    None
}

/// Evaluate a simple arithmetic expression (a ± b, a * b, literal, or variable).
/// Returns `None` for anything more complex (function calls, nested parens, etc.).
fn eval_number_expr(expr: &str, state: &HashMap<String, String>) -> Option<f64> {
    let expr = expr.trim();
    if let Some(n) = resolve_number(expr, state) {
        return Some(n);
    }
    // Use rfind so left-to-right precedence is preserved for chained ops.
    if let Some(idx) = expr.rfind(" + ") {
        let l = eval_number_expr(&expr[..idx], state)?;
        let r = eval_number_expr(&expr[idx + 3..], state)?;
        return Some(l + r);
    }
    if let Some(idx) = expr.rfind(" - ") {
        let l = eval_number_expr(&expr[..idx], state)?;
        let r = eval_number_expr(&expr[idx + 3..], state)?;
        return Some(l - r);
    }
    if let Some(idx) = expr.rfind(" * ") {
        let l = eval_number_expr(&expr[..idx], state)?;
        let r = eval_number_expr(&expr[idx + 3..], state)?;
        return Some(l * r);
    }
    None
}

/// Evaluate a simple boolean condition (a OP b, or truthy variable check).
///
/// The `cond` argument must already be HTML-unescaped.
/// Returns `None` when the condition is too complex to evaluate statically.
pub(crate) fn eval_cond_initial(cond: &str, state: &HashMap<String, String>) -> Option<bool> {
    let cond = cond.trim();

    // Check multi-char operators before single-char ones to avoid mis-parsing.
    type CmpFn = fn(f64, f64) -> bool;
    let ops: &[(&str, CmpFn)] = &[
        (">=", |a, b| a >= b),
        ("<=", |a, b| a <= b),
        ("!=", |a, b| (a - b).abs() > f64::EPSILON),
        ("==", |a, b| (a - b).abs() <= f64::EPSILON),
        (">", |a, b| a > b),
        ("<", |a, b| a < b),
    ];

    for (op, f) in ops {
        if let Some(idx) = cond.find(op) {
            let left = eval_number_expr(cond[..idx].trim(), state)?;
            let right = eval_number_expr(cond[idx + op.len()..].trim(), state)?;
            return Some(f(left, right));
        }
    }

    // Bare variable — truthy if non-zero / non-false / non-empty
    if let Some(val) = state.get(cond) {
        return Some(!matches!(val.as_str(), "0" | "false" | ""));
    }

    None
}

/// Evaluate a simple expression to its text representation for SSG interpolation.
///
/// Handles: direct variable, string literal, simple arithmetic, `t("key")` calls.
/// `locales` and `locale` are used for `t()` resolution; pass empty values if not applicable.
pub(crate) fn eval_expr_with_locale(
    expr: &str,
    state: &HashMap<String, String>,
    locales: &HashMap<String, HashMap<String, String>>,
    locale: &str,
) -> Option<String> {
    let expr = expr.trim();

    // t("key") — look up in the active locale
    if let Some(rest) = expr.strip_prefix("t(\"") {
        if let Some(key) = rest.strip_suffix("\")") {
            return locales.get(locale)?.get(key).cloned();
        }
    }

    if let Some(val) = state.get(expr) {
        return Some(val.clone());
    }

    if expr.starts_with('"') && expr.ends_with('"') && expr.len() >= 2 {
        return Some(expr[1..expr.len() - 1].to_string());
    }

    if let Some(n) = eval_number_expr(expr, state) {
        if n == n.floor() && n.abs() < 1e15 {
            return Some(format!("{}", n as i64));
        }
        return Some(format!("{n}"));
    }

    // Property access: items.length, name.toUpperCase(), etc.
    // Use rfind so `a.b.length` finds the last dot correctly.
    if let Some(dot_pos) = expr.rfind('.') {
        let obj_part = expr[..dot_pos].trim();
        let prop = expr[dot_pos + 1..].trim();
        // Skip $store.x / $route.x / $query.x — handled elsewhere
        if !obj_part.starts_with('$') {
            if let Some(obj_val) = eval_expr_with_locale(obj_part, state, locales, locale) {
                let t = obj_val.trim();
                return match prop {
                    "length" => {
                        if t == "[]" || t.is_empty() {
                            Some("0".to_string())
                        } else if t.starts_with('[') && t.ends_with(']') {
                            let inner = &t[1..t.len() - 1];
                            if inner.trim().is_empty() {
                                Some("0".to_string())
                            } else {
                                Some(inner.split(',').count().to_string())
                            }
                        } else {
                            Some(obj_val.chars().count().to_string())
                        }
                    }
                    "toUpperCase()" => Some(obj_val.to_uppercase()),
                    "toLowerCase()" => Some(obj_val.to_lowercase()),
                    "trim()" => Some(obj_val.trim().to_string()),
                    _ => None,
                };
            }
        }
    }

    None
}

/// Post-process generated HTML to pre-render initial state (SSG) with i18n support.
///
/// Steps:
/// 1. Fill empty `<span data-webcore-interpolation="expr"></span>` with the
///    initial computed value (including `t("key")` translations).
/// 2. Add `style="display:block/none"` to `<div data-webcore-if="...">` and
///    `<div data-webcore-else="...">` so the correct branch is shown before JS
///    runs `bindIf()` at `DOMContentLoaded`.
pub(crate) fn apply_ssg_with_locales(
    html: &str,
    state: &HashMap<String, String>,
    locales: &HashMap<String, HashMap<String, String>>,
    default_locale: &str,
) -> String {
    let mut result = html.to_string();

    // ── 1. Interpolation spans ───────────────────────────────────────────────
    // Pattern matches the attribute + the closing ></span> of an empty span.
    let interp_re = Regex::new(r#"(data-webcore-interpolation="([^"]+)")></span>"#).unwrap();
    let mut out = String::new();
    let mut last = 0usize;
    for cap in interp_re.captures_iter(&result) {
        let m = cap.get(0).unwrap();
        out.push_str(&result[last..m.start()]);
        let full_attr = &cap[1]; // data-webcore-interpolation="EXPR"
        let raw_expr = &cap[2]; // the raw (HTML-escaped) expression
        let expr = html_unescape(raw_expr);
        if let Some(val) = eval_expr_with_locale(&expr, state, locales, default_locale) {
            write!(out, "{}>{}</span>", full_attr, html_escape(&val)).unwrap();
        } else {
            out.push_str(m.as_str());
        }
        last = m.end();
    }
    out.push_str(&result[last..]);
    result = out;

    // ── 2. @if divs ─────────────────────────────────────────────────────────
    let if_re = Regex::new(r#"<div data-webcore-if="([^"]+)"([^>]*)>"#).unwrap();
    let mut out = String::new();
    let mut last = 0usize;
    for cap in if_re.captures_iter(&result) {
        let m = cap.get(0).unwrap();
        out.push_str(&result[last..m.start()]);
        let raw_cond = &cap[1];
        let rest_attrs = &cap[2];
        let cond = html_unescape(raw_cond);
        let style = match eval_cond_initial(&cond, state) {
            Some(true) => " style=\"display:block\"",
            Some(false) => " style=\"display:none\"",
            None => "",
        };
        write!(out, r#"<div data-webcore-if="{raw_cond}"{rest_attrs}{style}>"#).unwrap();
        last = m.end();
    }
    out.push_str(&result[last..]);
    result = out;

    // ── 3. @else divs (inverted condition) ──────────────────────────────────
    let else_re = Regex::new(r#"<div data-webcore-else="([^"]+)"([^>]*)>"#).unwrap();
    let mut out = String::new();
    let mut last = 0usize;
    for cap in else_re.captures_iter(&result) {
        let m = cap.get(0).unwrap();
        out.push_str(&result[last..m.start()]);
        let raw_cond = &cap[1];
        let rest_attrs = &cap[2];
        let cond = html_unescape(raw_cond);
        let style = match eval_cond_initial(&cond, state) {
            Some(true) => " style=\"display:none\"", // condition true → else hidden
            Some(false) => " style=\"display:block\"", // condition false → else shown
            None => "",
        };
        write!(out, r#"<div data-webcore-else="{raw_cond}"{rest_attrs}{style}>"#).unwrap();
        last = m.end();
    }
    out.push_str(&result[last..]);
    result = out;

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn test_eval_cond_gt() {
        let s = state(&[("count", "5")]);
        assert_eq!(eval_cond_initial("count > 0", &s), Some(true));
        assert_eq!(eval_cond_initial("count > 10", &s), Some(false));
    }

    #[test]
    fn test_eval_cond_eq() {
        let s = state(&[("count", "0")]);
        assert_eq!(eval_cond_initial("count == 0", &s), Some(true));
        assert_eq!(eval_cond_initial("count == 1", &s), Some(false));
    }

    #[test]
    fn test_eval_cond_unknown_var() {
        let s = state(&[]);
        assert_eq!(eval_cond_initial("foo > 0", &s), None);
    }

    #[test]
    fn test_eval_expr_direct_var() {
        let s = state(&[("count", "42")]);
        assert_eq!(
            eval_expr_with_locale("count", &s, &HashMap::new(), ""),
            Some("42".into())
        );
    }

    #[test]
    fn test_eval_expr_arithmetic() {
        let s = state(&[("count", "3")]);
        assert_eq!(
            eval_expr_with_locale("count + 1", &s, &HashMap::new(), ""),
            Some("4".into())
        );
    }

    #[test]
    fn test_apply_ssg_fills_interpolation() {
        let s = state(&[("count", "7")]);
        let html = r#"<span data-webcore-interpolation="count"></span>"#;
        let out = apply_ssg_with_locales(html, &s, &HashMap::new(), "");
        assert!(
            out.contains(r#"data-webcore-interpolation="count">7</span>"#),
            "{}",
            out
        );
    }

    #[test]
    fn test_apply_ssg_if_display_block() {
        let s = state(&[("count", "5")]);
        let html = r#"<div data-webcore-if="count &gt; 0">"#;
        let out = apply_ssg_with_locales(html, &s, &HashMap::new(), "");
        assert!(out.contains(r#"style="display:block""#), "{}", out);
    }

    #[test]
    fn test_apply_ssg_if_display_none() {
        let s = state(&[("count", "0")]);
        let html = r#"<div data-webcore-if="count &gt; 0">"#;
        let out = apply_ssg_with_locales(html, &s, &HashMap::new(), "");
        assert!(out.contains(r#"style="display:none""#), "{}", out);
    }

    #[test]
    fn test_apply_ssg_else_inverted() {
        let s = state(&[("count", "0")]);
        let html = r#"<div data-webcore-else="count &gt; 0">"#;
        let out = apply_ssg_with_locales(html, &s, &HashMap::new(), "");
        assert!(out.contains(r#"style="display:block""#), "{}", out);
    }

    #[test]
    fn test_apply_ssg_unknown_var_no_style() {
        let s = state(&[]);
        let html = r#"<div data-webcore-if="foo &gt; 0">"#;
        let out = apply_ssg_with_locales(html, &s, &HashMap::new(), "");
        assert!(!out.contains("style="), "{}", out);
    }
}
