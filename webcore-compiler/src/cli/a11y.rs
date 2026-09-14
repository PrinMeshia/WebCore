//! Accessibility lints for `webc check --a11y` (#49).
//!
//! Statically detectable RGAA/WCAG issues, walked over every page, layout and
//! component view. Rules are deliberately conservative (no false positives):
//!
//! - **`a11y-img-alt`** (RGAA 1.1) — `<img>` with no textual alternative
//!   (`alt`, `aria-label`, `aria-labelledby`, `title`) and not marked decorative
//!   (`aria-hidden`, `role="presentation"`/`"none"`). An empty `alt=""` counts
//!   as a valid decorative alternative and is accepted.
//! - **`a11y-label-for`** (RGAA 11.1) — `<label>` with neither a `for` attribute
//!   nor a wrapped form control.
//! - **`a11y-control-name`** (RGAA 6.1) — `<a>`/`<button>` with no accessible
//!   name: no visible text/interpolation, no labelled child image, and no
//!   `aria-label`/`aria-labelledby`/`title`.

use crate::core::ast::{Attribute, AttributeValue, Element, Span, WebCoreDocument};
use crate::core::diag::{Diagnostic, Severity};
use std::path::Path;

/// Run the accessibility lints over the whole document.
pub(crate) fn lint(document: &WebCoreDocument) -> Vec<Diagnostic> {
    let mut issues = Vec::new();
    let file_of = |name: &str| document.source_files.get(name).map(|p| p.as_path());
    for (name, page) in &document.pages {
        walk(&page.content, file_of(name), &mut issues);
    }
    for (name, layout) in &document.layouts {
        walk(&layout.content, file_of(name), &mut issues);
    }
    for (name, comp) in &document.components {
        walk(&comp.view, file_of(name), &mut issues);
    }
    issues
}

fn walk(elements: &[Element], file: Option<&Path>, issues: &mut Vec<Diagnostic>) {
    crate::core::ast::walk_elements(elements, &mut |el| {
        if let Element::Tag {
            name,
            attributes,
            content,
            span,
        } = el
        {
            check_tag(name, attributes, content, *span, file, issues);
        }
    });
}

fn check_tag(
    name: &str,
    attrs: &[Attribute],
    content: &[Element],
    span: Span,
    file: Option<&Path>,
    issues: &mut Vec<Diagnostic>,
) {
    match name.to_ascii_lowercase().as_str() {
        "img" => {
            let decorative =
                has_attr(attrs, "aria-hidden") || role_in(attrs, &["presentation", "none"]);
            let named = has_attr(attrs, "alt")
                || has_attr(attrs, "aria-label")
                || has_attr(attrs, "aria-labelledby")
                || has_attr(attrs, "title");
            if !named && !decorative {
                issues.push(diag(
                    "a11y-img-alt",
                    "<img> sans alternative textuelle (alt, aria-label, aria-labelledby ou title) — RGAA 1.1".to_string(),
                    span,
                    file,
                ));
            }
        }
        "label" if !has_attr(attrs, "for") && !any_tag(content, |n, _| is_form_control(n)) => {
            issues.push(diag(
                "a11y-label-for",
                "<label> sans attribut `for` ni champ de formulaire imbriqué — RGAA 11.1"
                    .to_string(),
                span,
                file,
            ));
        }
        tag @ ("a" | "button") => {
            let named = attr_nonempty(attrs, "aria-label")
                || has_attr(attrs, "aria-labelledby")
                || attr_nonempty(attrs, "title");
            if !named && !has_accessible_text(content) {
                issues.push(diag(
                    "a11y-control-name",
                    format!(
                        "<{tag}> sans intitulé accessible (texte visible ou aria-label) — RGAA 6.1"
                    ),
                    span,
                    file,
                ));
            }
        }
        _ => {}
    }
}

fn is_form_control(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "input" | "select" | "textarea"
    )
}

/// True if any descendant satisfies `pred(tag_name, attributes)`.
fn any_tag<F: Fn(&str, &[Attribute]) -> bool + Copy>(elements: &[Element], pred: F) -> bool {
    elements.iter().any(|el| match el {
        Element::Tag {
            name,
            attributes,
            content,
            ..
        } => pred(name, attributes) || any_tag(content, pred),
        Element::Component { content, .. }
        | Element::For { content, .. }
        | Element::SlotContent { content, .. }
        | Element::ErrorBlock { content, .. }
        | Element::Fragment { content, .. }
        | Element::Defer { content, .. } => any_tag(content, pred),
        Element::If {
            then_branch,
            else_branch,
            ..
        } => any_tag(then_branch, pred) || else_branch.as_ref().is_some_and(|eb| any_tag(eb, pred)),
        _ => false,
    })
}

/// True if the subtree provides an accessible name: visible text/interpolation,
/// or an image with a non-empty textual alternative.
fn has_accessible_text(elements: &[Element]) -> bool {
    elements.iter().any(|el| match el {
        Element::Text(t, _) => !t.trim().is_empty(),
        Element::Interpolation(_, _) => true,
        Element::Tag {
            name,
            attributes,
            content,
            ..
        } => {
            let t = name.to_ascii_lowercase();
            ((t == "img" || t == "svg")
                && (attr_nonempty(attributes, "alt") || attr_nonempty(attributes, "aria-label")))
                || has_accessible_text(content)
        }
        Element::Component { content, .. }
        | Element::For { content, .. }
        | Element::SlotContent { content, .. }
        | Element::ErrorBlock { content, .. }
        | Element::Fragment { content, .. }
        | Element::Defer { content, .. } => has_accessible_text(content),
        Element::If {
            then_branch,
            else_branch,
            ..
        } => {
            has_accessible_text(then_branch)
                || else_branch
                    .as_ref()
                    .is_some_and(|eb| has_accessible_text(eb))
        }
        _ => false,
    })
}

fn find_attr<'a>(attrs: &'a [Attribute], name: &str) -> Option<&'a Attribute> {
    attrs.iter().find(|a| a.name.eq_ignore_ascii_case(name))
}

fn has_attr(attrs: &[Attribute], name: &str) -> bool {
    find_attr(attrs, name).is_some()
}

/// True if the attribute is present with a non-empty value (a dynamic
/// `={expr}` value is assumed non-empty).
fn attr_nonempty(attrs: &[Attribute], name: &str) -> bool {
    find_attr(attrs, name).is_some_and(|a| match &a.value {
        AttributeValue::String(s) => !s.trim().is_empty(),
        AttributeValue::Expression(_) | AttributeValue::Spread(_) => true,
        AttributeValue::Boolean(_) => false,
    })
}

fn role_in(attrs: &[Attribute], roles: &[&str]) -> bool {
    find_attr(attrs, "role").is_some_and(|a| match &a.value {
        AttributeValue::String(s) => roles.iter().any(|r| s.eq_ignore_ascii_case(r)),
        _ => false,
    })
}

fn diag(code: &'static str, message: String, span: Span, file: Option<&Path>) -> Diagnostic {
    Diagnostic {
        severity: Severity::Warning,
        code,
        message,
        file: file.map(|p| p.display().to_string()),
        line: Some(span.line),
        col: Some(span.col),
    }
}
