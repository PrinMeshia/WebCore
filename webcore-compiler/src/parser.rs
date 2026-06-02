//! Parser for .webc files using Pest PEG grammar

use crate::ast::*;
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
}

impl ParseError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            span: None,
        }
    }

    pub fn with_span(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span: Some(span),
        }
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(span) = &self.span {
            write!(f, "{}:{}: {}", span.line, span.col, self.message)
        } else {
            write!(f, "{}", self.message)
        }
    }
}

pub fn parse_webc(source: &str) -> Result<WebCoreDocument, ParseError> {
    let pairs = WebCoreParser::parse(Rule::document, source)
        .map_err(|e| ParseError::new(format!("Parse error: {}", e)))?;

    let mut document = WebCoreDocument {
        app: None,
        store: Vec::new(),
        locales: HashMap::new(),
        default_locale: String::new(),
        wasm_module: None,
        layouts: HashMap::new(),
        pages: HashMap::new(),
        components: HashMap::new(),
    };

    for pair in pairs {
        if pair.as_rule() == Rule::document {
            for inner in pair.into_inner() {
                match inner.as_rule() {
                    Rule::app_decl => {
                        document.app = Some(parse_app(inner)?);
                    }
                    Rule::layout_decl => {
                        let layout = parse_layout(inner)?;
                        document.layouts.insert(layout.name.clone(), layout);
                    }
                    Rule::page_decl => {
                        let page = parse_page(inner)?;
                        document.pages.insert(page.name.clone(), page);
                    }
                    Rule::store_decl => {
                        let mut vars = parse_state_block(inner)?;
                        document.store.append(&mut vars);
                    }
                    Rule::component_decl => {
                        let component = parse_component(inner)?;
                        document
                            .components
                            .insert(component.name.clone(), component);
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(document)
}

fn parse_app(pair: pest::iterators::Pair<Rule>) -> Result<App, ParseError> {
    let span = Span::from_pest(pair.as_span());
    let mut inner = pair.into_inner();

    let name = inner
        .next()
        .ok_or_else(|| ParseError::with_span("Expected app name", span))?
        .as_str()
        .to_string();

    let mut app = App {
        name,
        theme: None,
        layout: None,
        routes: HashMap::new(),
        span,
    };

    // Parse app body
    if let Some(body) = inner.next() {
        for field in body.into_inner() {
            match field.as_rule() {
                Rule::app_theme => {
                    if let Some(val) = field.into_inner().next() {
                        app.theme = Some(extract_string_literal(val.as_str()));
                    }
                }
                Rule::app_layout => {
                    if let Some(val) = field.into_inner().next() {
                        app.layout = Some(val.as_str().to_string());
                    }
                }
                Rule::app_routes => {
                    for entry in field.into_inner() {
                        if entry.as_rule() == Rule::route_entry {
                            let mut parts = entry.into_inner();
                            if let (Some(path), Some(comp)) = (parts.next(), parts.next()) {
                                app.routes.insert(
                                    extract_string_literal(path.as_str()),
                                    comp.as_str().to_string(),
                                );
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    Ok(app)
}

fn parse_layout(pair: pest::iterators::Pair<Rule>) -> Result<Layout, ParseError> {
    let span = Span::from_pest(pair.as_span());
    let mut inner = pair.into_inner();

    let name = inner
        .next()
        .ok_or_else(|| ParseError::with_span("Expected layout name", span))?
        .as_str()
        .to_string();

    let mut content = Vec::new();
    for elem in inner {
        if elem.as_rule() == Rule::element {
            content.push(parse_element(elem)?);
        }
    }

    Ok(Layout {
        name,
        content,
        span,
    })
}

fn parse_page(pair: pest::iterators::Pair<Rule>) -> Result<Page, ParseError> {
    let span = Span::from_pest(pair.as_span());
    let mut inner = pair.into_inner();

    let name_token = inner
        .next()
        .ok_or_else(|| ParseError::with_span("Expected page name", span))?;
    let name = extract_string_literal(name_token.as_str());

    let mut content = Vec::new();
    for elem in inner {
        if elem.as_rule() == Rule::element {
            content.push(parse_element(elem)?);
        }
    }

    Ok(Page {
        name,
        content,
        span,
    })
}

fn parse_component(pair: pest::iterators::Pair<Rule>) -> Result<Component, ParseError> {
    let span = Span::from_pest(pair.as_span());
    let mut inner = pair.into_inner();

    let name = inner
        .next()
        .ok_or_else(|| ParseError::with_span("Expected component name", span))?
        .as_str()
        .to_string();

    let mut component = Component {
        name,
        props: Vec::new(),
        state: Vec::new(),
        view: Vec::new(),
        style: Vec::new(),
        span,
    };

    // Parse component body
    if let Some(body) = inner.next() {
        for section in body.into_inner() {
            match section.as_rule() {
                Rule::props_block => {
                    component.props = parse_props_block(section)?;
                }
                Rule::state_block => {
                    component.state = parse_state_block(section)?;
                }
                Rule::view_block => {
                    for elem in section.into_inner() {
                        if elem.as_rule() == Rule::element {
                            component.view.push(parse_element(elem)?);
                        }
                    }
                }
                Rule::style_block => {
                    component.style = parse_style_block(section)?;
                }
                Rule::element => {
                    // Fallback: elements directly in component (old syntax)
                    component.view.push(parse_element(section)?);
                }
                _ => {}
            }
        }
    }

    Ok(component)
}

fn parse_props_block(pair: pest::iterators::Pair<Rule>) -> Result<Vec<Prop>, ParseError> {
    let mut props = Vec::new();

    for def in pair.into_inner() {
        if def.as_rule() == Rule::prop_def {
            let span = Span::from_pest(def.as_span());
            let mut inner = def.into_inner();

            let name = inner
                .next()
                .map(|p| p.as_str().to_string())
                .unwrap_or_default();

            let type_ = inner.next().map(|p| p.as_str().to_string());

            props.push(Prop { name, type_, span });
        }
    }

    Ok(props)
}

fn parse_state_block(pair: pest::iterators::Pair<Rule>) -> Result<Vec<StateVar>, ParseError> {
    let mut state_vars = Vec::new();

    for def in pair.into_inner() {
        if def.as_rule() == Rule::state_def {
            let span = Span::from_pest(def.as_span());
            let mut inner = def.into_inner();

            let name = inner
                .next()
                .map(|p| p.as_str().to_string())
                .unwrap_or_default();

            let type_ = inner
                .next()
                .map(|p| p.as_str().to_string())
                .unwrap_or_else(|| "Any".to_string());

            let default_value = inner.next().map(|p| {
                let val = p.as_str().trim();
                // Remove quotes if string literal
                if val.starts_with('"') && val.ends_with('"') {
                    extract_string_literal(val)
                } else {
                    val.to_string()
                }
            });

            state_vars.push(StateVar {
                name,
                type_,
                default_value,
                span,
            });
        }
    }

    Ok(state_vars)
}

fn parse_style_block(pair: pest::iterators::Pair<Rule>) -> Result<Vec<StyleRule>, ParseError> {
    let mut rules = Vec::new();

    for rule_pair in pair.into_inner() {
        if rule_pair.as_rule() == Rule::style_rule {
            let span = Span::from_pest(rule_pair.as_span());
            let mut inner = rule_pair.into_inner();

            let selector = inner
                .next()
                .map(|p| p.as_str().trim().to_string())
                .unwrap_or_default();

            let mut properties = Vec::new();
            for prop in inner {
                if prop.as_rule() == Rule::style_property {
                    let prop_span = Span::from_pest(prop.as_span());
                    let mut prop_inner = prop.into_inner();

                    let name = prop_inner
                        .next()
                        .map(|p| p.as_str().to_string())
                        .unwrap_or_default();

                    let value = prop_inner
                        .next()
                        .map(|p| p.as_str().trim().to_string())
                        .unwrap_or_default();

                    properties.push(StyleProperty {
                        name,
                        value,
                        span: prop_span,
                    });
                }
            }

            rules.push(StyleRule {
                selector,
                properties,
                span,
            });
        }
    }

    Ok(rules)
}

fn parse_element(pair: pest::iterators::Pair<Rule>) -> Result<Element, ParseError> {
    let inner = pair
        .into_inner()
        .next()
        .ok_or_else(|| ParseError::new("Empty element"))?;

    match inner.as_rule() {
        Rule::control_flow => parse_control_flow(inner),
        Rule::slot_element => parse_slot(inner),
        Rule::tag_element => parse_tag(inner),
        Rule::text_element => parse_text_element(inner),
        _ => Err(ParseError::new(format!(
            "Unexpected element rule: {:?}",
            inner.as_rule()
        ))),
    }
}

fn parse_control_flow(pair: pest::iterators::Pair<Rule>) -> Result<Element, ParseError> {
    let inner = pair
        .into_inner()
        .next()
        .ok_or_else(|| ParseError::new("Empty control flow"))?;

    match inner.as_rule() {
        Rule::for_loop => {
            let span = Span::from_pest(inner.as_span());
            let mut parts = inner.into_inner();

            let item = parts
                .next()
                .map(|p| p.as_str().to_string())
                .unwrap_or_default();

            let iterable = parts
                .next()
                .map(|p| p.as_str().trim().to_string())
                .unwrap_or_default();

            let mut content = Vec::new();
            for elem in parts {
                if elem.as_rule() == Rule::element {
                    content.push(parse_element(elem)?);
                }
            }

            Ok(Element::For {
                item,
                iterable,
                content,
                span,
            })
        }
        Rule::if_statement => {
            let span = Span::from_pest(inner.as_span());
            let mut parts = inner.into_inner();

            let condition = parts
                .next()
                .map(|p| p.as_str().trim().to_string())
                .unwrap_or_default();

            let mut then_branch = Vec::new();
            let mut else_branch = None;

            for part in parts {
                match part.as_rule() {
                    Rule::element => {
                        then_branch.push(parse_element(part)?);
                    }
                    Rule::else_branch => {
                        let mut else_content = Vec::new();
                        for elem in part.into_inner() {
                            if elem.as_rule() == Rule::element {
                                else_content.push(parse_element(elem)?);
                            }
                        }
                        else_branch = Some(else_content);
                    }
                    _ => {}
                }
            }

            Ok(Element::If {
                condition,
                then_branch,
                else_branch,
                span,
            })
        }
        Rule::error_block => {
            let span = Span::from_pest(inner.as_span());
            let mut parts = inner.into_inner();

            let field_token = parts
                .next()
                .ok_or_else(|| ParseError::with_span("Expected field name in @error", span))?;
            let field = extract_string_literal(field_token.as_str());

            let mut content = Vec::new();
            for elem in parts {
                if elem.as_rule() == Rule::element {
                    content.push(parse_element(elem)?);
                }
            }

            Ok(Element::ErrorBlock {
                field,
                content,
                span,
            })
        }
        _ => Err(ParseError::new(format!(
            "Unknown control flow: {:?}",
            inner.as_rule()
        ))),
    }
}

fn parse_slot(pair: pest::iterators::Pair<Rule>) -> Result<Element, ParseError> {
    let span = Span::from_pest(pair.as_span());
    let name = pair
        .into_inner()
        .next()
        .map(|p| p.as_str().to_string())
        .unwrap_or_else(|| "content".to_string());

    Ok(Element::Slot(name, span))
}

fn parse_tag(pair: pest::iterators::Pair<Rule>) -> Result<Element, ParseError> {
    let span = Span::from_pest(pair.as_span());
    let mut inner = pair.into_inner();

    let tag_name = inner
        .next()
        .ok_or_else(|| ParseError::with_span("Expected tag name", span))?
        .as_str()
        .to_string();

    let mut attributes = Vec::new();
    let mut content = Vec::new();

    for part in inner {
        match part.as_rule() {
            Rule::attribute => {
                attributes.push(parse_attribute(part)?);
            }
            Rule::tag_content => {
                content = parse_tag_content(part)?;
            }
            _ => {}
        }
    }

    // Check if it's a component (capitalized) or regular tag
    if tag_name.chars().next().is_some_and(|c| c.is_uppercase()) {
        Ok(Element::Component {
            name: tag_name,
            attributes,
            content,
            span,
        })
    } else {
        Ok(Element::Tag {
            name: tag_name,
            attributes,
            content,
            span,
        })
    }
}

fn parse_attribute(pair: pest::iterators::Pair<Rule>) -> Result<Attribute, ParseError> {
    let span = Span::from_pest(pair.as_span());
    let mut inner = pair.into_inner();

    let name = inner
        .next()
        .map(|p| p.as_str().to_string())
        .unwrap_or_default();

    let value = if let Some(val_pair) = inner.next() {
        // val_pair is attr_value, we need to look inside it
        if val_pair.as_rule() == Rule::attr_value {
            // Get the inner content of attr_value
            if let Some(inner_val) = val_pair.into_inner().next() {
                match inner_val.as_rule() {
                    Rule::expression_attr => {
                        let expr = inner_val
                            .into_inner()
                            .next()
                            .map(|p| p.as_str().trim().to_string())
                            .unwrap_or_default();
                        AttributeValue::Expression(expr)
                    }
                    Rule::string_literal => {
                        AttributeValue::String(extract_string_literal(inner_val.as_str()))
                    }
                    _ => {
                        let text = inner_val.as_str();
                        match text {
                            "true" => AttributeValue::Boolean(true),
                            "false" => AttributeValue::Boolean(false),
                            _ => AttributeValue::String(text.to_string()),
                        }
                    }
                }
            } else {
                AttributeValue::Boolean(true)
            }
        } else {
            // Direct match (shouldn't happen with current grammar)
            match val_pair.as_rule() {
                Rule::expression_attr => {
                    let expr = val_pair
                        .into_inner()
                        .next()
                        .map(|p| p.as_str().trim().to_string())
                        .unwrap_or_default();
                    AttributeValue::Expression(expr)
                }
                Rule::string_literal => {
                    AttributeValue::String(extract_string_literal(val_pair.as_str()))
                }
                _ => {
                    let text = val_pair.as_str();
                    match text {
                        "true" => AttributeValue::Boolean(true),
                        "false" => AttributeValue::Boolean(false),
                        _ => AttributeValue::String(text.to_string()),
                    }
                }
            }
        }
    } else {
        AttributeValue::Boolean(true)
    };

    Ok(Attribute { name, value, span })
}

fn parse_tag_content(pair: pest::iterators::Pair<Rule>) -> Result<Vec<Element>, ParseError> {
    let mut content = Vec::new();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::element => {
                content.push(parse_element(inner)?);
            }
            Rule::interpolated_content | Rule::string_with_interpolation => {
                // Parse string with potential interpolations
                let text = inner.as_str();
                let clean_text = if text.starts_with('"') && text.ends_with('"') {
                    &text[1..text.len() - 1]
                } else {
                    text
                };
                content.extend(split_interpolated_text(clean_text));
            }
            _ => {}
        }
    }

    Ok(content)
}

fn parse_text_element(pair: pest::iterators::Pair<Rule>) -> Result<Element, ParseError> {
    let span = Span::from_pest(pair.as_span());
    let text = pair.as_str();

    // Remove surrounding quotes
    let clean_text = if text.starts_with('"') && text.ends_with('"') {
        &text[1..text.len() - 1]
    } else {
        text
    };

    // Check for interpolations
    if clean_text.contains('{') && clean_text.contains('}') {
        // If it's just a single interpolation, return Interpolation
        if clean_text.starts_with('{')
            && clean_text.ends_with('}')
            && clean_text.matches('{').count() == 1
        {
            return Ok(Element::Interpolation(
                clean_text[1..clean_text.len() - 1].to_string(),
                span,
            ));
        }
    }

    Ok(Element::Text(clean_text.to_string(), span))
}

/// Split a string potentially containing multiple {var} interpolations into Elements
fn split_interpolated_text(text: &str) -> Vec<Element> {
    let mut elements: Vec<Element> = Vec::new();
    let mut i = 0usize;
    let len = text.len();
    let default_span = Span::default();

    while i < len {
        if let Some(start) = text[i..].find('{') {
            let start_idx = i + start;

            // Push prefix text if any
            if start_idx > i {
                elements.push(Element::Text(text[i..start_idx].to_string(), default_span));
            }

            // Find matching '}'
            if let Some(end) = text[start_idx..].find('}') {
                let end_idx = start_idx + end;
                let var_name = text[start_idx + 1..end_idx].trim().to_string();
                elements.push(Element::Interpolation(var_name, default_span));
                i = end_idx + 1;
            } else {
                // No closing brace, treat rest as text
                elements.push(Element::Text(text[start_idx..].to_string(), default_span));
                break;
            }
        } else {
            // No more '{'
            if i < len {
                elements.push(Element::Text(text[i..].to_string(), default_span));
            }
            break;
        }
    }

    if elements.is_empty() && !text.is_empty() {
        elements.push(Element::Text(text.to_string(), default_span));
    }

    elements
}

/// Extract string content from quoted literal
fn extract_string_literal(s: &str) -> String {
    let trimmed = s.trim();
    if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
        trimmed[1..trimmed.len() - 1].to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
