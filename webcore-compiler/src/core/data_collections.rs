//! Build-time data collections (#72).
//!
//! Expands `@for item in <data_import>` loops — where the iterable is a
//! build-time data import (`import projects from "data/projects.toml"`) — into
//! **static elements at the AST level**, before codegen. Each item's fields are
//! substituted into the loop body:
//!
//! - `{item.field}` interpolations become plain text (fully pre-rendered, no JS);
//! - `attr={item.field}` expression attributes become static string attributes;
//! - `item.field` / index references inside conditions, handlers and other
//!   expressions are replaced by the item's literal values.
//!
//! Because the result is ordinary static markup, the whole existing codegen
//! path (SSG `@if`, attributes, scoping) applies unchanged — the runtime never
//! sees these loops.
//!
//! Supported data shape: a JSON array, or (for TOML array-of-tables) an object
//! with a single array-valued field.

use crate::core::ast::{Attribute, AttributeValue, Element, WebCoreDocument};
use serde_json::Value;
use std::collections::BTreeMap;

/// Expand every data-backed `@for` in the document into static elements.
pub(crate) fn expand(document: &mut WebCoreDocument) {
    let arrays: BTreeMap<String, Vec<Value>> = document
        .data_imports
        .iter()
        .filter_map(|(name, json)| item_array(json).map(|a| (name.clone(), a)))
        .collect();
    if arrays.is_empty() {
        return;
    }
    for page in document.pages.values_mut() {
        expand_elements(&mut page.content, &arrays);
    }
    for comp in document.components.values_mut() {
        expand_elements(&mut comp.view, &arrays);
    }
    for layout in document.layouts.values_mut() {
        expand_elements(&mut layout.content, &arrays);
    }
}

/// Interpret a resolved data import as an array of items.
pub(crate) fn item_array(json: &str) -> Option<Vec<Value>> {
    match serde_json::from_str::<Value>(json).ok()? {
        Value::Array(a) => Some(a),
        // TOML array-of-tables → `{ "project": [ {...}, {...} ] }`.
        Value::Object(map) => {
            let mut arrays = map.values().filter(|v| v.is_array());
            match (arrays.next(), arrays.next()) {
                (Some(Value::Array(a)), None) => Some(a.clone()),
                _ => None,
            }
        }
        _ => None,
    }
}

fn expand_elements(elements: &mut Vec<Element>, arrays: &BTreeMap<String, Vec<Value>>) {
    let mut out: Vec<Element> = Vec::with_capacity(elements.len());
    for el in std::mem::take(elements) {
        match el {
            Element::For {
                item,
                index,
                iterable,
                content,
                ..
            } if arrays.contains_key(iterable.trim()) => {
                let items = &arrays[iterable.trim()];
                for (idx, obj) in items.iter().enumerate() {
                    let mut cloned = content.clone();
                    substitute(&mut cloned, &item, index.as_deref(), idx, obj);
                    // Handle data-backed @for nested inside this one.
                    expand_elements(&mut cloned, arrays);
                    out.extend(cloned);
                }
            }
            mut other => {
                recurse_children(&mut other, arrays);
                out.push(other);
            }
        }
    }
    *elements = out;
}

/// Recurse the expansion into an element's child element containers.
fn recurse_children(el: &mut Element, arrays: &BTreeMap<String, Vec<Value>>) {
    for child_vec in el.child_vecs_mut() {
        expand_elements(child_vec, arrays);
    }
}

