//! Parser for .webc files using Pest PEG grammar

use crate::ast::{
    App, Attribute, AttributeValue, Component, ComputedVar, Element, Layout, Page, Prop, Span,
    StateVar, StyleItem, StyleProperty, StyleRule, WebCoreDocument,
};
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
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            span: None,
            source_line: None,
        }
    }

    pub fn with_span(message: impl Into<String>, span: Span) -> Self {
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
                write!(f, "\n  |\n  = hint: {}", hint)?;
            }
            Ok(())
        } else {
            write!(f, "{}", self.message)
        }
    }
}

pub fn parse_webc(source: &str) -> Result<WebCoreDocument, ParseError> {
    let pairs = WebCoreParser::parse(Rule::document, source).map_err(|e| {
        let (line, col) = match e.line_col {
            pest::error::LineColLocation::Pos((l, c)) => (l as u32, c as u32),
            pest::error::LineColLocation::Span((l, c), _) => (l as u32, c as u32),
        };
        let source_line = source
            .lines()
            .nth((line as usize).saturating_sub(1))
            .map(str::to_string);
        ParseError {
            message: format!("{}", e),
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
        for field_wrapper in body.into_inner() {
            // field_wrapper is Rule::app_field; unwrap to get the actual field type
            for field in field_wrapper.into_inner() {
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
        computed: Vec::new(),
        mount_body: None,
        destroy_body: None,
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
                Rule::computed_block => {
                    component.computed = parse_computed_block(section)?;
                }
                Rule::on_mount_block => {
                    component.mount_body = Some(parse_on_mount_block(section)?);
                }
                Rule::on_destroy_block => {
                    component.destroy_body = Some(parse_on_mount_block(section)?);
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

fn parse_computed_block(pair: pest::iterators::Pair<Rule>) -> Result<Vec<ComputedVar>, ParseError> {
    let mut vars = Vec::new();

    for def in pair.into_inner() {
        if def.as_rule() == Rule::computed_def {
            let span = Span::from_pest(def.as_span());
            let mut inner = def.into_inner();

            let name = inner
                .next()
                .map(|p| p.as_str().to_string())
                .unwrap_or_default();

            let expr = inner
                .next()
                .map(|p| p.as_str().trim().to_string())
                .unwrap_or_default();

            vars.push(ComputedVar { name, expr, span });
        }
    }

    Ok(vars)
}

fn parse_on_mount_block(pair: pest::iterators::Pair<Rule>) -> Result<String, ParseError> {
    let raw = pair.into_inner().next().map(|p| p.as_str()).unwrap_or("{}");
    // Strip outer { } and trim
    let trimmed = raw.trim();
    let body = if trimmed.starts_with('{') && trimmed.ends_with('}') {
        trimmed[1..trimmed.len() - 1].trim()
    } else {
        trimmed
    };
    Ok(body.to_string())
}

fn parse_style_rule_pair(rule_pair: pest::iterators::Pair<Rule>) -> Result<StyleRule, ParseError> {
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

    Ok(StyleRule {
        selector,
        properties,
        span,
    })
}

fn parse_style_block(pair: pest::iterators::Pair<Rule>) -> Result<Vec<StyleItem>, ParseError> {
    let mut items = Vec::new();

    for item_pair in pair.into_inner() {
        match item_pair.as_rule() {
            Rule::style_rule => {
                items.push(StyleItem::Rule(parse_style_rule_pair(item_pair)?));
            }
            Rule::media_block => {
                let span = Span::from_pest(item_pair.as_span());
                let mut inner = item_pair.into_inner();

                let query = inner
                    .next()
                    .map(|p| p.as_str().trim().to_string())
                    .unwrap_or_default();

                let mut rules = Vec::new();
                for rule_pair in inner {
                    if rule_pair.as_rule() == Rule::style_rule {
                        rules.push(parse_style_rule_pair(rule_pair)?);
                    }
                }

                items.push(StyleItem::Media { query, rules, span });
            }
            _ => {}
        }
    }

    Ok(items)
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

            // Optional index variable: @for item, i in items
            let mut index: Option<String> = None;
            let mut key: Option<String> = None;
            let mut iterable = String::new();

            for part in parts.by_ref() {
                match part.as_rule() {
                    // Second identifier after a comma is the index var
                    Rule::identifier => {
                        index = Some(part.as_str().to_string());
                    }
                    Rule::for_key_decl => {
                        key = part
                            .into_inner()
                            .find(|p| p.as_rule() == Rule::for_key_expr)
                            .map(|p| p.as_str().to_string());
                    }
                    Rule::expression => {
                        iterable = part.as_str().trim().to_string();
                        break;
                    }
                    _ => {}
                }
            }

            let mut content = Vec::new();
            for elem in parts {
                if elem.as_rule() == Rule::element {
                    content.push(parse_element(elem)?);
                }
            }

            Ok(Element::For {
                item,
                index,
                iterable,
                key,
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
        Rule::switch_statement => {
            let span = Span::from_pest(inner.as_span());
            let mut parts = inner.into_inner();

            // First child is switch_expr (dedicated atomic rule, stops before {)
            let switch_expr = parts
                .next()
                .map(|p| p.as_str().trim().to_string())
                .unwrap_or_default();

            // Collect @case / @default clauses
            let mut case_entries: Vec<(String, Vec<Element>)> = Vec::new();
            let mut default_content: Option<Vec<Element>> = None;

            for clause in parts {
                match clause.as_rule() {
                    Rule::case_clause => {
                        let mut clause_parts = clause.into_inner();
                        let val = clause_parts
                            .next()
                            .map(|p| p.as_str().trim().to_string())
                            .unwrap_or_default();
                        let mut content = Vec::new();
                        for elem in clause_parts {
                            if elem.as_rule() == Rule::element {
                                content.push(parse_element(elem)?);
                            }
                        }
                        case_entries.push((val, content));
                    }
                    Rule::default_clause => {
                        let mut content = Vec::new();
                        for elem in clause.into_inner() {
                            if elem.as_rule() == Rule::element {
                                content.push(parse_element(elem)?);
                            }
                        }
                        default_content = Some(content);
                    }
                    _ => {}
                }
            }

            if case_entries.is_empty() {
                return Err(ParseError::with_span(
                    "@switch must have at least one @case",
                    span,
                ));
            }

            // Build @if / @else chain from last case to first
            let mut else_branch: Option<Vec<Element>> = default_content;
            for (val, content) in case_entries.into_iter().rev() {
                let condition = format!("{} === {}", switch_expr, val);
                else_branch = Some(vec![Element::If {
                    condition,
                    then_branch: content,
                    else_branch,
                    span,
                }]);
            }

            else_branch
                .and_then(|mut v| v.pop())
                .ok_or_else(|| ParseError::with_span("@switch: empty case chain", span))
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
    let mut inner = pair.into_inner().peekable();

    let name = if inner
        .peek()
        .map(|p| p.as_rule() == Rule::identifier)
        .unwrap_or(false)
    {
        inner
            .next()
            .map(|p| p.as_str().to_string())
            .unwrap_or_else(|| "content".to_string())
    } else {
        "content".to_string()
    };

    let mut content = Vec::new();
    for part in inner {
        if part.as_rule() == Rule::element {
            content.push(parse_element(part)?);
        }
    }

    if content.is_empty() {
        Ok(Element::Slot(name, span))
    } else {
        Ok(Element::SlotContent {
            name,
            content,
            span,
        })
    }
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

/// Parse a single attribute from its Pest pair.
///
/// The grammar rule is:
/// ```pest
/// attribute  = { attr_name ~ "=" ~ attr_value }
/// attr_value = { expression_attr | string_literal | "true" | "false" }
/// expression_attr = { "{" ~ expression_content ~ "}" }
/// ```
///
/// So each attribute pair has exactly two inner pairs:
/// 1. `attr_name`  — the raw attribute name string (e.g. `class`, `on:click`, `validate:required`)
/// 2. `attr_value` — one of: `expression_attr` (wraps `expression_content`),
///                            `string_literal` (wraps `string_inner`),
///                            or a bare boolean token `true`/`false`
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

/// Split a string potentially containing multiple {var} interpolations into Elements.
/// Handles escape sequences: \{ → literal '{', \" → literal '"'.
fn split_interpolated_text(text: &str) -> Vec<Element> {
    let mut elements: Vec<Element> = Vec::new();
    let default_span = Span::default();
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0usize;
    let mut current_text = String::new();

    while i < len {
        if chars[i] == '\\' && i + 1 < len {
            match chars[i + 1] {
                '{' => { current_text.push('{'); i += 2; }
                '"' => { current_text.push('"'); i += 2; }
                '\\' => { current_text.push('\\'); i += 2; }
                other => { current_text.push('\\'); current_text.push(other); i += 2; }
            }
        } else if chars[i] == '{' {
            // Start of interpolation — flush pending text
            if !current_text.is_empty() {
                elements.push(Element::Text(current_text.clone(), default_span));
                current_text.clear();
            }
            // Find matching '}'
            if let Some(close) = chars[i + 1..].iter().position(|&c| c == '}') {
                let var_name: String = chars[i + 1..i + 1 + close].iter().collect();
                elements.push(Element::Interpolation(var_name.trim().to_string(), default_span));
                i += close + 2;
            } else {
                // No closing brace — treat rest as literal text
                current_text.extend(chars[i..].iter());
                break;
            }
        } else {
            current_text.push(chars[i]);
            i += 1;
        }
    }

    if !current_text.is_empty() {
        elements.push(Element::Text(current_text, default_span));
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
