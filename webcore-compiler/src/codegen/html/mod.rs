//! HTML Code Generator

mod analysis;
mod attrs;
mod components;
mod elements;
mod minify;
mod props;
mod shell;
mod slots;
mod tags;
mod utils;

use crate::core::ast::{HeadBlock, Span, WebCoreDocument};
use crate::core::error::CompileError;
use crate::core::ssg::SsgContext;
#[cfg(test)]
use std::fmt::Write as _;
use std::path::Path;

use analysis::{document_needs_js, find_layout};
use shell::emit_html_shell;
#[cfg(test)]
use shell::{generate_component_content, generate_layout_shell, generate_page_content};
use slots::generate_layout_with_page_and_components;

use crate::codegen::js::collect_state_variables;

pub(crate) use analysis::collect_page_components;
pub(crate) use minify::minify_html;

/// Placeholder emitted for a `{$build.<field>}` interpolation. Resolved by the
/// build's post-pass once output sizes are known — see
/// `cli::build::substitute_build_vars`. Shared so codegen and the pass agree.
pub(crate) fn build_var_placeholder(field: &str) -> String {
    format!("\u{2063}wcbuild:{field}\u{2063}")
}

// Options passed from the build to influence the page shell
#[derive(Debug, Clone)]
pub struct HtmlPageOptions {
    pub lang: String,
    pub title: String,
    pub extra_css_files: Vec<String>,
    /// When set (prod mode), this CSS is inlined in a `<style>` tag in `<head>`
    /// and the full stylesheet is loaded deferred (non render-blocking).
    pub critical_css: Option<String>,
    /// When set (prod mode with CSP enabled), emitted as a
    /// `<meta http-equiv="Content-Security-Policy" content="...">` tag.
    pub csp_meta: Option<String>,
    /// Production mode: enables JS minification and cleanup of data-webcore-* attrs.
    /// Default: false (dev mode).
    pub prod: bool,
    /// When true (dev mode), generate a source map JSON for the inline script.
    /// Default: false.
    pub source_maps: bool,
    /// When true, the full WebCore runtime is inlined in each page's `<script>`.
    /// When false, pages reference a single shared `/assets/webcore.js` (built
    /// from the union of all pages' expressions + handlers) so the browser
    /// caches it once across the whole site. Default: true (legacy / tests).
    pub inline_runtime: bool,
    /// Absolute site URL (`[app] url` in webc.toml). When set, root-relative
    /// `og:image` / `twitter:image` values are rewritten to absolute URLs
    /// (social crawlers require them). Default: None.
    pub site_url: Option<String>,
    /// Absolute canonical URL for this page (`site_url` + route). When set,
    /// emitted as `<link rel="canonical">`. Default: None.
    pub canonical: Option<String>,
    /// PWA head data. When set, the manifest link, theme-color and Apple
    /// web-app meta/icon tags are emitted into `<head>`. Default: None.
    pub pwa: Option<PwaHead>,
    /// SSG i18n (#46): `(hreflang, href)` alternates emitted as
    /// `<link rel="alternate" hreflang="…">` in `<head>`. Includes one entry
    /// per locale plus `x-default`. Empty when localized static pages are off.
    pub hreflang_alternates: Vec<(String, String)>,
}

/// Head-level PWA data emitted when a `[pwa]` section is configured.
#[derive(Debug, Clone)]
pub struct PwaHead {
    pub theme_color: String,
    pub short_name: String,
    /// Root-relative Apple touch icon path (fingerprinted by the asset pass).
    pub apple_icon: String,
}

impl Default for HtmlPageOptions {
    fn default() -> Self {
        HtmlPageOptions {
            lang: "en".into(),
            title: String::new(),
            extra_css_files: vec![],
            critical_css: None,
            csp_meta: None,
            prod: false,
            source_maps: false,
            inline_runtime: true,
            site_url: None,
            canonical: None,
            pwa: None,
            hreflang_alternates: Vec::new(),
        }
    }
}

/// Tracks a compiled event handler so the JS runtime can wire it up.
///
/// `id` is a page-scoped unique string (`<prefix>btn<n>`) used as the
/// `data-webcore-handler` HTML attribute value and as the key in the
/// generated `onclick`/`onsubmit`/etc. handler map.
#[derive(Debug, Clone)]
pub struct HandlerMapping {
    pub id: String,
    pub event_type: String,
    pub expression: String,
}

