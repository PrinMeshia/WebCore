//! Static Site Generation: compile-time evaluation of initial state values.
//!
//! The HTML emitters fill `<span data-webcore-interpolation="expr">` with the
//! initial computed value and set the correct `display` style on `@if`/`@else`
//! divs (see `codegen/html/elements.rs`), eliminating flash-of-wrong-content
//! on first load. This module provides the evaluation context they use.
//!
//! The JS runtime (`bindIf`, `bind`) overwrites these values reactively after
//! `DOMContentLoaded`, so SSG is fully compatible with the existing runtime.

use crate::core::ast::WebCoreDocument;
use std::collections::BTreeMap;

/// Compile-time evaluation context for SSG pre-rendering, passed (optionally)
/// to the HTML generators. `state` already contains per-page additions such
/// as `$route.<param>` values for SSG collections.
pub(crate) struct SsgContext<'a> {
    pub state: &'a BTreeMap<String, String>,
    pub locales: &'a BTreeMap<String, BTreeMap<String, String>>,
    pub locale: &'a str,
}

impl SsgContext<'_> {
    /// Evaluate an interpolation expression to its initial text, if statically known.
    pub(crate) fn eval_expr(&self, expr: &str) -> Option<String> {
        eval_expr_with_locale(expr, self.state, self.locales, self.locale)
    }

    /// Evaluate an `@if` condition against the initial state, if statically known.
    pub(crate) fn eval_cond(&self, cond: &str) -> Option<bool> {
        eval_cond_initial(cond, self.state)
    }
}

/// Collect the initial default value of every state/store variable.
///
/// Store vars are inserted first; component-state vars use `entry().or_insert`
/// so the first-seen value wins when multiple components declare the same name.
pub(crate) fn build_initial_state(document: &WebCoreDocument) -> BTreeMap<String, String> {
    let mut state: BTreeMap<String, String> = BTreeMap::new();

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

    state
}

/// Single-pass HTML escape for plain text (handles `&`, `<`, `>`).
pub(crate) fn html_escape_text(text: &str) -> String {
    if !text.contains(['&', '<', '>']) {
        return text.to_string();
    }
    let mut result = String::with_capacity(text.len() + 16);
    for c in text.chars() {
        match c {
            '&' => result.push_str("&amp;"),
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            _ => result.push(c),
        }
    }
    result
}

/// Resolve a single token (variable name or numeric literal) to f64.
fn resolve_number(token: &str, state: &BTreeMap<String, String>) -> Option<f64> {
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
fn eval_number_expr(expr: &str, state: &BTreeMap<String, String>) -> Option<f64> {
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
pub(crate) fn eval_cond_initial(cond: &str, state: &BTreeMap<String, String>) -> Option<bool> {
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
    state: &BTreeMap<String, String>,
    locales: &BTreeMap<String, BTreeMap<String, String>>,
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

    // .length on a state variable
    if let Some(var_name) = expr.strip_suffix(".length") {
        if let Some(val) = state.get(var_name.trim()) {
            let trimmed = val.trim();
            if trimmed.starts_with('[') {
                if trimmed == "[]" {
                    return Some("0".to_string());
                }
                // Parse as JSON so quoted commas don't inflate the count.
                if let Ok(serde_json::Value::Array(arr)) = serde_json::from_str(trimmed) {
                    return Some(arr.len().to_string());
                }
                // Fallback: naive comma-split (only reached for non-JSON arrays).
                let count = trimmed[1..trimmed.len() - 1].split(',').count();
                return Some(count.to_string());
            }
            // chars().count() for Unicode-correct length (byte len is wrong for multibyte).
            return Some(val.chars().count().to_string());
        }
    }

    // .toUpperCase() on a state variable
    if let Some(var_name) = expr.strip_suffix(".toUpperCase()") {
        if let Some(val) = state.get(var_name.trim()) {
            return Some(val.to_uppercase());
        }
    }

    // .toLowerCase() on a state variable
    if let Some(var_name) = expr.strip_suffix(".toLowerCase()") {
        if let Some(val) = state.get(var_name.trim()) {
            return Some(val.to_lowercase());
        }
    }

    // .trim() on a state variable
    if let Some(var_name) = expr.strip_suffix(".trim()") {
        if let Some(val) = state.get(var_name.trim()) {
            return Some(val.trim().to_string());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
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
            eval_expr_with_locale("count", &s, &BTreeMap::new(), ""),
            Some("42".into())
        );
    }

    #[test]
    fn test_eval_expr_arithmetic() {
        let s = state(&[("count", "3")]);
        assert_eq!(
            eval_expr_with_locale("count + 1", &s, &BTreeMap::new(), ""),
            Some("4".into())
        );
    }

    #[test]
    fn test_ssg_context_eval_expr() {
        let s = state(&[("count", "7")]);
        let locales = BTreeMap::new();
        let ctx = SsgContext {
            state: &s,
            locales: &locales,
            locale: "",
        };
        assert_eq!(ctx.eval_expr("count"), Some("7".into()));
        assert_eq!(ctx.eval_expr("unknown"), None);
    }

    #[test]
    fn test_ssg_context_eval_cond() {
        let s = state(&[("count", "0")]);
        let locales = BTreeMap::new();
        let ctx = SsgContext {
            state: &s,
            locales: &locales,
            locale: "",
        };
        assert_eq!(ctx.eval_cond("count > 0"), Some(false));
        assert_eq!(ctx.eval_cond("count == 0"), Some(true));
        assert_eq!(ctx.eval_cond("foo > 0"), None);
    }
}
