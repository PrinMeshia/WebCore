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
        Rule::tag_element => parse_tag(inner),
        Rule::text_element => parse_text_element(inner),
        _ => Err(ParseError::new(format!(
            "Unexpected element rule: {:?}",
            inner.as_rule()
        ))),
    }
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
                '{' => {
                    current_text.push('{');
                    i += 2;
                }
                '"' => {
                    current_text.push('"');
                    i += 2;
                }
                '\\' => {
                    current_text.push('\\');
                    i += 2;
                }
                other => {
                    current_text.push('\\');
                    current_text.push(other);
                    i += 2;
                }
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
                elements.push(Element::Interpolation(
                    var_name.trim().to_string(),
                    default_span,
                ));
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
pub(super) fn extract_string_literal(s: &str) -> String {
    let trimmed = s.trim();
    if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
        trimmed[1..trimmed.len() - 1].to_string()
    } else {
        trimmed.to_string()
    }
}