/// Output of generating HTML for one page.
pub struct HtmlGenerationResult {
    pub html: String,
    /// All event handlers collected while generating this page's HTML,
    /// to be emitted into `webcore.js` as `onclick`/`onsubmit` assignments.
    pub handlers: Vec<HandlerMapping>,
    /// Compiled expression map: (id, JS closure string). Populated during HTML generation.
    pub compiled_exprs: Vec<(String, String)>,
    /// Source spans for each entry in `compiled_exprs` (parallel vec).
    pub expr_spans: Vec<Span>,
    /// Source map v3 JSON, when `HtmlPageOptions::source_maps` was true.
    pub source_map_json: Option<String>,
}

/// Shared, page-scoped generation state threaded through the HTML emitters.
///
/// Bundles what used to be five separate parameters (`document`, `counter`,
/// `prefix`, `project_root`, …) so element generators only take the varying
/// per-node arguments plus the current CSS `scope_id`.
pub(super) struct GenContext<'a> {
    pub document: &'a WebCoreDocument,
    /// Page-scoped id prefix for generated handler ids (`<prefix>btn<n>`).
    pub prefix: &'a str,
    /// Project root for compile-time asset lookups (`webc:img` dimensions).
    pub project_root: Option<&'a Path>,
    /// SSG pre-rendering context: when set, interpolation spans are filled
    /// with their initial value and `@if`/`@else` divs get an initial
    /// `display` style at emission time.
    pub ssg: Option<&'a SsgContext<'a>>,
    /// Monotonic counter for unique handler ids within the page.
    pub counter: usize,
    // v3: compiled expression fields
    /// Pre-compiled variable regexes for read-expression compilation.
    /// When `None`, `register_expr` returns the raw expression (v2 compat / tests).
    pub compiled_vars: Option<&'a crate::codegen::js::js_events::CompiledVars>,
    /// Accumulated (id, closure) pairs for all read expressions registered this page.
    pub expr_map: Vec<(String, String)>,
    /// Source spans for each entry in `expr_map` (parallel vec).
    pub expr_spans: Vec<Span>,
    /// Counter for unique expression IDs (`e0`, `e1`, …).
    pub expr_counter: usize,
    /// Whether the document uses `$route.` param expressions (for closure compilation).
    pub has_route_params: bool,
    /// Whether the document uses `$query.` param expressions (for closure compilation).
    pub has_query_params: bool,
    /// Names of the loop variables (items + indices) currently in scope, innermost
    /// last. Interpolations/attributes referencing one of these are resolved per
    /// item at runtime by `fillItem`, so they must emit the raw expression rather
    /// than a global `_e` closure ID (which would evaluate the loop var in global
    /// scope and throw `ReferenceError`).
    pub loop_vars: Vec<String>,
}

impl<'a> GenContext<'a> {
    /// True when `expr` is a shape the runtime can resolve against a loop item.
    ///
    /// This is one half of a pair: the other is `resolveScoped` in the emitted
    /// runtime, and the two must accept exactly the same language. They used not
    /// to. This side tested a bare prefix (`expr.starts_with("post.")`) while
    /// `resolveScoped` splits everything after the item name on `.` and walks the
    /// object — so `{post.likes + 1}` was emitted raw, then looked up as a
    /// property literally named `likes + 1`, found nothing, and wrote an empty
    /// string. A loud `ReferenceError` had become a silent blank.
    ///
    /// So the predicate now matches the resolvable shapes and nothing else: the
    /// item or index name alone, or plain property accesses on it — `post`, `i`,
    /// `post.title`, `post.author.name`.
    pub(super) fn is_loop_scoped(&self, expr: &str) -> bool {
        let e = expr.trim();
        self.loop_vars.iter().any(|v| is_property_path_on(e, v))
    }

    /// True when `expr` must be emitted raw for `fillItem` rather than compiled
    /// into the global `_e` map — the single rule the three emit sites share.
    ///
    /// It covers the unresolvable shapes too, on purpose: a global closure over a
    /// loop variable throws, and an uncaught throw inside the binder's `forEach`
    /// takes the rest of the page's bindings with it. Emitting raw keeps the
    /// damage to the one element, and the build warns.
    pub(super) fn emits_per_item(&self, expr: &str) -> bool {
        self.is_loop_scoped(expr) || self.unresolvable_loop_var(expr).is_some()
    }

