//! AST pre-pass: scope each component's reactive state to a unique key namespace.
//!
//! The WebCore runtime uses a single, page-global reactive store keyed by the
//! state-variable name (`S.get('open')`). Two components that each declare a
//! state var of the same name therefore collide — mutating one silently mutates
//! the other (e.g. a burger menu that also toggled a command palette).
//!
//! This pass runs once, after component resolution and before codegen. For every
//! component it rewrites all references to that component's *own* state and
//! computed vars to a globally-unique key `<Component>__<var>`. Because the rest
//! of the codegen keys off these names, it then produces non-colliding keys with
//! no further changes. Component-local `on:mount` code (which addresses the store
//! directly via `S.get('x')` / `S.set('x', …)`) is rewritten too.
//!
//! The global `$store` (`document.store`) is intentionally shared and left alone.

use crate::core::ast::{AttributeValue, Element, WebCoreDocument};
use regex::Regex;

/// Rewrite every component's local state/computed references into a per-component
/// key namespace, so identically-named state in different components no longer
/// collide in the shared runtime store.
pub(crate) fn scope_component_state(document: &mut WebCoreDocument) {
    let names: Vec<String> = document.components.keys().cloned().collect();
    for cname in names {
        let comp = match document.components.get_mut(&cname) {
            Some(c) => c,
            None => continue,
        };
        if comp.state.is_empty() && comp.computed.is_empty() {
            continue;
        }
        let prefix = sanitize(&cname);

        // Rename map (old → new), longest name first so a shorter name never
        // clobbers inside a longer one during whole-word replacement.
        let mut renames: Vec<(String, String)> = Vec::new();
        for sv in &comp.state {
            renames.push((sv.name.clone(), format!("{prefix}__{}", sv.name)));
        }
        for cv in &comp.computed {
            renames.push((cv.name.clone(), format!("{prefix}__{}", cv.name)));
        }
        renames.sort_by_key(|b| std::cmp::Reverse(b.0.len()));

        // 1. Declarations
        for sv in comp.state.iter_mut() {
            sv.name = rename_exact(&sv.name, &renames);
        }
        for cv in comp.computed.iter_mut() {
            cv.name = rename_exact(&cv.name, &renames);
            cv.expr = rewrite_expr(&cv.expr, &renames);
        }

        // 2. http `into:` target + `$watch` targets reference a local state var
        if let Some(http) = comp.http.as_mut() {
            http.into = rename_exact(&http.into, &renames);
        }
        for w in comp.watch_hooks.iter_mut() {
            w.var = rename_exact(&w.var, &renames);
        }

        // 3. View expressions (interpolations, conditions, attribute values…)
        rewrite_elements(&mut comp.view, &renames);

        // 4. Raw JS bodies address the store directly: `S.get('x')`, `S.set('x',…)`
        if let Some(body) = comp.mount_body.as_mut() {
            *body = rewrite_store_calls(body, &renames);
        }
        if let Some(body) = comp.destroy_body.as_mut() {
            *body = rewrite_store_calls(body, &renames);
        }
    }
}

/// Keep only identifier-safe characters so the prefix is a valid JS token.
fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Exact-match rename for a bare identifier field (declaration name, watch var…).
fn rename_exact(name: &str, renames: &[(String, String)]) -> String {
    for (old, new) in renames {
        if old == name {
            return new.clone();
        }
    }
    name.to_string()
}

/// Whole-word replace each `old` identifier with `new` in an expression string,
/// protecting property accesses (`obj.open`) so only bare references are renamed.
/// Mirrors the semantics of `CompiledVars::replace_into`.
fn rewrite_expr(expr: &str, renames: &[(String, String)]) -> String {
    let mut result = expr.to_owned();
    for (old, new) in renames {
        // Protect `.old` (property access) — a word-char sentinel prevents the
        // `\bold\b` word boundary from matching inside it.
        let prop_pat = format!(".{old}");
        let sentinel = format!(".__WCSCOPE_{old}__");
        let protected = result.replace(&prop_pat, &sentinel);
        let re = match Regex::new(&format!(r"\b{}\b", regex::escape(old))) {
            Ok(re) => re,
            Err(_) => continue,
        };
        let replaced = re.replace_all(&protected, new.as_str()).into_owned();
        result = replaced.replace(&sentinel, &prop_pat);
    }
    result
}

/// Rewrite direct store calls in raw JS: `S.get('x')`, `S.set('x', …)`,
/// `S.setQ('x', …)`, `S.on('x', …)` — for both quote styles.
fn rewrite_store_calls(body: &str, renames: &[(String, String)]) -> String {
    let mut result = body.to_owned();
    for (old, new) in renames {
        for q in ['\'', '"'] {
            let pat = format!(r"(S\.(?:get|set|setQ|on)\(\s*){q}{}{q}", regex::escape(old));
            let re = match Regex::new(&pat) {
                Ok(re) => re,
                Err(_) => continue,
            };
            let rep = format!("${{1}}{q}{new}{q}");
            result = re.replace_all(&result, rep.as_str()).into_owned();
        }
    }
    result
}

/// Recursively rewrite state references inside a list of view elements.
fn rewrite_elements(elements: &mut [Element], renames: &[(String, String)]) {
    crate::core::ast::walk_elements_mut(elements, &mut |el| match el {
        Element::Interpolation(expr, _) => {
            *expr = rewrite_expr(expr, renames);
        }
        Element::Tag { attributes, .. } | Element::Component { attributes, .. } => {
            for attr in attributes.iter_mut() {
                match &mut attr.value {
                    AttributeValue::Expression(e) => *e = rewrite_expr(e, renames),
                    AttributeValue::Spread(v) => *v = rename_exact(v, renames),
                    AttributeValue::String(_) | AttributeValue::Boolean(_) => {}
                }
            }
        }
        Element::For { iterable, key, .. } => {
            *iterable = rewrite_expr(iterable, renames);
            if let Some(k) = key.as_mut() {
                *k = rewrite_expr(k, renames);
            }
        }
        Element::If { condition, .. } => {
            *condition = rewrite_expr(condition, renames);
        }
        _ => {}
    });
}
