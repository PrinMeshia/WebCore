//! Parser for .webc files using Pest PEG grammar

mod declarations;
mod directives;
mod elements;

use crate::core::ast::{ImportDecl, Span, WebCoreDocument};
use pest::Parser;
use pest_derive::Parser;
use std::collections::HashMap;

#[derive(Parser)]
#[grammar = "grammar.pest"]
pub struct WebCoreParser;

/// Parse error with source location
#[derive(Debug)]
pub struct ParseError {
    pub message: String,
    pub span: Option<Span>,
    /// The exact source line where the error occurred (for caret display).
    pub source_line: Option<String>,
}

impl ParseError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            span: None,
            source_line: None,
        }
    }

    pub(crate) fn with_span(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span: Some(span),
            source_line: None,
        }
    }
}

fn parse_hint(msg: &str) -> Option<&'static str> {
    if msg.contains("interp_expr") {
        Some("{} est vide — écris {maVar} ou utilise un attribut string: attr=\"valeur\"")
    } else if msg.contains("string_literal") {
        Some("valeur texte attendue entre guillemets, ex: \"ma valeur\"")
    } else if msg.contains("identifier") {
        Some("nom attendu (lettres, chiffres, _) sans guillemets")
    } else {
        None
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let (Some(span), Some(src)) = (&self.span, &self.source_line) {
            let col0 = (span.col as usize).saturating_sub(1);
            write!(
                f,
                "{}:{}\n  |\n{:>3} | {}\n  | {}^",
                span.line,
                span.col,
                span.line,
                src,
                " ".repeat(col0)
            )?;
            if let Some(hint) = parse_hint(&self.message) {
                write!(f, "\n  |\n  = hint: {hint}")?;
            }
            Ok(())
        } else {
            write!(f, "{}", self.message)
        }
    }
}

pub(crate) fn parse_webc(source: &str) -> Result<WebCoreDocument, ParseError> {
    let pairs = WebCoreParser::parse(Rule::document, source).map_err(|e| {
        let (line, col) = match e.line_col {
            pest::error::LineColLocation::Pos((l, c))
            | pest::error::LineColLocation::Span((l, c), _) => (l as u32, c as u32),
        };
        let source_line = source
            .lines()
            .nth((line as usize).saturating_sub(1))
            .map(str::to_string);
        ParseError {
            message: format!("{e}"),
            span: Some(Span::new(0, 0, line, col)),
            source_line,
        }
    })?;

    let mut document = WebCoreDocument {
        app: None,
        store: Vec::new(),
        locales: HashMap::new(),
        default_locale: String::new(),
        wasm_module: None,
        layouts: HashMap::new(),
        pages: HashMap::new(),
        components: HashMap::new(),
        imports: Vec::new(),
        data_imports: HashMap::new(),
    };

    for pair in pairs {
        if pair.as_rule() == Rule::document {
            for inner in pair.into_inner() {
                match inner.as_rule() {
                    Rule::app_decl => {
                        document.app = Some(declarations::parse_app(inner)?);
                    }
                    Rule::layout_decl => {
                        let layout = declarations::parse_layout(inner)?;
                        document.layouts.insert(layout.name.clone(), layout);
                    }
                    Rule::page_decl => {
                        let page = declarations::parse_page(inner)?;
                        document.pages.insert(page.name.clone(), page);
                    }
                    Rule::store_decl => {
                        let mut vars = declarations::parse_state_block(inner)?;
                        document.store.append(&mut vars);
                    }
                    Rule::component_decl => {
                        let component = declarations::parse_component(inner)?;
                        document
                            .components
                            .insert(component.name.clone(), component);
                    }
                    Rule::import_decl => {
                        let mut parts = inner.into_inner();
                        let name = parts.next().map(|p| p.as_str().to_string()).unwrap_or_default();
                        let path_raw = parts.next().map(|p| p.as_str().to_string()).unwrap_or_default();
                        let path = elements::extract_string_literal(&path_raw);
                        document.imports.push(ImportDecl { name, path });
                    }
                    _ => {}
                }
            }
        }
    }

    // Validate nesting depth across all elements to prevent stack-overflow bombs.
    const MAX_DEPTH: usize = 128;
    let mut depth_err: Option<ParseError> = None;
    for page in document.pages.values() {
        for el in &page.content {
            if let Err(e) = check_nesting_depth(el, 0, MAX_DEPTH) {
                depth_err = Some(e);
                break;
            }
        }
    }
    for comp in document.components.values() {
        for el in &comp.view {
            if let Err(e) = check_nesting_depth(el, 0, MAX_DEPTH) {
                depth_err = Some(e);
                break;
            }
        }
    }
    for layout in document.layouts.values() {
        for el in &layout.content {
            if let Err(e) = check_nesting_depth(el, 0, MAX_DEPTH) {
                depth_err = Some(e);
                break;
            }
        }
    }
    if let Some(e) = depth_err {
        return Err(e);
    }

    Ok(document)
}