    /// True when `expr` reads a loop variable in a shape the runtime *cannot*
    /// resolve — the case the caller has to report rather than emit silently.
    pub(super) fn unresolvable_loop_var(&self, expr: &str) -> Option<&str> {
        let e = expr.trim();
        if self.is_loop_scoped(e) {
            return None;
        }
        self.loop_vars
            .iter()
            .find(|v| mentions_identifier(e, v))
            .map(String::as_str)
    }

    /// Register a read expression: compile it to a closure, assign an ID, and return the ID.
    /// Falls back to returning the raw expression when compiled_vars is not set (v2 compat / tests).
    pub(super) fn register_expr(&mut self, expr: &str, span: Span) -> String {
        if let Some(vars) = self.compiled_vars {
            let closure = crate::codegen::js::js_events::compile_read_expr(
                expr,
                vars,
                self.has_route_params,
                self.has_query_params,
            );
            // Prefix with the page-scoped id prefix so expression IDs stay
            // unique across pages — required when all pages share one runtime
            // file whose `_e` map is the union of every page's expressions.
            let id = format!("{}e{}", self.prefix, self.expr_counter);
            self.expr_counter += 1;
            self.expr_map.push((id.clone(), closure));
            self.expr_spans.push(span);
            id
        } else {
            self.expr_spans.push(span);
            expr.to_string()
        }
    }
}

/// Report an expression that reads a `@for` variable in a shape the runtime
/// cannot evaluate per item.
///
/// Neither available path is right for it: emitted raw, `resolveScoped` writes
/// an empty string; compiled to a global closure, it throws `ReferenceError`
/// because the loop variable exists in no scope but the loop's. Both are
/// silent-ish failures at a place where the compiler knows the answer, so it
/// says so, and names the shapes that do work.
pub(super) fn warn_unresolvable_loop_expr(expr: &str, var: &str, where_: &str) {
    eprintln!(
        "warning[for]: `{{{expr}}}` ({where_}) lit la variable de boucle `{var}` dans une forme non résoluble par élément. \
Seuls `{var}` et ses accès de propriété (`{var}.champ`, `{var}.a.b`) le sont — une expression calculée rendra du vide. \
Préparez la valeur dans les données, ou passez `{var}` en prop à un composant."
    );
}

/// Characters that can appear inside a JS identifier.
fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$'
}

/// True when `expr` is `name`, or `name` followed only by plain property
/// accesses (`name.a`, `name.a.b`).
///
/// This is the language `resolveScoped` speaks, spelled out on the compiler
/// side: it takes the text after `name.`, splits it on `.`, and walks the
/// object one key at a time. So every segment has to be a bare identifier —
/// no operators, no calls, no indexing, no trailing dot.
pub(crate) fn is_property_path_on(expr: &str, name: &str) -> bool {
    let Some(rest) = expr.strip_prefix(name) else {
        return false;
    };
    if rest.is_empty() {
        return true;
    }
    let Some(path) = rest.strip_prefix('.') else {
        // `postX` merely starts with `post`; it is a different name.
        return false;
    };
    !path.is_empty()
        && path
            .split('.')
            .all(|seg| !seg.is_empty() && seg.chars().all(is_ident_char))
}

/// True when `name` appears in `expr` as a whole identifier rather than as a
/// fragment of a longer one — `post` in `post + 1` but not in `postCount`.
pub(crate) fn mentions_identifier(expr: &str, name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let bytes = expr.as_bytes();
    let mut from = 0;
    while let Some(rel) = expr[from..].find(name) {
        let start = from + rel;
        let end = start + name.len();
        let before_ok = start == 0 || !is_ident_char(bytes[start - 1] as char);
        let after_ok = end == expr.len() || !is_ident_char(bytes[end] as char);
        if before_ok && after_ok {
            return true;
        }
        from = start + 1;
    }
    false
}

