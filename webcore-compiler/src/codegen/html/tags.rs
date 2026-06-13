//! Single `<tag>` element rendering: attributes (static, boolean, dynamic),
//! event handlers, validation attrs, `webc:img` defaults and SPA links.

use crate::codegen::attr_names;
use crate::core::ast::{Attribute, AttributeValue, Element};
use crate::core::error::CompileError;
use std::fmt::Write as _;

use super::attrs::{
    expand_bind_attrs, handle_class_binding, handle_event_attr, handle_ref_attr,
    handle_style_binding, handle_validation_attr,
};
use super::elements::generate_elements;
use super::utils::{html_escape, is_void_element, push_close_tag};
use super::{GenContext, HandlerMapping};

/// Generate HTML for a single `<tag>` element, including:
/// - CSS scope attribute (`data-v`)
/// - Static, boolean, and expression attributes
/// - Event handler attributes (`on:click` → `onclick="webcore_handle_click(...)"`)
/// - Validation data attributes (`validate:required` → `data-webcore-validate-required`)
/// - SPA-aware `<a href>` with `onclick="webcore_navigate(...)"` for internal links
pub(super) fn generate_tag_element(
    name: &str,
    attributes: &[Attribute],
    content: &[Element],
    ctx: &mut GenContext,
    scope_id: Option<&str>,
) -> Result<(String, Vec<HandlerMapping>), CompileError> {
    let mut result = String::new();
    let mut handlers = Vec::new();
    if name == "text" {
        return generate_elements(content, ctx, scope_id);
    }

    // Expand bind:attr={expr} → attr={expr} + on:event={expr = event.target.value}
    let expanded = expand_bind_attrs(attributes);
    let attributes = &expanded;

    let mapped_name = if name == "link" { "a" } else { name };
    let is_link = mapped_name == "a";
    let mut resolved_href: Option<String> = None;
    write!(result, "<{mapped_name}").expect("write! to String is infallible");

    // Add scope attribute for CSS scoping
    if let Some(sid) = scope_id {
        write!(result, " {}=\"{}\"", attr_names::SCOPE, sid)
            .expect("write! to String is infallible");
    }

    // Mark elements that have dynamic (expression) attribute bindings via data-webcore-attr-*
    if attributes.iter().any(|a| {
        !a.name.starts_with("on:")
            && !a.name.starts_with("class:")
            && matches!(&a.value, AttributeValue::Expression(_))
    }) {
        write!(result, " {}", attr_names::BOUND).expect("write! to String is infallible");
    }
    // Mark elements with class: bindings separately so bindClassBindings can use a targeted
    // selector instead of querySelectorAll('*') which scans the entire DOM.
    if attributes
        .iter()
        .any(|a| a.name.starts_with("class:") && matches!(&a.value, AttributeValue::Expression(_)))
    {
        write!(result, " {}", attr_names::CLASS_BOUND).expect("write! to String is infallible");
    }

    // Detect validate:* attributes and add data-webcore-field
    let has_validate = attributes.iter().any(|a| a.name.starts_with("validate:"));
    if has_validate {
        let field_name = attributes.iter().find(|a| a.name == "name").and_then(|a| {
            if let AttributeValue::String(v) = &a.value {
                Some(v.clone())
            } else {
                None
            }
        });
        if let Some(field) = field_name {
            write!(result, " data-webcore-field=\"{}\"", html_escape(&field))
                .expect("write! to String is infallible");
        }
    }

    // Single-pass attribute scan — avoids multiple O(n) passes over the attribute list.
    struct TagScan {
        has_webc_img: bool,
        has_alt: bool,
        has_loading: bool,
        has_decoding: bool,
        src_value: Option<String>,
    }
    let scan = {
        let mut s = TagScan {
            has_webc_img: false,
            has_alt: false,
            has_loading: false,
            has_decoding: false,
            src_value: None,
        };
        for a in attributes {
            match a.name.as_str() {
                "webc:img" => s.has_webc_img = true,
                "alt" => s.has_alt = true,
                "loading" => s.has_loading = true,
                "decoding" => s.has_decoding = true,
                "src" => {
                    if let AttributeValue::String(v) = &a.value {
                        s.src_value = Some(v.clone());
                    }
                }
                _ => {}
            }
        }
        s
    };

    // webc:img — smart image defaults (Feature A)
    // Detect on the *original* tag name (before link→a mapping) AND mapped name
    let is_img = name == "img";
    let has_webc_img = is_img && scan.has_webc_img;
    if has_webc_img {
        // Emit a11y warning if alt is absent
        if !scan.has_alt {
            eprintln!("warning[a11y]: <img> with webc:img is missing alt attribute");
        }
        // Inject loading="lazy" if not already present
        if !scan.has_loading {
            result.push_str(" loading=\"lazy\"");
        }
        // Inject decoding="async" if not already present
        if !scan.has_decoding {
            result.push_str(" decoding=\"async\"");
        }
        // Read image dimensions at compile time
        if let Some(root) = ctx.project_root {
            if let Some(src) = &scan.src_value {
                let rel = src.trim_start_matches('/');
                let img_path = root.join("public").join(rel);
                if img_path.exists() {
                    if let Ok(sz) = imagesize::size(&img_path) {
                        write!(result, " width=\"{}\" height=\"{}\"", sz.width, sz.height)
                            .expect("write! to String is infallible");
                    }
                }
            }
        }
    }

    // Generate attributes
    for attr in attributes {
        // Skip validate:* here — converted below after the loop
        if attr.name.starts_with("validate:") {
            continue;
        }
        // Skip webc:img — it's a compiler directive, not a real HTML attribute
        if attr.name == "webc:img" {
            continue;
        }
        // Skip on:event|debounce="N" modifier-only attributes (just a delay hint, no handler)
        if attr.name.starts_with("on:") && attr.name.contains("|debounce") {
            if let AttributeValue::String(_) = &attr.value {
                // This is purely a delay specifier — skip it (handled in JS codegen)
                continue;
            }
        }
        // ref:name=true → data-webcore-ref="name"
        if attr.name.starts_with("ref:") {
            if let Some(s) = handle_ref_attr(&attr.name) {
                result.push_str(&s);
            }
            continue;
        }
        // webc:transition="name" → data-webcore-transition="name"
        if attr.name == "webc:transition" {
            if let AttributeValue::String(value) = &attr.value {
                write!(
                    result,
                    " {}=\"{}\"",
                    attr_names::TRANSITION,
                    html_escape(value)
                )
                .expect("write! to String is infallible");
            }
            continue;
        }
        // style:prop={expr} → data-webcore-style-prop="expr"
        if attr.name.starts_with("style:") {
            if let AttributeValue::Expression(expr) = &attr.value {
                if let Some(s) = handle_style_binding(&attr.name, expr) {
                    result.push_str(&s);
                }
            }
            continue;
        }
        match &attr.value {
            AttributeValue::String(value) => {
                if is_link && attr.name == "to" {
                    resolved_href = Some(value.clone());
                } else {
                    write!(result, " {}=\"{}\"", attr.name, html_escape(value))
                        .expect("write! to String is infallible");
                }
            }
            AttributeValue::Boolean(true) => {
                write!(result, " {}", attr.name).expect("write! to String is infallible");
            }
            AttributeValue::Boolean(false) => {}
            AttributeValue::Expression(expr) => {
                if attr.name.starts_with("class:") {
                    // Conditional class binding: class:name={expr} → data-webcore-class-name="expr"
                    if let Some(s) = handle_class_binding(&attr.name, expr) {
                        result.push_str(&s);
                    }
                } else if attr.name.starts_with("on:") {
                    // Event handler: on:click={ count += 1 }
                    if let Some(s) = handle_event_attr(
                        &attr.name,
                        expr,
                        is_link,
                        ctx.prefix,
                        &mut ctx.counter,
                        &mut handlers,
                        &mut resolved_href,
                    ) {
                        result.push_str(&s);
                    }
                } else {
                    // Dynamic attribute: bound at runtime via bindAttrs()
                    write!(
                        result,
                        " data-webcore-attr-{}=\"{}\"",
                        attr.name,
                        html_escape(expr)
                    )
                    .expect("write! to String is infallible");
                }
            }
        }
    }

    // Emit validate:* attrs as data-webcore-validate-* attributes
    for attr in attributes
        .iter()
        .filter(|a| a.name.starts_with("validate:"))
    {
        if let Some(s) = handle_validation_attr(attr) {
            result.push_str(&s);
        }
    }

    if is_link {
        if let Some(h) = resolved_href {
            let has_nav = ctx
                .document
                .app
                .as_ref()
                .is_some_and(|a| !a.routes.is_empty());
            // Internal paths use data-webcore-nav for CSP-safe SPA navigation.
            // The JS runtime delegates via document.addEventListener('click', ...).
            if has_nav && h.starts_with('/') {
                write!(result, " href=\"{}\" data-webcore-nav", html_escape(&h))
                    .expect("write! to String is infallible");
            } else {
                write!(result, " href=\"{}\"", html_escape(&h))
                    .expect("write! to String is infallible");
            }
        } else if !attributes.iter().any(|a| a.name == "href") {
            result.push_str(" href=\"#\"");
        }
    }

    result.push('>');
    // Void elements (input, img, br, …) cannot have children and must not
    // get a closing tag — `</input>` is invalid HTML.
    if is_void_element(mapped_name) && content.is_empty() {
        return Ok((result, handlers));
    }
    let (content_html, content_handlers) = generate_elements(content, ctx, scope_id)?;
    // Drop the trailing per-child newline: whitespace before the closing tag
    // is redundant with the newline emitted after the element itself, and it
    // pollutes inline elements (`<span>x\n</span>`).
    result.push_str(content_html.trim_end_matches('\n'));
    push_close_tag(&mut result, mapped_name);
    handlers.extend(content_handlers);
    Ok((result, handlers))
}