fn check_nesting_depth(
    el: &crate::core::ast::Element,
    depth: usize,
    max: usize,
) -> Result<(), ParseError> {
    if depth > max {
        return Err(ParseError::new(format!(
            "Element nesting exceeds maximum depth of {max} — reduce component complexity"
        )));
    }
    for child in el.children() {
        check_nesting_depth(child, depth + 1, max)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ast::{AttributeValue, Element};

    #[test]
    fn test_parse_simple_component() {
        let src = r#"
component Counter {
    state {
        count: Int = 0
    }
    view {
        button on:click={count += 1} "Click me"
    }
}
"#;
        let doc = parse_webc(src).expect("parse ok");
        assert!(doc.components.contains_key("Counter"));
        let counter = doc.components.get("Counter").unwrap();
        assert_eq!(counter.state.len(), 1);
        assert_eq!(counter.state[0].name, "count");
    }

    #[test]
    fn test_parse_interpolation() {
        let src = r#"
component Hello {
    view {
        p "Hello {name}!"
    }
}
"#;
        let doc = parse_webc(src).expect("parse ok");
        let comp = doc.components.get("Hello").unwrap();
        assert!(!comp.view.is_empty());
    }

    #[test]
    fn test_parse_layout_with_slot() {
        let src = r#"
layout MainLayout {
    header { nav "Navigation" }
    main { slot content }
    footer "Footer"
}
"#;
        let doc = parse_webc(src).expect("parse ok");
        assert!(doc.layouts.contains_key("MainLayout"));
    }

    #[test]
    fn test_parse_expression_interpolation() {
        // {count + 1} and {user.name} should parse — not just simple identifiers
        let src = r#"
component Expr {
    state { count: Int = 0 }
    view {
        p "Score: {count + 1} pts"
        p "Remaining: {10 - count}"
    }
}
"#;
        let doc = parse_webc(src).expect("expression interpolation should parse");
        let comp = doc.components.get("Expr").unwrap();
        assert!(!comp.view.is_empty());
    }

    #[test]
    fn test_parse_mixed_tag_content() {
        // Mixed strings and child elements inside a tag block
        let src = r#"
layout Mixed {
    p {
        "Hello "
        strong { "World" }
        "!"
    }
}
"#;
        let doc = parse_webc(src).expect("mixed content should parse");
        let layout = doc.layouts.get("Mixed").unwrap();
        // p tag should contain mixed content: Text, Tag(strong), Text
        if let Element::Tag { name, content, .. } = &layout.content[0] {
            assert_eq!(name, "p");
            assert!(
                content.len() >= 2,
                "mixed content: got {} children",
                content.len()
            );
        } else {
            panic!("expected Tag element");
        }
    }

    #[test]
    fn test_parse_onclick_expression() {
        let src = r#"
layout TestLayout {
    button on:click={count += 1} "Click me"
}
"#;
        let doc = parse_webc(src).expect("parse ok");
        let layout = doc.layouts.get("TestLayout").expect("layout exists");
        assert!(!layout.content.is_empty());

        // Find the button element
        if let Element::Tag {
            name, attributes, ..
        } = &layout.content[0]
        {
            assert_eq!(name, "button");
            assert!(!attributes.is_empty());
            let attr = &attributes[0];
            assert_eq!(attr.name, "on:click");
            match &attr.value {
                AttributeValue::Expression(expr) => {
                    assert!(expr.contains("count"));
                    assert!(expr.contains("+="));
                }
                other => panic!("Expected Expression, got {:?}", other),
            }
        } else {
            panic!("Expected Tag element");
        }
    }
}