#[cfg(test)]
pub(crate) fn generate_spa_html(
    document: &WebCoreDocument,
    options: &HtmlPageOptions,
) -> Result<HtmlGenerationResult, CompileError> {
    let layout = find_layout(document)?;

    // SPA always needs JS (routing)
    let needs_js = true;
    let mut html = emit_html_shell(
        &options.title,
        &options.lang,
        &options.extra_css_files,
        needs_js,
        options.critical_css.as_deref(),
        options.csp_meta.as_deref(),
        None,
        options.site_url.as_deref(),
        options.canonical.as_deref(),
        options.pwa.as_ref(),
        &options.hreflang_alternates,
        None,
    );

    // Generate layout shell (without page content, just the structure)
    let (layout_content, mut all_handlers) = generate_layout_shell(layout, document, None)?;
    html.push_str(&layout_content);

    // Add routing container
    html.push_str("  <div id=\"webcore-router\" style=\"display: none;\">\n");

    // Generate all pages as hidden divs
    for (page_name, page) in &document.pages {
        let (page_html, handlers) = generate_page_content(page, document, None)?;
        writeln!(
            html,
            "    <div id=\"page-{}\" data-route=\"/{}\">",
            page_name.to_lowercase(),
            page_name.to_lowercase()
        )
        .expect("write! to String is infallible");
        html.push_str(&page_html);
        html.push_str("    </div>\n");
        all_handlers.extend(handlers);
    }

    // Generate all page components as hidden divs
    for (component_name, component) in &document.components {
        if component_name.ends_with("Page") {
            let (page_html, handlers) = generate_component_content(component, document, None)?;
            let route_name = component_name.replace("Page", "").to_lowercase();
            writeln!(
                html,
                "    <div id=\"page-{route_name}\" data-route=\"/{route_name}\">"
            )
            .expect("write! to String is infallible");
            html.push_str(&page_html);
            html.push_str("    </div>\n");
            all_handlers.extend(handlers);
        }
    }

    html.push_str("  </div>\n");

    html.push_str("  <script defer src=\"/assets/webcore.js\"></script>\n");
    html.push_str("</body>\n</html>");

    Ok(HtmlGenerationResult {
        html,
        handlers: all_handlers,
        compiled_exprs: vec![],
        expr_spans: vec![],
        source_map_json: None,
    })
}

/// Merge the site-wide head (`app { head { } }`) with a page's `head { }`.
///
/// Precedence — page over global: `title` and `favicon` use the page value
/// when set, else the global one; `meta` entries are the union of both, with
/// global metas first and a page meta overriding a global one sharing its key.
/// Returns `None` only when neither side declares a head block.
fn merge_head(global: Option<&HeadBlock>, page: Option<&HeadBlock>) -> Option<HeadBlock> {
    if global.is_none() && page.is_none() {
        return None;
    }
    let mut metas: Vec<crate::core::ast::HeadMeta> = Vec::new();
    if let Some(g) = global {
        metas.extend(g.metas.iter().cloned());
    }
    if let Some(p) = page {
        for meta in &p.metas {
            // A page meta overrides a global one only when both key AND extra
            // attributes match — so `theme-color` variants that differ by
            // `media` (light/dark) coexist instead of clobbering each other.
            if let Some(slot) = metas
                .iter_mut()
                .find(|m| m.key == meta.key && m.extra == meta.extra)
            {
                slot.value = meta.value.clone();
            } else {
                metas.push(meta.clone());
            }
        }
    }
    // Links have no unique key to dedupe on, so global and page links are both
    // kept (global first, then page).
    let mut links: Vec<Vec<(String, String)>> = Vec::new();
    if let Some(g) = global {
        links.extend(g.links.iter().cloned());
    }
    if let Some(p) = page {
        links.extend(p.links.iter().cloned());
    }
    // JSON-LD is a single object per page: the page's block replaces the
    // global one when present, otherwise the global block is used.
    let jsonld = match page {
        Some(p) if !p.jsonld.is_empty() => p.jsonld.clone(),
        _ => global.map(|g| g.jsonld.clone()).unwrap_or_default(),
    };
    let pick =
        |f: fn(&HeadBlock) -> Option<String>| page.and_then(f).or_else(|| global.and_then(f));
    Some(HeadBlock {
        title: pick(|h| h.title.clone()),
        metas,
        links,
        jsonld,
        favicon: pick(|h| h.favicon.clone()),
    })
}

