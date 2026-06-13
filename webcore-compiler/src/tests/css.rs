//! Tests that verify CSS output.

#[cfg(test)]
use super::*;

#[test]
fn golden_scoped_css_emits_data_v_selector() {
    let mut doc = WebCoreDocument {
        app: None,
        store: vec![],
        locales: std::collections::HashMap::new(),
        default_locale: String::new(),
        wasm_module: None,
        layouts: std::collections::HashMap::new(),
        pages: std::collections::HashMap::new(),
        components: std::collections::HashMap::new(),
    };
    doc.components.insert(
        "Counter".into(),
        Component {
            name: "Counter".into(),
            props: vec![],
            state: vec![],
            computed: vec![],
            mount_body: None,
            destroy_body: None,
            http: None,
            view: vec![],
            style: vec![crate::core::ast::StyleItem::Rule(crate::core::ast::StyleRule {
                selector: "button".into(),
                properties: vec![crate::core::ast::StyleProperty {
                    name: "color".into(),
                    value: "red".into(),
                    span: Span::default(),
                }],
                nested: vec![],
                span: Span::default(),
            })],
            span: Span::default(),
        },
    );
    let css = generate_combined_css(None, &doc);
    assert!(css.contains(&format!("[{}=", attr_names::SCOPE)), "no scoped selector in:\n{}", css);
    assert!(css.contains("color: red") || css.contains("color:red"));
}

#[test]
fn golden_media_block_scoped_in_css() {
    let src = r#"
layout MainLayout { main { slot content } }
component Card {
    view { div "Contenu" }
    style {
        .card { padding: 1rem; }
        @media (max-width: 768px) {
            .card { padding: 0.5rem; }
        }
    }
}
page "home" { Card {} }
"#;
    let doc = parse_webc(src).expect("parse");
    let css = crate::codegen::css::generate_combined_css(None, &doc);
    assert!(css.contains("@media (max-width: 768px)"), "@media block missing in output:\n{}", css);
    assert!(css.contains(&format!("[{}=", attr_names::SCOPE)), "scoped selector missing inside @media:\n{}", css);
}

#[test]
fn golden_css_nesting_flattened() {
    let src = r#"
        component Button {
            style {
                button {
                    color: blue;
                    &:hover {
                        color: darkblue;
                    }
                }
            }
        }
    "#;
    let doc = parse_webc(src).expect("parse");
    let css = generate_combined_css(None, &doc);
    assert!(css.contains("color: blue"), "base property missing from CSS:\n{css}");
    assert!(!css.contains('&'), "& should be replaced in flattened output:\n{css}");
    assert!(css.contains(":hover") && css.contains("color: darkblue"), "flattened :hover rule missing from CSS:\n{css}");
}

#[test]
fn golden_css_nesting_parse_and_roundtrip() {
    let src = r#"
        component Card {
            style {
                .card {
                    background: white;
                    &:focus {
                        outline: 2px solid blue;
                    }
                    &::before {
                        content: "";
                    }
                }
            }
        }
    "#;
    let doc = parse_webc(src).expect("parse");
    let comp = doc.components.values().next().expect("component");
    let rule = match comp.style.first().expect("style item") {
        crate::core::ast::StyleItem::Rule(r) => r,
        _ => panic!("expected Rule"),
    };
    assert_eq!(rule.nested.len(), 2, "expected 2 nested rules, got: {:?}", rule.nested);
    assert!(rule.nested[0].selector.starts_with("&:focus"), "first nested selector wrong");
    assert!(rule.nested[1].selector.starts_with("&::before"), "second nested selector wrong");
    let css = generate_combined_css(None, &doc);
    assert!(!css.contains('&'), "& leaked into output CSS:\n{css}");
    assert!(css.contains(":focus"), ":focus missing from output:\n{css}");
    assert!(css.contains("::before"), "::before missing from output:\n{css}");
}
