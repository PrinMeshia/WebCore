//! i18n consistency checks for `webc check` (#70).
//!
//! Two static guarantees over a project's `locales/*.toml`, both emitted as
//! **warnings** (they only fail the command under `--strict`):
//!
//! - **`i18n-parity`** — every locale should declare the same set of keys; a key
//!   present in one locale but missing from another is flagged.
//! - **`i18n-missing-key`** — every `t("key")` reference in the sources must
//!   resolve to a key declared in some locale (directly, or via its
//!   `key_one`/`key_other` plural variants).
//!
//! Projects with no `locales/` directory produce no diagnostics.

use crate::core::ast::{AttributeValue, Element, HeadBlock, WebCoreDocument};
use crate::core::diag::Diagnostic;
use std::collections::BTreeSet;

/// Run the i18n checks over the whole document.
pub(crate) fn lint(document: &WebCoreDocument) -> Vec<Diagnostic> {
    let mut issues = Vec::new();
    if document.locales.is_empty() {
        return issues;
    }

    // Union of every key declared across all locales.
    let all_keys: BTreeSet<&str> = document
        .locales
        .values()
        .flat_map(|m| m.keys().map(String::as_str))
        .collect();

    // ── Parity: flag keys missing from a locale that another locale declares ──
    for (code, entries) in &document.locales {
        for key in &all_keys {
            if !entries.contains_key(*key) {
                issues.push(Diagnostic::project_warning(
                    "i18n-parity",
                    format!("locale '{code}' is missing key '{key}' (declared in another locale)"),
                ));
            }
        }
    }

    // ── Referenced t("key") must resolve in at least one locale ───────────────
    let mut refs: BTreeSet<String> = BTreeSet::new();
    collect_doc(document, &mut refs);
    let resolves = |key: &str| {
        all_keys.contains(key)
            || all_keys.contains(&*format!("{key}_one"))
            || all_keys.contains(&*format!("{key}_other"))
    };
    for key in &refs {
        if !resolves(key) {
            issues.push(Diagnostic::project_warning(
                "i18n-missing-key",
                format!("t(\"{key}\") references a key defined in no locale"),
            ));
        }
    }

    issues
}

/// Collect every `t("key")` reference across pages, components, layouts and
/// their `head {}` blocks.
fn collect_doc(document: &WebCoreDocument, out: &mut BTreeSet<String>) {
    for page in document.pages.values() {
        walk(&page.content, out);
        if let Some(h) = &page.head {
            collect_head(h, out);
        }
    }
    for layout in document.layouts.values() {
        walk(&layout.content, out);
    }
    for comp in document.components.values() {
        walk(&comp.view, out);
    }
    if let Some(app) = &document.app {
        if let Some(h) = &app.head {
            collect_head(h, out);
        }
    }
}

fn collect_head(head: &HeadBlock, out: &mut BTreeSet<String>) {
    if let Some(t) = &head.title {
        extract_t_keys(t, out);
    }
    for meta in &head.metas {
        extract_t_keys(&meta.value, out);
        for (_, v) in &meta.extra {
            extract_t_keys(v, out);
        }
    }
    for (_, v) in &head.jsonld {
        collect_jsonld(v, out);
    }
}

/// Recurse a JSON-LD value, extracting `t("…")` keys from scalar strings.
fn collect_jsonld(v: &crate::core::ast::JsonLdValue, out: &mut BTreeSet<String>) {
    use crate::core::ast::JsonLdValue;
    match v {
        JsonLdValue::Str(s) => extract_t_keys(s, out),
        JsonLdValue::Array(items) => items.iter().for_each(|i| collect_jsonld(i, out)),
        JsonLdValue::Object(fields) => fields.iter().for_each(|(_, val)| collect_jsonld(val, out)),
    }
}

fn walk(elements: &[Element], out: &mut BTreeSet<String>) {
    crate::core::ast::walk_elements(elements, &mut |el| match el {
        Element::Interpolation(expr, _) => extract_t_keys(expr, out),
        Element::Tag { attributes, .. } | Element::Component { attributes, .. } => {
            for attr in attributes {
                match &attr.value {
                    AttributeValue::Expression(e) | AttributeValue::String(e) => {
                        extract_t_keys(e, out)
                    }
                    _ => {}
                }
            }
        }
        Element::For { iterable, .. } => extract_t_keys(iterable, out),
        _ => {}
    });
}

/// Pull every `t("key")` call out of an expression/string, keying on `t("` at a
/// call boundary (not preceded by an identifier char or `.`, so `format t(` or
/// `obj.t(` don't match the translation helper).
fn extract_t_keys(s: &str, out: &mut BTreeSet<String>) {
    let mut search = 0usize;
    while let Some(rel) = s[search..].find("t(\"") {
        let t_at = search + rel;
        let prev = s[..t_at].chars().next_back();
        let boundary = prev.is_none_or(|c| !c.is_alphanumeric() && c != '_' && c != '.');
        let key_start = t_at + 3; // past `t("`
        if let Some(end_rel) = s[key_start..].find('"') {
            if boundary {
                out.insert(s[key_start..key_start + end_rel].to_string());
            }
            search = key_start + end_rel + 1;
        } else {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(s: &str) -> Vec<String> {
        let mut set = BTreeSet::new();
        extract_t_keys(s, &mut set);
        set.into_iter().collect()
    }

    #[test]
    fn extracts_translation_keys_at_call_boundary() {
        assert_eq!(keys("t(\"welcome\")"), vec!["welcome"]);
        assert_eq!(keys("t(\"items\", count)"), vec!["items"]);
        assert_eq!(
            keys("t(\"a\") + \" \" + t(\"b\")"),
            vec!["a".to_string(), "b".to_string()]
        );
        // Not the translation helper — different identifier / method call.
        assert!(keys("format(\"x\")").is_empty());
        assert!(keys("obj.t(\"x\")").is_empty());
    }
}