/// Scan the document AST to check whether any expression uses `$query.`.
fn document_has_query_params(document: &WebCoreDocument) -> bool {
    use crate::core::ast::{walk_elements, AttributeValue, Element};
    fn check_elements(elements: &[Element]) -> bool {
        let mut found = false;
        walk_elements(elements, &mut |e| match e {
            Element::Interpolation(expr, _) => found |= expr.contains("$query."),
            Element::Text(t, _) => found |= t.contains("$query."),
            Element::Tag { attributes, .. } => {
                found |= attributes.iter().any(|a| match &a.value {
                    AttributeValue::Expression(expr) | AttributeValue::Spread(expr) => {
                        expr.contains("$query.")
                    }
                    AttributeValue::String(s) => s.contains("$query."),
                    _ => false,
                })
            }
            Element::For { iterable, .. } => found |= iterable.contains("$query."),
            Element::If { condition, .. } => found |= condition.contains("$query."),
            _ => {}
        });
        found
    }
    document.pages.values().any(|p| check_elements(&p.content))
        || document
            .components
            .values()
            .any(|c| check_elements(&c.view))
        || document
            .layouts
            .values()
            .any(|l| check_elements(&l.content))
}

/// Generate one full page (shell + layout + content).
///
/// `ssg` enables compile-time pre-rendering of initial state values; pass
/// `None` to emit runtime-only bindings (used by most golden tests).
pub(crate) fn generate_page(
    document: &WebCoreDocument,
    page_name: &str,
    options: &HtmlPageOptions,
    project_root: Option<&Path>,
    ssg: Option<&SsgContext>,
) -> Result<HtmlGenerationResult, CompileError> {
    // Find the page
    let page = document
        .pages
        .get(page_name)
        .ok_or_else(|| CompileError::MissingPage {
            name: page_name.to_string(),
        })?;

    let layout = find_layout(document)?;

    // Merge the site-wide `app { head { } }` with this page's `head { }`
    // (page wins on title/favicon and on any meta key it redefines).
    let merged_head = merge_head(
        document.app.as_ref().and_then(|a| a.head.as_ref()),
        page.head.as_ref(),
    );

    // critical_css injects a deferred <link> whose media swap requires JS;
    // force needs_js=true so webcore.js is included even on otherwise static pages.
    let needs_js = document_needs_js(document, page_name) || options.critical_css.is_some();
    let mut html = emit_html_shell(
        &options.title,
        &options.lang,
        &options.extra_css_files,
        needs_js,
        options.critical_css.as_deref(),
        options.csp_meta.as_deref(),
        merged_head.as_ref(),
        options.site_url.as_deref(),
        options.canonical.as_deref(),
        options.pwa.as_ref(),
        &options.hreflang_alternates,
        ssg,
    );

    // Build CompiledVars for v3 expression compilation
    let state_vars = collect_state_variables(document);
    let compiled_vars = crate::codegen::js::js_events::CompiledVars::new(&state_vars);

    // Detect route params: any route pattern contains `:param`
    let has_route_params = document
        .app
        .as_ref()
        .is_some_and(|a| a.routes.keys().any(|p| p.contains(':')));
    // Detect query params: any expression/text contains `$query.`
    let has_query_params = document_has_query_params(document);

    // Generate layout content, replacing slots with page content (v3: compiles exprs to closures)
    let layout_result = generate_layout_with_page_and_components(
        layout,
        page,
        document,
        project_root,
        ssg,
        Some(&compiled_vars),
        has_route_params,
        has_query_params,
    )?;
    html.push_str(&layout_result.html);

    let mut source_map_json: Option<String> = None;

    if needs_js && !options.inline_runtime {
        // Shared-runtime mode: reference the single cached /assets/webcore.js
        // (built from the union of every page's expressions + handlers). The
        // `webcore.js` placeholder is rewritten to the hashed filename by the
        // asset pipeline. No per-page inline runtime.
        html.push_str("  <script defer src=\"/assets/webcore.js\"></script>\n");
    } else if needs_js {
        let inline_js_result = crate::codegen::js::generate_inline_js(
            &layout_result.handlers,
            &layout_result.compiled_exprs,
            if options.source_maps {
                &layout_result.expr_spans
            } else {
                &[]
            },
            document,
            options.prod,
        );
        let mut script_content = inline_js_result.js;

        // Build source map when requested and there are expression mappings
        if options.source_maps && !inline_js_result.expr_mappings.is_empty() {
            // Find the source file for this page (use page_name as fallback)
            let source_name = format!("{page_name}.webc");
            // source_files maps basename → PathBuf; read content for source map embedding
            let source_content = document
                .source_files
                .get(&source_name)
                .and_then(|path| std::fs::read_to_string(path).ok())
                .unwrap_or_default();
            let mut builder =
                crate::codegen::js::SourceMapBuilder::new(&source_name, source_content);
            for (output_line, span) in &inline_js_result.expr_mappings {
                // Span.line is 1-indexed; source map uses 0-indexed
                let source_line = if span.line > 0 { span.line - 1 } else { 0 };
                builder.add(crate::codegen::js::SourceMapMapping {
                    output_line: *output_line,
                    output_col: 0,
                    source_line,
                    source_col: span.col.saturating_sub(1),
                });
            }
            let map_json = builder.build();
            // Append source mapping URL comment to inline script
            script_content.push_str(&format!("\n//# sourceMappingURL={page_name}.js.map\n"));
            source_map_json = Some(map_json);
        }

        html.push_str("  <script>\n");
        html.push_str(&script_content);
        html.push_str("  </script>\n");
    }
    html.push_str("</body>\n</html>");

    Ok(HtmlGenerationResult {
        html,
        handlers: layout_result.handlers,
        compiled_exprs: layout_result.compiled_exprs,
        expr_spans: layout_result.expr_spans,
        source_map_json,
    })
}

