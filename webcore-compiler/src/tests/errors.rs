//! Error-path and correctness tests.

#[cfg(test)]
use super::*;

#[test]
fn error_missing_layout_returns_err() {
    let src = r#"page "home" { h1 "Hello" }"#;
    let doc = parse_webc(src).expect("parse");
    let result = generate_html(&doc, "home", &opts());
    assert!(result.is_err(), "expected error for missing layout");
    let msg = match result {
        Err(e) => format!("{e}"),
        Ok(_) => panic!("expected Err"),
    };
    assert!(msg.to_lowercase().contains("layout"), "error should mention 'layout': {msg}");
}

#[test]
fn error_missing_page_returns_err() {
    let src = r#"layout MainLayout { main { slot content } }"#;
    let doc = parse_webc(src).expect("parse");
    let result = generate_html(&doc, "home", &opts());
    assert!(result.is_err(), "expected error for missing page 'home'");
}

#[test]
fn error_parse_empty_interpolation_fails() {
    let src = r#"page "home" { p "value: {}" }"#;
    let result = parse_webc(src);
    assert!(result.is_err(), "empty string interpolation {{}} should fail to parse");
}

#[test]
fn golden_error_message_has_caret() {
    let src = "page \"home\" { p \"hello {}\" }";
    let err = crate::parser::parse_webc(src).unwrap_err();
    let display = format!("{}", err);
    assert!(display.contains('^'), "caret missing in error: {display}");
}

#[test]
fn compile_errors_display_shows_count() {
    use crate::core::error::{CompileError, CompileErrors};
    let errors = CompileErrors(vec![
        CompileError::MissingPage { name: "home".into() },
        CompileError::MissingComponent { name: "Counter".into() },
    ]);
    let display = format!("{}", errors);
    assert!(display.contains("2 error(s) found."), "error count missing from display: {display}");
}

#[test]
fn compile_errors_from_single_error() {
    use crate::core::error::{CompileError, CompileErrors};
    let single = CompileError::MissingLayout {
        name: "Main".into(),
        available: vec![],
    };
    let multi: CompileErrors = single.into();
    assert_eq!(multi.0.len(), 1);
    assert!(
        format!("{}", multi).contains("1 error(s) found."),
        "expected '1 error(s) found.' in display"
    );
}

#[test]
fn check_circular_component_reference_detected() {
    let src = r#"
layout MainLayout { main { slot content } }
component A { view { B {} } }
component B { view { A {} } }
page "home" { A {} }
"#;
    let doc = parse_webc(src).expect("parse");
    let a = doc.components.get("A").expect("component A");
    assert!(a.view.iter().any(|e| matches!(e, ast::Element::Component { name, .. } if name == "B")));
    let b = doc.components.get("B").expect("component B");
    assert!(b.view.iter().any(|e| matches!(e, ast::Element::Component { name, .. } if name == "A")));
}

#[test]
fn bundle_analysis_detects_bindfor() {
    let src = r#"
        page "home" {
            @for item in items {
                p "{item}"
            }
        }
    "#;
    let doc = parse_webc(src).expect("parse");
    let js = generate_runtime_js(&[], &doc);
    assert!(js.contains("bindFor"), "expected bindFor marker in JS when @for loop is used");
}

#[test]
fn bundle_analysis_tree_shaken_when_unused() {
    let src = r#"
        page "home" {
            p "Hello"
        }
    "#;
    let doc = parse_webc(src).expect("parse");
    let js = generate_runtime_js(&[], &doc);
    assert!(!js.contains("bindFor"), "bindFor should be tree-shaken when @for is not used");
}

#[test]
fn unit_fnv1a_hash_stable() {
    let hash = crate::cli::assets::fnv1a_hash(b"hello");
    assert_eq!(hash.len(), 8, "hash must be 8 hex chars");
    assert_eq!(hash, crate::cli::assets::fnv1a_hash(b"hello"), "hash must be deterministic");
    assert_ne!(hash, crate::cli::assets::fnv1a_hash(b"world"), "hash must depend on content");
}