/// Substitute `var`/`index_var` references throughout a cloned loop body.
///
/// Rebuilds the vector so an `@if` whose condition becomes a compile-time
/// literal after substitution is **folded** — replaced by its taken branch
/// (no runtime binding emitted).
fn substitute(
    elements: &mut Vec<Element>,
    var: &str,
    index_var: Option<&str>,
    idx: usize,
    obj: &Value,
) {
    let mut out: Vec<Element> = Vec::with_capacity(elements.len());
    for mut el in std::mem::take(elements) {
        match &mut el {
            Element::Interpolation(expr, span) => {
                if let Some(text) = resolve_pure(expr, var, index_var, idx, obj) {
                    out.push(Element::Text(text, *span));
                    continue;
                }
                *expr = substitute_tokens(expr, var, index_var, idx, obj);
            }
            Element::Tag {
                attributes,
                content,
                ..
            }
            | Element::Component {
                attributes,
                content,
                ..
            } => {
                for a in attributes.iter_mut() {
                    substitute_attr(a, var, index_var, idx, obj);
                }
                substitute(content, var, index_var, idx, obj);
            }
            Element::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                *condition = substitute_tokens(condition, var, index_var, idx, obj);
                substitute(then_branch, var, index_var, idx, obj);
                if let Some(eb) = else_branch {
                    substitute(eb, var, index_var, idx, obj);
                }
                // Fold when the condition resolved to a compile-time literal.
                if let Some(taken) = eval_literal_cond(condition) {
                    let branch = if taken {
                        std::mem::take(then_branch)
                    } else {
                        else_branch.as_mut().map(std::mem::take).unwrap_or_default()
                    };
                    out.extend(branch);
                    continue;
                }
            }
            Element::For {
                item: inner_item,
                index: inner_index,
                iterable,
                content,
                ..
            } => {
                // Nested `@for x in item.field` over a sub-array of the current
                // item → expand it inline (once per sub-element).
                if let Some(sub) = resolve_array_ref(iterable, var, obj) {
                    let inner_item = inner_item.clone();
                    let inner_index = inner_index.clone();
                    for (j, sub_obj) in sub.iter().enumerate() {
                        let mut c = content.clone();
                        // Resolve outer-item references first, then inner-loop ones.
                        substitute(&mut c, var, index_var, idx, obj);
                        substitute(&mut c, &inner_item, inner_index.as_deref(), j, sub_obj);
                        out.extend(c);
                    }
                    continue;
                }
                *iterable = substitute_tokens(iterable, var, index_var, idx, obj);
                substitute(content, var, index_var, idx, obj);
            }
            Element::Fragment { content, .. }
            | Element::Defer { content, .. }
            | Element::SlotContent { content, .. }
            | Element::ErrorBlock { content, .. } => substitute(content, var, index_var, idx, obj),
            Element::Text(_, _) | Element::Slot(_, _) | Element::Markdown(_, _) => {}
        }
        out.push(el);
    }
    *elements = out;
}

/// Evaluate a condition that has been reduced to literals (numbers, quoted
/// strings, `true`/`false`) after collection substitution. Returns `None` when
/// any operand is not a literal, so a still-dynamic `@if` stays a runtime
/// binding.
fn eval_literal_cond(cond: &str) -> Option<bool> {
    let c = cond.trim();
    // A bare literal folds on its truthiness (e.g. `@if item.thumb`, which after
    // substitution is `"thumb.png"` → true, or `""` → false).
    if let Some(t) = literal_truthiness(c) {
        return Some(t);
    }
    // Comparison operators, longest first so `>=`/`<=`/`==`/`!=` win over `>`/`<`.
    for op in ["==", "!=", ">=", "<=", ">", "<"] {
        if let Some((l, r)) = c.split_once(op) {
            let (l, r) = (literal(l.trim())?, literal(r.trim())?);
            // Equality compares numerically when both are numbers, else textually.
            let eq = match (l.parse::<f64>(), r.parse::<f64>()) {
                (Ok(a), Ok(b)) => a == b,
                _ => l == r,
            };
            return Some(match op {
                "==" => eq,
                "!=" => !eq,
                _ => {
                    // Ordering comparisons only make sense for numbers.
                    let (ln, rn) = (l.parse::<f64>().ok()?, r.parse::<f64>().ok()?);
                    match op {
                        ">=" => ln >= rn,
                        "<=" => ln <= rn,
                        ">" => ln > rn,
                        "<" => ln < rn,
                        _ => return None,
                    }
                }
            });
        }
    }
    None
}

/// Truthiness of a **single** literal condition (`true`/`false`/`null`, a lone
/// number, or a single quoted string). Returns `None` when `c` is not one bare
/// literal (e.g. a comparison like `"a" == "b"`), so the caller falls through to
/// operator handling.
fn literal_truthiness(c: &str) -> Option<bool> {
    match c {
        "true" => return Some(true),
        "false" | "null" => return Some(false),
        _ => {}
    }
    if is_single_string(c) {
        // Non-empty string is truthy; `""` (len 2) is falsy.
        return Some(c.len() > 2);
    }
    if let Ok(n) = c.parse::<f64>() {
        return Some(n != 0.0);
    }
    None
}

/// Whether `c` is exactly one quoted string (no unescaped closing quote before
/// the end), i.e. not a two-string comparison such as `"a" == "b"`.
fn is_single_string(c: &str) -> bool {
    let bytes = c.as_bytes();
    if bytes.len() < 2 {
        return false;
    }
    let q = bytes[0];
    if (q != b'"' && q != b'\'') || bytes[bytes.len() - 1] != q {
        return false;
    }
    let mut prev_backslash = false;
    for &b in &bytes[1..bytes.len() - 1] {
        if b == q && !prev_backslash {
            return false;
        }
        prev_backslash = b == b'\\' && !prev_backslash;
    }
    true
}

