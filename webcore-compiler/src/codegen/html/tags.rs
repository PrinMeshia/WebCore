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

/// Warn when a `webc:img` source names a real file of `public/` but omits the
/// `/assets/` prefix under which that directory is served.
///
/// Such a URL used to fail twice in silence: no dimensions (the file was never
/// found where the lookup expected it) and a 404 in the browser (nothing is
/// emitted at the site root). Naming the form that works costs one line here
/// and saves an author the whole investigation.
fn warn_unprefixed_public_src(root: &std::path::Path, src: &str) {
    let Some(rel) = crate::core::asset_url::looks_like_unprefixed_public_url(src) else {
        return;
    };
    if root.join("public").join(rel).exists() {
        eprintln!(
            "warning[assets]: <img webc:img src=\"{src}\"> — les fichiers de public/ sont servis sous /assets/. Écrivez src=\"{}\" ; sinon : pas de dimensions, pas de srcset, et une 404 à l'affichage.",
            crate::core::asset_url::url_from_public_rel(rel)
        );
    }
}

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

    // Event delegation keys handlers by the element's `id`. An author-supplied
    // `id` is therefore reused as the handler id — emitting a second, generated
    // `id` would be a duplicate attribute (the browser keeps the first one) and
    // every handler on the element would become unreachable.
    let has_event_attrs = attributes
        .iter()
        .any(|a| a.name.starts_with("on:") && matches!(a.value, AttributeValue::Expression(_)));
    let author_id = attributes.iter().find(|a| a.name == "id").map(|a| &a.value);
    let element_id = if has_event_attrs {
        match author_id {
            Some(AttributeValue::String(id)) => id.clone(),
            Some(AttributeValue::Expression(_)) => {
                eprintln!(
                    "warning: <{name}> has both a dynamic `id={{…}}` and event handlers — \
                     the runtime id overwrites the delegation key and the handlers will not fire"
                );
                ctx.counter += 1;
                format!("{}btn{}", ctx.prefix, ctx.counter)
            }
            _ => {
                ctx.counter += 1;
                format!("{}btn{}", ctx.prefix, ctx.counter)
            }
        }
    } else {
        String::new()
    };
    // Only emit an `id` when we generated one — the author's is written by the
    // attribute loop below.
    if has_event_attrs && !matches!(author_id, Some(AttributeValue::String(_))) {
        write!(result, " id=\"{}\"", html_escape(&element_id))
            .expect("write! to String is infallible");
    }
    let mut event_descriptors: Vec<String> = Vec::new();

    // Add scope attribute for CSS scoping
    if let Some(sid) = scope_id {
        write!(result, " {}=\"{}\"", attr_names::SCOPE, sid)
            .expect("write! to String is infallible");
    }

    // Mark elements that have dynamic (expression) or spread attribute bindings.
    // Loop-scoped expressions are excluded: they emit per-item `data-webcore-fattr-*`
    // resolved by `fillItem`, not the global `bindAttrs` binder.
    if attributes.iter().any(|a| {
        if a.name.starts_with("on:") || a.name.starts_with("class:") {
            return false;
        }
        match &a.value {
            AttributeValue::Expression(e) => !ctx.emits_per_item(e),
            AttributeValue::Spread(_) => true,
            _ => false,
        }
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
        // Read image dimensions at compile time. The URL → path mapping lives in
        // `core::asset_url`: `public/` is served at `/assets/`, and resolving it
        // here by hand is what used to make `src` and `width`/`height` mutually
        // exclusive (one convention read the file, the other rewrote the URL).
        if let Some(root) = ctx.project_root {
            if let Some(src) = &scan.src_value {
                match crate::core::asset_url::public_rel_from_url(src) {
                    Some(rel) => {
                        let img_path = root.join("public").join(rel);
                        if img_path.exists() {
                            if let Ok(sz) = imagesize::size(&img_path) {
                                write!(result, " width=\"{}\" height=\"{}\"", sz.width, sz.height)
                                    .expect("write! to String is infallible");
                                // Responsive-images marker (#74): the post-build
                                // pass turns this into a `srcset` (or strips it)
                                // for raster sources. Vector images (svg) don't
                                // get variants. The value is the public-relative
                                // path, which that pass turns back into URLs
                                // through `asset_url`.
                                let ext = rel.rsplit('.').next().unwrap_or("").to_lowercase();
                                if matches!(ext.as_str(), "png" | "jpg" | "jpeg") {
                                    write!(
                                        result,
                                        " {}=\"{}|{}\"",
                                        attr_names::IMG_MARKER,
                                        html_escape(rel),
                                        sz.width
                                    )
                                    .expect("write! to String is infallible");
                                }
                            }
                        }
                    }
                    None => warn_unprefixed_public_src(root, src),
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
        // style:prop={expr} → data-webcore-style-prop="id"
        if attr.name.starts_with("style:") {
            if let AttributeValue::Expression(expr) = &attr.value {
                if let Some(s) = handle_style_binding(&attr.name, expr, attr.span, ctx) {
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
            AttributeValue::Spread(expr) => {
                // Spread: `...obj` — all properties of obj become attributes at runtime
                let id = ctx.register_expr(expr, attr.span);
                write!(result, " {}=\"{}\"", attr_names::SPREAD, html_escape(&id))
                    .expect("write! to String is infallible");
            }
            AttributeValue::Expression(expr) => {
                if attr.name.starts_with("class:") {
                    // Conditional class binding: class:name={expr} → data-webcore-class-name="id"
                    if let Some(s) = handle_class_binding(&attr.name, expr, attr.span, ctx) {
                        result.push_str(&s);
                    }
                } else if attr.name.starts_with("on:") {
                    // Event handler: on:click={ count += 1 }
                    if let Some(descriptor) = handle_event_attr(
                        &attr.name,
                        expr,
                        is_link,
                        &element_id,
                        &mut handlers,
                        &mut resolved_href,
                    ) {
                        event_descriptors.push(descriptor);
                    }
                } else if ctx.emits_per_item(expr) {
                    // Per-item attribute inside a runtime `@for`: emit the raw
                    // loop expression for `fillItem` to resolve per item, not a
                    // global closure ID (which has no `it` in scope). A shape
                    // `resolveScoped` cannot walk stays here too — a global
                    // closure would throw and abort the binder — and is reported.
                    if let Some(var) = ctx.unresolvable_loop_var(expr) {
                        super::warn_unresolvable_loop_expr(
                            expr.trim(),
                            var,
                            &format!("attribut `{}`", attr.name),
                        );
                    }
                    write!(
                        result,
                        " {}{}=\"{}\"",
                        attr_names::FOR_ATTR_PREFIX,
                        attr.name,
                        html_escape(expr.trim())
                    )
                    .expect("write! to String is infallible");
                } else {
                    // Dynamic attribute: bound at runtime via bindAttrs().
                    // SSG (#45): when the expression is statically known
                    // (e.g. `aria-label={t("key")}`, `href={base}`), also emit
                    // the resolved value as a real attribute so it is present
                    // in the no-JS HTML — for crawlers, screen readers and the
                    // first paint. The runtime overwrites it reactively (and on
                    // locale switch) via `data-webcore-attr-*`.
                    if let Some(value) = ctx.ssg.and_then(|ssg| ssg.eval_expr(expr)) {
                        write!(result, " {}=\"{}\"", attr.name, html_escape(&value))
                            .expect("write! to String is infallible");
                    }
                    let id = ctx.register_expr(expr, attr.span);
                    write!(
                        result,
                        " data-webcore-attr-{}=\"{}\"",
                        attr.name,
                        html_escape(&id)
                    )
                    .expect("write! to String is infallible");
                }
            }
        }
    }

    // One `data-webcore-e` per element, listing every delegated event type:
    // `data-webcore-e="input,blur|stop"`.
    if !event_descriptors.is_empty() {
        write!(
            result,
            " data-webcore-e=\"{}\"",
            html_escape(&event_descriptors.join(","))
        )
        .expect("write! to String is infallible");
    }

    // Emit validate:* attrs as data-webcore-validate-* attributes
    for attr in attributes
        .iter()
        .filter(|a| a.name.starts_with("validate:"))
    {
        if let Some(s) = handle_validation_attr(attr, ctx.ssg) {
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