/// Test-only alias of [`generate_page`] without root/SSG, kept for the golden tests.
#[cfg(test)]
pub(crate) fn generate_html(
    document: &WebCoreDocument,
    page_name: &str,
    options: &HtmlPageOptions,
) -> Result<HtmlGenerationResult, CompileError> {
    generate_page(document, page_name, options, None, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ast::{Attribute, AttributeValue, Element, Layout, Page, Span};

    fn make_button_page(page_name: &str) -> Page {
        Page {
            name: page_name.to_string(),
            head: None,
            content: vec![Element::Tag {
                name: "button".to_string(),
                attributes: vec![Attribute {
                    name: "on:click".to_string(),
                    value: AttributeValue::Expression("count += 1".to_string()),
                    span: Span::default(),
                }],
                content: vec![],
                span: Span::default(),
            }],
            span: Span::default(),
        }
    }

    fn make_doc_with_pages(pages: Vec<(&str, Page)>) -> WebCoreDocument {
        let mut doc = WebCoreDocument {
            app: None,
            store: vec![],
            store_computed: vec![],
            locales: std::collections::BTreeMap::new(),
            default_locale: String::new(),
            wasm_module: None,
            view_transitions: false,
            layouts: std::collections::BTreeMap::new(),
            pages: std::collections::BTreeMap::new(),
            components: std::collections::BTreeMap::new(),
            imports: vec![],
            data_imports: std::collections::BTreeMap::new(),
            component_imports: vec![],
            page_imports: std::collections::BTreeMap::new(),
            source_files: std::collections::BTreeMap::new(),
        };
        doc.layouts.insert(
            "MainLayout".to_string(),
            Layout {
                name: "MainLayout".to_string(),
                content: vec![Element::Slot("content".to_string(), Span::default())],
                span: Span::default(),
            },
        );
        for (name, page) in pages {
            doc.pages.insert(name.to_string(), page);
        }
        doc
    }

    #[test]
    fn page_handlers_do_not_collide() {
        let home = make_button_page("home");
        let about = make_button_page("about");
        let doc = make_doc_with_pages(vec![("home", home), ("about", about)]);

        let opts = HtmlPageOptions {
            lang: "en".to_string(),
            title: "t".to_string(),
            extra_css_files: vec![],
            critical_css: None,
            csp_meta: None,
            prod: false,
            source_maps: false,
            inline_runtime: true,
            site_url: None,
            canonical: None,
            pwa: None,
            hreflang_alternates: vec![],
        };
        let res = generate_spa_html(&doc, &opts).expect("spa ok");

        let ids: Vec<&str> = res.handlers.iter().map(|h| h.id.as_str()).collect();
        // All handler IDs must be unique
        let unique: std::collections::HashSet<&str> = ids.iter().copied().collect();
        assert_eq!(ids.len(), unique.len(), "duplicate handler IDs: {:?}", ids);
        // IDs must carry the page prefix
        assert!(
            res.handlers.iter().any(|h| h.id.starts_with("home")),
            "expected a 'home' prefixed handler, got {:?}",
            ids
        );
        assert!(
            res.handlers.iter().any(|h| h.id.starts_with("about")),
            "expected an 'about' prefixed handler, got {:?}",
            ids
        );
    }

    #[test]
    fn dynamic_attr_emits_data_webcore_attr() {
        let mut doc = WebCoreDocument {
            app: None,
            store: vec![],
            store_computed: vec![],
            locales: std::collections::BTreeMap::new(),
            default_locale: String::new(),
            wasm_module: None,
            view_transitions: false,
            layouts: std::collections::BTreeMap::new(),
            pages: std::collections::BTreeMap::new(),
            components: std::collections::BTreeMap::new(),
            imports: vec![],
            data_imports: std::collections::BTreeMap::new(),
            component_imports: vec![],
            page_imports: std::collections::BTreeMap::new(),
            source_files: std::collections::BTreeMap::new(),
        };
        doc.layouts.insert(
            "MainLayout".to_string(),
            Layout {
                name: "MainLayout".to_string(),
                content: vec![Element::Slot("content".to_string(), Span::default())],
                span: Span::default(),
            },
        );
        doc.pages.insert(
            "test".to_string(),
            Page {
                name: "test".to_string(),
                head: None,
                content: vec![Element::Tag {
                    name: "div".to_string(),
                    attributes: vec![Attribute {
                        name: "class".to_string(),
                        value: AttributeValue::Expression("dynamicClass".to_string()),
                        span: Span::default(),
                    }],
                    content: vec![],
                    span: Span::default(),
                }],
                span: Span::default(),
            },
        );
        let opts = HtmlPageOptions {
            lang: "fr".to_string(),
            title: "t".to_string(),
            extra_css_files: vec![],
            critical_css: None,
            csp_meta: None,
            prod: false,
            source_maps: false,
            inline_runtime: true,
            site_url: None,
            canonical: None,
            pwa: None,
            hreflang_alternates: vec![],
        };
        let res = generate_html(&doc, "test", &opts).expect("html ok");
        assert!(
            res.html.contains("data-webcore-bound"),
            "marker attribute missing"
        );
        // v3: value is a compiled expression ID, not the raw expr name
        assert!(
            res.html.contains("data-webcore-attr-class=\""),
            "binding attribute missing"
        );
        assert!(
            !res.html.contains("class=\"{}\""),
            "broken placeholder still present"
        );
    }

    #[test]
    fn event_fallback_uses_on_event_attribute() {
        // Build minimal doc with a button using an unknown event
        let mut doc = WebCoreDocument {
            app: None,
            store: vec![],
            store_computed: vec![],
            locales: std::collections::BTreeMap::new(),
            default_locale: String::new(),
            wasm_module: None,
            view_transitions: false,
            layouts: std::collections::BTreeMap::new(),
            pages: std::collections::BTreeMap::new(),
            components: std::collections::BTreeMap::new(),
            imports: vec![],
            data_imports: std::collections::BTreeMap::new(),
            component_imports: vec![],
            page_imports: std::collections::BTreeMap::new(),
            source_files: std::collections::BTreeMap::new(),
        };
        doc.layouts.insert(
            "MainLayout".to_string(),
            Layout {
                name: "MainLayout".to_string(),
                content: vec![Element::Slot("content".to_string(), Span::default())],
                span: Span::default(),
            },
        );
        doc.pages.insert(
            "test".to_string(),
            Page {
                name: "test".to_string(),
                head: None,
                content: vec![Element::Tag {
                    name: "button".to_string(),
                    attributes: vec![Attribute {
                        name: "on:foo".to_string(),
                        value: AttributeValue::Expression("count += 1".to_string()),
                        span: Span::default(),
                    }],
                    content: vec![],
                    span: Span::default(),
                }],
                span: Span::default(),
            },
        );

        let opts = HtmlPageOptions {
            lang: "fr".to_string(),
            title: "t".to_string(),
            extra_css_files: vec![],
            critical_css: None,
            csp_meta: None,
            prod: false,
            source_maps: false,
            inline_runtime: true,
            site_url: None,
            canonical: None,
            pwa: None,
            hreflang_alternates: vec![],
        };
        let res = generate_html(&doc, "test", &opts).expect("html ok");
        assert!(res.html.contains("data-webcore-e=\"foo\""));
    }
}