/// Normalise a literal operand to a comparable string, or `None` if it is not a
/// pure literal (number, quoted string, or boolean).
fn literal(tok: &str) -> Option<String> {
    if tok == "true" || tok == "false" || tok == "null" {
        return Some(tok.to_string());
    }
    if (tok.starts_with('"') && tok.ends_with('"') || tok.starts_with('\'') && tok.ends_with('\''))
        && tok.len() >= 2
    {
        return Some(tok[1..tok.len() - 1].to_string());
    }
    if tok.parse::<f64>().is_ok() {
        return Some(tok.to_string());
    }
    None
}

fn substitute_attr(
    attr: &mut Attribute,
    var: &str,
    index_var: Option<&str>,
    idx: usize,
    obj: &Value,
) {
    // Directive attributes keep their expression form (their value is code, not
    // a plain attribute string), so only substitute their tokens.
    let directive = attr.name.starts_with("on:")
        || attr.name.starts_with("class:")
        || attr.name.starts_with("bind:")
        || attr.name.starts_with("style:")
        || attr.name.starts_with("ref:")
        || attr.name.starts_with("validate:");
    match &attr.value {
        AttributeValue::Expression(e) => {
            if !directive {
                if let Some(text) = resolve_pure(e, var, index_var, idx, obj) {
                    attr.value = AttributeValue::String(text);
                    return;
                }
            }
            attr.value = AttributeValue::Expression(substitute_tokens(e, var, index_var, idx, obj));
        }
        AttributeValue::String(s) if s.contains('{') => {
            attr.value = AttributeValue::String(resolve_in_text(s, var, index_var, idx, obj));
        }
        _ => {}
    }
}

/// If `expr` is exactly a reference to the loop var (`item`, `item.a.b`) or the
/// index var, return its resolved display text. Otherwise `None`.
fn resolve_pure(
    expr: &str,
    var: &str,
    index_var: Option<&str>,
    idx: usize,
    obj: &Value,
) -> Option<String> {
    let e = expr.trim();
    if index_var == Some(e) {
        return Some(idx.to_string());
    }
    if e == var {
        return Some(value_to_text(obj));
    }
    let path = e.strip_prefix(var)?.strip_prefix('.')?;
    if path.is_empty()
        || !path
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '.')
    {
        return None;
    }
    Some(navigate(obj, path).map(value_to_text).unwrap_or_default())
}

