//! Element rendering: dispatch over the `Element` AST node and rendering of
//! the structural directives (`@for`, `@if`/`@else`, fragments, error blocks).

use crate::codegen::attr_names;
use crate::core::ast::Element;
use crate::core::error::CompileError;
use crate::core::ssg::html_escape_text;
use std::fmt::Write as _;

use super::components::generate_component_element;
use super::tags::generate_tag_element;
use super::utils::html_escape;
use super::{GenContext, HandlerMapping};

pub(super) fn generate_elements(
    elements: &[Element],
    ctx: &mut GenContext,
    scope_id: Option<&str>,
) -> Result<(String, Vec<HandlerMapping>), CompileError> {
    let mut result = String::new();
    let mut all_handlers = Vec::new();

    for element in elements {
        let (element_html, handlers) = generate_element(element, ctx, scope_id)?;
        result.push_str(&element_html);
        result.push('\n');
        all_handlers.extend(handlers);
    }
    Ok((result, all_handlers))
}

pub(super) fn generate_element(
    element: &Element,
    ctx: &mut GenContext,
    scope_id: Option<&str>,
) -> Result<(String, Vec<HandlerMapping>), CompileError> {
    match element {
        Element::Text(text, _span) => Ok((html_escape(text), Vec::new())),
        Element::Tag {
            name,
            attributes,
            content,
            ..
        } => generate_tag_element(name, attributes, content, ctx, scope_id),
        Element::Slot(name, _span) => Ok((format!("<!-- Slot: {name} -->"), Vec::new())),
        Element::SlotContent { content, .. } => {
            // SlotContent consumed by slot matching; render children as fallback
            generate_elements(content, ctx, scope_id)
        }
        Element::Component {
            name,
            attributes,
            content,
            ..
        } => generate_component_element(name, attributes, content, ctx, scope_id),
        Element::Interpolation(expr, _span) => {
            // SSG: pre-render the initial value so the first paint shows real
            // content; the runtime overwrites it reactively after load.
            let initial = ctx
                .ssg
                .and_then(|ssg| ssg.eval_expr(expr))
                .map(|v| html_escape_text(&v))
                .unwrap_or_default();
            Ok((
                format!(
                    "<span {}=\"{}\">{}</span>",
                    attr_names::INTERPOLATION,
                    html_escape(expr),
                    initial
                ),
                Vec::new(),
            ))
        }
        Element::ErrorBlock { field, content, .. } => {
            let mut html = format!(
                "<div {}=\"{}\" style=\"display:none\">\n",
                attr_names::ERROR,
                html_escape(field)
            );
            let (content_html, handlers) = generate_elements(content, ctx, scope_id)?;
            html.push_str(&content_html);
            html.push_str("</div>\n");
            Ok((html, handlers))
        }
        Element::For {
            item,
            index,
            iterable,
            key,
            content,
            ..
        } => render_for_element(
            item,
            index.as_deref(),
            iterable,
            key.as_deref(),
            content,
            ctx,
            scope_id,
        ),
        Element::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => render_if_element(
            condition,
            then_branch,
            else_branch.as_deref(),
            ctx,
            scope_id,
        ),
        Element::Fragment { content, .. } => generate_elements(content, ctx, scope_id),
        Element::Defer { content, .. } => render_defer_element(content, ctx, scope_id),
    }
}

#[allow(clippy::too_many_arguments)]
fn render_for_element(
    item: &str,
    index: Option<&str>,
    iterable: &str,
    key: Option<&str>,
    content: &[Element],
    ctx: &mut GenContext,
    scope_id: Option<&str>,
) -> Result<(String, Vec<HandlerMapping>), CompileError> {
    // Detect range syntax: "0..5" or "N..M"
    let is_range = {
        let parts: Vec<&str> = iterable.splitn(2, "..").collect();
        parts.len() == 2
            && parts[0].trim().parse::<i64>().is_ok()
            && parts[1].trim().parse::<i64>().is_ok()
    };

    let mut open = format!(
        "<template {}=\"{}\" {}=\"{}\"",
        attr_names::FOR,
        item,
        attr_names::FOR_IN,
        iterable
    );
    if is_range {
        write!(
            open,
            " {}=\"{}\"",
            attr_names::FOR_RANGE,
            html_escape(iterable)
        )
        .expect("write! to String is infallible");
    }
    if let Some(idx) = index {
        write!(open, " {}=\"{}\"", attr_names::FOR_INDEX, html_escape(idx))
            .expect("write! to String is infallible");
    }
    if let Some(k) = key {
        write!(open, " {}=\"{}\"", attr_names::FOR_KEY, html_escape(k))
            .expect("write! to String is infallible");
    }
    if let Some(sid) = scope_id {
        write!(open, " {}=\"{}\"", attr_names::SCOPE, sid).expect("write! to String is infallible");
    }
    open.push('>');
    let (content_html, handlers) = generate_elements(content, ctx, scope_id)?;
    let result = format!(
        "{}\n{}</template>\n<div {}=\"{}\"></div>",
        open,
        content_html,
        attr_names::FOR_CONTAINER,
        iterable
    );
    Ok((result, handlers))
}

fn scope_attr_str(scope_id: Option<&str>) -> String {
    scope_id.map_or(String::new(), |sid| {
        format!(" {}=\"{}\"", attr_names::SCOPE, sid)
    })
}

fn render_defer_element(
    content: &[Element],
    ctx: &mut GenContext,
    scope_id: Option<&str>,
) -> Result<(String, Vec<HandlerMapping>), CompileError> {
    let scope_attr = scope_attr_str(scope_id);
    let mut result = format!(
        "<div {}=\"\" style=\"display:none\"{}>\n",
        attr_names::DEFER,
        scope_attr
    );
    let (content_html, handlers) = generate_elements(content, ctx, scope_id)?;
    result.push_str(&content_html);
    result.push_str("</div>\n");
    Ok((result, handlers))
}

fn render_if_element(
    condition: &str,
    then_branch: &[Element],
    else_branch: Option<&[Element]>,
    ctx: &mut GenContext,
    scope_id: Option<&str>,
) -> Result<(String, Vec<HandlerMapping>), CompileError> {
    let scope_attr = scope_attr_str(scope_id);
    // SSG: show/hide the branch on first paint when the condition is
    // statically known; bindIf() takes over reactively after load.
    let initial_cond = ctx.ssg.and_then(|ssg| ssg.eval_cond(condition));
    let if_style = match initial_cond {
        Some(true) => " style=\"display:block\"",
        Some(false) => " style=\"display:none\"",
        None => "",
    };
    let mut result = format!(
        "<div {}=\"{}\"{}{}>\n",
        attr_names::IF,
        html_escape(condition),
        scope_attr,
        if_style
    );
    let mut all_handlers = Vec::new();

    let (then_html, then_handlers) = generate_elements(then_branch, ctx, scope_id)?;
    result.push_str(&then_html);
    result.push_str("</div>\n");
    all_handlers.extend(then_handlers);

    if let Some(else_content) = else_branch {
        let else_style = match initial_cond {
            Some(true) => " style=\"display:none\"",
            Some(false) => " style=\"display:block\"",
            None => "",
        };
        writeln!(
            result,
            "<div {}=\"{}\"{}{}>",
            attr_names::IF_ELSE,
            html_escape(condition),
            scope_attr,
            else_style
        )
        .expect("write! to String is infallible");
        let (else_html, else_handlers) = generate_elements(else_content, ctx, scope_id)?;
        result.push_str(&else_html);
        result.push_str("</div>\n");
        all_handlers.extend(else_handlers);
    }
    Ok((result, all_handlers))
}
