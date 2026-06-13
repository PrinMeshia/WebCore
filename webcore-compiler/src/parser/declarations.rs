//! Top-level declaration parsing: component, page, layout, app, store.

use crate::core::ast::{App, Span, Layout, Page, HeadBlock, HttpBlock, Component, StateVar, Prop, ComputedVar, WatchDef, StyleRule, StyleProperty, StyleItem, KeyframeStep};
use crate::parser::{ParseError, Rule};
use crate::parser::elements::{parse_element, extract_string_literal};
use pest::iterators::Pair;

pub(super) fn parse_app(pair: Pair<Rule>) -> Result<App, ParseError> {
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
        routes: std::collections::HashMap::new(),
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

pub(super) fn parse_layout(pair: Pair<Rule>) -> Result<Layout, ParseError> {
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

pub(super) fn parse_page(pair: Pair<Rule>) -> Result<Page, ParseError> {
    let span = Span::from_pest(pair.as_span());
    let mut inner = pair.into_inner();

    let name_token = inner
        .next()
        .ok_or_else(|| ParseError::with_span("Expected page name", span))?;
    let name = extract_string_literal(name_token.as_str());

    let mut head: Option<HeadBlock> = None;
    let mut content = Vec::new();
    for item in inner {
        match item.as_rule() {
            Rule::head_block => {
                head = Some(parse_head_block(item)?);
            }
            Rule::element => {
                content.push(parse_element(item)?);
            }
            _ => {}
        }
    }

    Ok(Page {
        name,
        head,
        content,
        span,
    })
}

pub(super) fn parse_head_block(pair: Pair<Rule>) -> Result<HeadBlock, ParseError> {
    let mut title: Option<String> = None;
    let mut metas: Vec<(String, String)> = Vec::new();

    for item in pair.into_inner() {
        match item.as_rule() {
            Rule::head_title => {
                if let Some(s) = item.into_inner().next() {
                    title = Some(extract_string_literal(s.as_str()));
                }
            }
            Rule::head_meta => {
                let mut parts = item.into_inner();
                let key = parts
                    .next()
                    .map(|p| p.as_str().to_string())
                    .unwrap_or_default();
                let val = parts
                    .next()
                    .map(|p| extract_string_literal(p.as_str()))
                    .unwrap_or_default();
                metas.push((key, val));
            }
            _ => {}
        }
    }

    Ok(HeadBlock { title, metas })
}

pub(super) fn parse_http_block(pair: Pair<Rule>) -> Result<HttpBlock, ParseError> {
    let mut method = String::new();
    let mut url = String::new();
    let mut into = String::new();

    for field in pair.into_inner() {
        if field.as_rule() == Rule::http_field {
            let mut parts = field.into_inner();
            let field_name = parts
                .next()
                .map(|p| p.as_str().to_string())
                .unwrap_or_default();
            let field_value = parts
                .next()
                .map(|p| {
                    // http_field_value wraps either string_literal or identifier
                    let inner = p.into_inner().next();
                    if let Some(inner_pair) = inner {
                        match inner_pair.as_rule() {
                            Rule::string_literal => extract_string_literal(inner_pair.as_str()),
                            _ => inner_pair.as_str().to_string(),
                        }
                    } else {
                        String::new()
                    }
                })
                .unwrap_or_default();

            match field_name.as_str() {
                "get" => {
                    method = "GET".to_string();
                    url = field_value;
                }
                "post" => {
                    method = "POST".to_string();
                    url = field_value;
                }
                "into" => {
                    into = field_value;
                }
                _ => {}
            }
        }
    }

    Ok(HttpBlock { method, url, into })
}

pub(super) fn parse_component(pair: Pair<Rule>) -> Result<Component, ParseError> {
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
        watches: Vec::new(),
        mount_body: None,
        destroy_body: None,
        http: None,
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
                Rule::watch_block => {
                    component.watches.push(parse_watch_block(section)?);
                }
                Rule::http_block => {
                    component.http = Some(parse_http_block(section)?);
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

    // Auto-inject `loading` and `error` state vars when an http block is present.
    // This saves the developer from having to declare them manually; if they already
    // declared their own, those declarations take precedence (we only add the missing ones).
    if component.http.is_some() {
        let has_loading = component.state.iter().any(|v| v.name == "loading");
        let has_error = component.state.iter().any(|v| v.name == "error");
        if !has_loading {
            component.state.push(StateVar {
                name: "loading".to_string(),
                type_: "Boolean".to_string(),
                default_value: Some("true".to_string()),
                span,
            });
        }
        if !has_error {
            component.state.push(StateVar {
                name: "error".to_string(),
                type_: "String".to_string(),
                default_value: Some(String::new()),
                span,
            });
        }
    }

    Ok(component)
}

pub(super) fn parse_props_block(pair: Pair<Rule>) -> Result<Vec<Prop>, ParseError> {
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

pub(super) fn parse_state_block(pair: Pair<Rule>) -> Result<Vec<StateVar>, ParseError> {
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
                .next().map_or_else(|| "Any".to_string(), |p| p.as_str().to_string());

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

pub(super) fn parse_computed_block(pair: Pair<Rule>) -> Result<Vec<ComputedVar>, ParseError> {
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

pub(super) fn parse_on_mount_block(pair: Pair<Rule>) -> Result<String, ParseError> {
    let raw = pair.into_inner().next().map_or("{}", |p| p.as_str());
    // Strip outer { } and trim
    let trimmed = raw.trim();
    let body = if trimmed.starts_with('{') && trimmed.ends_with('}') {
        trimmed[1..trimmed.len() - 1].trim()
    } else {
        trimmed
    };
    Ok(body.to_string())
}

/// Parse a `$watch varName => { body }` or `$watch $store.varName => { body }` block.
fn parse_watch_block(pair: Pair<Rule>) -> Result<WatchDef, ParseError> {
    let span = Span::from_pest(pair.as_span());
    let mut inner = pair.into_inner();
    let target_raw = inner
        .next()
        .map(|p| p.as_str().to_string())
        .unwrap_or_default();
    let (var_name, is_store) = if let Some(stripped) = target_raw.strip_prefix("$store.") {
        (stripped.to_string(), true)
    } else {
        (target_raw, false)
    };
    // on_mount_body captures `{ ... }` verbatim — strip the outer braces
    let body_raw = inner.next().map_or("{}", |p| p.as_str());
    let trimmed = body_raw.trim();
    let body = if trimmed.starts_with('{') && trimmed.ends_with('}') {
        trimmed[1..trimmed.len() - 1].trim().to_string()
    } else {
        trimmed.to_string()
    };
    Ok(WatchDef { var_name, is_store, body, span })
}

fn parse_style_property(pair: Pair<Rule>) -> Result<StyleProperty, ParseError> {
    let prop_span = Span::from_pest(pair.as_span());
    let mut inner = pair.into_inner();
    let name = inner.next().map(|p| p.as_str().to_string()).unwrap_or_default();
    let value = inner.next().map(|p| p.as_str().trim().to_string()).unwrap_or_default();
    Ok(StyleProperty { name, value, span: prop_span })
}

pub(super) fn parse_style_rule_pair(rule_pair: Pair<Rule>) -> Result<StyleRule, ParseError> {
    let span = Span::from_pest(rule_pair.as_span());
    let mut inner = rule_pair.into_inner();

    let selector = inner
        .next()
        .map(|p| p.as_str().trim().to_string())
        .unwrap_or_default();

    let mut properties = Vec::new();
    let mut nested = Vec::new();
    for item in inner {
        match item.as_rule() {
            Rule::style_property => {
                properties.push(parse_style_property(item)?);
            }
            Rule::nested_rule => {
                let nested_span = Span::from_pest(item.as_span());
                let mut nested_inner = item.into_inner();

                let nested_selector = nested_inner
                    .next()
                    .map(|p| p.as_str().trim().to_string())
                    .unwrap_or_default();

                let mut nested_props = Vec::new();
                for prop in nested_inner {
                    if prop.as_rule() == Rule::style_property {
                        nested_props.push(parse_style_property(prop)?);
                    }
                }
                nested.push(StyleRule {
                    selector: nested_selector,
                    properties: nested_props,
                    nested: vec![],
                    span: nested_span,
                });
            }
            _ => {}
        }
    }

    Ok(StyleRule {
        selector,
        properties,
        nested,
        span,
    })
}

pub(super) fn parse_style_block(pair: Pair<Rule>) -> Result<Vec<StyleItem>, ParseError> {
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
            Rule::keyframes_block => {
                let mut inner = item_pair.into_inner();
                let name = inner
                    .next()
                    .map(|p| p.as_str().to_string())
                    .unwrap_or_default();

                let mut steps = Vec::new();
                for step_pair in inner {
                    if step_pair.as_rule() == Rule::keyframe_step {
                        let mut sp = step_pair.into_inner();
                        let selector = sp
                            .next()
                            .map(|p| p.as_str().to_string())
                            .unwrap_or_default();
                        let mut properties = Vec::new();
                        for prop_pair in sp {
                            if prop_pair.as_rule() == Rule::style_property {
                                if let Ok(sp) = parse_style_property(prop_pair) {
                                    properties.push(sp);
                                }
                            }
                        }
                        steps.push(KeyframeStep { selector, properties });
                    }
                }

                items.push(StyleItem::Keyframes { name, steps });
            }
            _ => {}
        }
    }

    Ok(items)
}