/// Replace `var.path` and `index_var` tokens with their literal values,
/// skipping string-literal spans so text that merely contains the loop-var
/// name is left alone.
fn substitute_tokens(
    expr: &str,
    var: &str,
    index_var: Option<&str>,
    idx: usize,
    obj: &Value,
) -> String {
    let chars: Vec<char> = expr.chars().collect();
    let mut out = String::with_capacity(expr.len());
    let mut i = 0;
    let mut quote: Option<char> = None;
    while i < chars.len() {
        let c = chars[i];
        if let Some(q) = quote {
            out.push(c);
            if c == q && (i == 0 || chars[i - 1] != '\\') {
                quote = None;
            }
            i += 1;
            continue;
        }
        if c == '"' || c == '\'' || c == '`' {
            quote = Some(c);
            out.push(c);
            i += 1;
            continue;
        }
        if is_ident_start(c) {
            let start = i;
            while i < chars.len() && is_ident_part(chars[i]) {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            if word == var {
                let mut path = String::new();
                while i < chars.len() && chars[i] == '.' {
                    let seg_start = i + 1;
                    let mut j = seg_start;
                    while j < chars.len() && is_ident_part(chars[j]) {
                        j += 1;
                    }
                    if j == seg_start {
                        break;
                    }
                    if !path.is_empty() {
                        path.push('.');
                    }
                    path.extend(&chars[seg_start..j]);
                    i = j;
                }
                let literal = if path.is_empty() {
                    value_to_literal(obj)
                } else {
                    navigate(obj, &path)
                        .map(value_to_literal)
                        .unwrap_or_else(|| "null".to_string())
                };
                out.push_str(&literal);
            } else if index_var == Some(word.as_str()) {
                out.push_str(&idx.to_string());
            } else {
                out.push_str(&word);
            }
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

/// Resolve `{var.path}` / `{index}` interpolations inside a plain string
/// attribute value; other braces are left untouched.
fn resolve_in_text(s: &str, var: &str, index_var: Option<&str>, idx: usize, obj: &Value) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '{' {
            if let Some(close) = chars[i + 1..].iter().position(|&c| c == '}') {
                let inner: String = chars[i + 1..i + 1 + close].iter().collect();
                if let Some(text) = resolve_pure(inner.trim(), var, index_var, idx, obj) {
                    out.push_str(&text);
                    i += close + 2;
                    continue;
                }
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// If `iterable` is a pure reference to the current item (`item` or
/// `item.field.path`) that resolves to an array, return that array.
fn resolve_array_ref(iterable: &str, var: &str, obj: &Value) -> Option<Vec<Value>> {
    let it = iterable.trim();
    let v = if it == var {
        obj
    } else {
        let path = it.strip_prefix(var)?.strip_prefix('.')?;
        if path.is_empty()
            || !path
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '.')
        {
            return None;
        }
        navigate(obj, path)?
    };
    v.as_array().cloned()
}

fn navigate<'a>(mut v: &'a Value, path: &str) -> Option<&'a Value> {
    for seg in path.split('.') {
        v = v.get(seg)?;
    }
    Some(v)
}

/// Display text for interpolation/attribute output.
fn value_to_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

/// Code literal for substitution inside a live expression.
fn value_to_literal(v: &Value) -> String {
    match v {
        Value::String(s) => serde_json::to_string(s).unwrap_or_else(|_| "\"\"".to_string()),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        other => serde_json::to_string(other).unwrap_or_else(|_| "null".to_string()),
    }
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || c == '$'
}

fn is_ident_part(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '$'
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn item_array_accepts_json_array_and_toml_tables() {
        assert_eq!(item_array("[{\"a\":1}]").unwrap().len(), 1);
        assert_eq!(
            item_array("{\"project\":[{\"a\":1},{\"a\":2}]}")
                .unwrap()
                .len(),
            2
        );
        assert!(item_array("{\"a\":1}").is_none());
        assert!(item_array("42").is_none());
    }

    #[test]
    fn resolve_pure_handles_field_and_index() {
        let obj = json!({"title": "Ada", "meta": {"year": 1843}});
        assert_eq!(
            resolve_pure("item.title", "item", None, 0, &obj),
            Some("Ada".to_string())
        );
        assert_eq!(
            resolve_pure("item.meta.year", "item", None, 0, &obj),
            Some("1843".to_string())
        );
        assert_eq!(
            resolve_pure("i", "item", Some("i"), 3, &obj),
            Some("3".to_string())
        );
        // Not a pure reference → None (handled by token substitution instead).
        assert_eq!(resolve_pure("item.title + 1", "item", None, 0, &obj), None);
    }

    #[test]
    fn substitute_tokens_replaces_refs_but_not_strings() {
        let obj = json!({"id": 5, "featured": true});
        assert_eq!(
            substitute_tokens("select(item.id)", "item", None, 0, &obj),
            "select(5)"
        );
        assert_eq!(
            substitute_tokens("item.featured == true", "item", None, 0, &obj),
            "true == true"
        );
        // A string literal containing the loop-var name is untouched.
        assert_eq!(
            substitute_tokens("item.id + \" by item\"", "item", None, 0, &obj),
            "5 + \" by item\""
        );
    }

    #[test]
    fn eval_literal_cond_folds_constants_only() {
        assert_eq!(eval_literal_cond("true"), Some(true));
        assert_eq!(eval_literal_cond("false"), Some(false));
        assert_eq!(eval_literal_cond("true == true"), Some(true));
        assert_eq!(eval_literal_cond("5 > 3"), Some(true));
        assert_eq!(eval_literal_cond("2 >= 3"), Some(false));
        assert_eq!(eval_literal_cond("\"gold\" == \"vip\""), Some(false));
        assert_eq!(eval_literal_cond("5 == 5.0"), Some(true));
        // A non-literal operand → not foldable (stays a runtime binding).
        assert_eq!(eval_literal_cond("count > 3"), None);
        assert_eq!(eval_literal_cond("item.x == 1"), None);
    }

    #[test]
    fn eval_literal_cond_bare_truthiness() {
        // Bare string / number after substitution (`@if item.thumb`).
        assert_eq!(eval_literal_cond("\"thumb.png\""), Some(true));
        assert_eq!(eval_literal_cond("\"\""), Some(false));
        assert_eq!(eval_literal_cond("0"), Some(false));
        assert_eq!(eval_literal_cond("3"), Some(true));
        assert_eq!(eval_literal_cond("null"), Some(false));
        // A string comparison is not a single string literal.
        assert_eq!(eval_literal_cond("\"a\" == \"b\""), Some(false));
        assert_eq!(eval_literal_cond("\"a\" == \"a\""), Some(true));
    }

    #[test]
    fn resolve_array_ref_reads_item_subarray() {
        let obj = json!({"tags": ["a", "b"], "name": "x"});
        assert_eq!(
            resolve_array_ref("item.tags", "item", &obj),
            Some(vec![json!("a"), json!("b")])
        );
        // Non-array field or non-item reference → None.
        assert_eq!(resolve_array_ref("item.name", "item", &obj), None);
        assert_eq!(resolve_array_ref("other.tags", "item", &obj), None);
    }
}
