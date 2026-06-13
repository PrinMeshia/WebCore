//! Control-flow directive parsing: @for, @if, @switch, @error.

use crate::ast::{Element, Span};
use crate::parser::elements::{extract_string_literal, parse_element};
use crate::parser::{ParseError, Rule};
use pest::iterators::Pair;

pub(super) fn parse_control_flow(pair: Pair<Rule>) -> Result<Element, ParseError> {
    let inner = pair
        .into_inner()
        .next()
        .ok_or_else(|| ParseError::new("Empty control flow"))?;

    match inner.as_rule() {
        Rule::for_loop => parse_for_loop(inner),
        Rule::if_statement => parse_if_block(inner),
        Rule::switch_statement => parse_switch(inner),
        Rule::error_block => parse_error_block(inner),
        _ => Err(ParseError::new(format!(
            "Unknown control flow: {:?}",
            inner.as_rule()
        ))),
    }
}

fn parse_for_loop(inner: Pair<Rule>) -> Result<Element, ParseError> {
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
                // for_key_value contains either for_key_braced or for_key_expr
                if let Some(key_val) = part.into_inner().next() {
                    for inner_kv in key_val.into_inner() {
                        match inner_kv.as_rule() {
                            Rule::for_key_braced => {
                                // Extract expression_content from braces
                                key = inner_kv.into_inner().next().map(|p| p.as_str().to_string());
                            }
                            Rule::for_key_expr => {
                                key = Some(inner_kv.as_str().to_string());
                            }
                            _ => {}
                        }
                    }
                }
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

fn parse_if_block(inner: Pair<Rule>) -> Result<Element, ParseError> {
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

fn parse_switch(inner: Pair<Rule>) -> Result<Element, ParseError> {
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
        let condition = format!("{switch_expr} === {val}");
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

fn parse_error_block(inner: Pair<Rule>) -> Result<Element, ParseError> {
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
