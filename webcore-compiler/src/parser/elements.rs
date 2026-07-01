//! Element parsing: tags, components, slots, text, and interpolation.

use crate::core::ast::{Attribute, AttributeValue, Element, Span};
use crate::parser::{ParseError, Rule};
use pest::iterators::Pair;

pub(super) fn parse_element(pair: Pair<Rule>) -> Result<Element, ParseError> {
    let inner = pair
        .into_inner()
        .next()
        .ok_or_else(|| ParseError::new("Empty element"))?;

    match inner.as_rule() {
        Rule::control_flow => super::directives::parse_control_flow(inner),
        Rule::slot_element => parse_slot(inner),
        Rule::fragment_element => parse_fragment(inner),
        Rule::tag_element => parse_tag(inner),
        Rule::text_element => parse_text_element(inner),
        _ => Err(ParseError::new(format!(
            "Unexpected element rule: {:?}",
            inner.as_rule()
        ))),
    }
}

pub(super) fn parse_fragment(pair: Pair<Rule>) -> Result<Element, ParseError> {
    let span = Span::from_pest(pair.as_span());
    let mut content = Vec::new();
    for child in pair.into_inner() {
        if child.as_rule() == Rule::element {
            content.push(parse_element(child)?);
        }
    }
    Ok(Element::Fragment { content, span })
}

pub(super) fn parse_slot(pair: Pair<Rule>) -> Result<Element, ParseError> {
    let span = Span::from_pest(pair.as_span());
    let mut inner = pair.into_inner().peekable();

    let name = if inner
        .peek()
        .is_some_and(|p| p.as_rule() == Rule::identifier)
    {
        inner
            .next()
            .map_or_else(|| "content".to_string(), |p| p.as_str().to_string())
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

pub(super) fn parse_tag(pair: Pair<Rule>) -> Result<Element, ParseError> {
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
    if tag_name.chars().next().is_some_and(char::is_uppercase) {
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
fn parse_attribute(pair: Pair<Rule>) -> Result<Attribute, ParseError> {
    let span = Span::from_pest(pair.as_span());
    let mut inner = pair.into_inner();

    let first = inner
        .next()
        .ok_or_else(|| ParseError::with_span("Empty attribute", span))?;

    // Dispatch on what kind of attribute syntax was matched
    match first.as_rule() {
        Rule::spread_attr => {
            let ident = first
                .into_inner()
                .next()
                .map(|p| p.as_str().to_string())
                .unwrap_or_default();
            Ok(Attribute {
                name: "...".to_string(),
                value: AttributeValue::Spread(ident),
                span,
            })
        }
        Rule::shorthand_attr => {
            let ident = first
                .into_inner()
                .next()
                .map(|p| p.as_str().to_string())
                .unwrap_or_default();
            Ok(Attribute {
                name: ident.clone(),
                value: AttributeValue::Expression(ident),
                span,
            })
        }
        Rule::attr_name => {
            // Regular `name = value` attribute — first is attr_name, next is attr_value
            let name = first.as_str().to_string();
            let value = if let Some(val_pair) = inner.next() {
                if val_pair.as_rule() == Rule::attr_value {
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
        _ => Err(ParseError::with_span("Unknown attribute form", span)),
    }
}

pub(super) fn parse_tag_content(pair: Pair<Rule>) -> Result<Vec<Element>, ParseError> {
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

pub(super) fn parse_text_element(pair: Pair<Rule>) -> Result<Element, ParseError> {
    let span = Span::from_pest(pair.as_span());
    let text = pair.as_str();

    // Remove surrounding quotes
    let clean_text = if text.starts_with('"') && text.ends_with('"') {
        &text[1..text.len() - 1]
    } else {
        text
    };

    let mut elems = split_interpolated_text(clean_text);
    match elems.len() {
        0 => Ok(Element::Text(String::new(), span)),
        1 => Ok(elems.pop().expect("len checked")),
        _ => Ok(Element::Fragment {
            content: elems,
            span,
        }),
    }
}

/// Return `Some(offset_of_closing_brace)` (relative to `rest`, the slice just
/// after a `{`) when `rest` opens a real interpolation, else `None` — in which
/// case the `{` is a literal brace.
///
/// A `{` opens an interpolation only when it is immediately followed by a
/// non-space, non-`}` character and the body closes with `}` on the same line
/// without any nested `{`. This lets literal code samples carry unescaped
/// braces: `{ status: "x" }` (space after `{`), `{}` (empty), and multi-line
/// `component App {\n…\n}` are all treated as plain text, while `{count}` and
/// `{count + 1}` remain interpolations.
fn interpolation_close(rest: &[char]) -> Option<usize> {
    match rest.first() {
        Some(&c) if c == ' ' || c == '}' => return None,
        None => return None,
        _ => {}
    }
    for (k, &c) in rest.iter().enumerate() {
        match c {
            '}' => return Some(k),
            '{' | '\n' | '\r' => return None,
            _ => {}
        }
    }
    None
}

/// Split a string potentially containing `{var}` interpolations into Elements.
/// Handles escape sequences (`\{`, `\}`, `\"`, `\\`) for backward compatibility,
/// but bare literal braces no longer need escaping (see `interpolation_close`).
pub(super) fn split_interpolated_text(text: &str) -> Vec<Element> {
    let mut elements: Vec<Element> = Vec::new();
    let default_span = Span::default();
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0usize;
    let mut current_text = String::new();

    while i < len {
        if chars[i] == '\\' && i + 1 < len {
            match chars[i + 1] {
                '{' => current_text.push('{'),
                '}' => current_text.push('}'),
                '"' => current_text.push('"'),
                '\\' => current_text.push('\\'),
                other => {
                    current_text.push('\\');
                    current_text.push(other);
                }
            }
            i += 2;
        } else if chars[i] == '{' {
            if let Some(close) = interpolation_close(&chars[i + 1..]) {
                // Real interpolation — flush pending text first.
                if !current_text.is_empty() {
                    elements.push(Element::Text(
                        std::mem::take(&mut current_text),
                        default_span,
                    ));
                }
                let expr: String = chars[i + 1..i + 1 + close].iter().collect();
                elements.push(Element::Interpolation(
                    expr.trim().to_string(),
                    default_span,
                ));
                i += close + 2;
            } else {
                // Literal brace.
                current_text.push('{');
                i += 1;
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
pub(super) fn extract_string_literal(s: &str) -> String {
    let trimmed = s.trim();
    if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
        trimmed[1..trimmed.len() - 1].to_string()
    } else {
        trimmed.to_string()
    }
}
