//! JavaScript Code Generator for `WebCore` Runtime
//!
//! ## Runtime naming conventions
//!
//! The generated JavaScript uses short names intentionally — the runtime ships
//! as a single shared `/assets/webcore.js` and the names survive minification:
//!
//! | Name    | Meaning                              |
//! |---------|--------------------------------------|
//! | `S`     | Local component `State` instance     |
//! | `STORE` | Global shared `State` instance       |
//! | `U`     | Math utilities (`max`, `min`, `abs`) |
//! | `_e`    | Map of compiled read-expression closures (`e0`, …) |
//!
//! ## Block-scoping constraint
//!
//! The entire runtime is wrapped in a `{}` JS block so that `const` declarations
//! do not pollute `globalThis`.  Read expressions are compiled to real JS
//! closures at build time (`const _e={e0:()=>S.get('x'),…}`) — there is no
//! runtime `new Function()` / `evalCond`, which keeps the output CSP-safe.
//!
//! ## Tree-shaking
//!
//! `generate_runtime_js_with_vars_and_exprs` calls `detect_features()` to inspect the
//! document AST and emits each runtime helper only when it is actually needed.
//! Simple pages (no `@if`, no `@for`, no validation) get a runtime of ~300 bytes.

mod js_dom;
pub(crate) mod js_events;
mod js_runtime;
pub(crate) mod sourcemap;

pub(crate) use sourcemap::{Mapping as SourceMapMapping, SourceMapBuilder};

use crate::codegen::html::HandlerMapping;
use crate::core::ast::{Span, WebCoreDocument};
use js_dom::{
    collect_component_event_listeners, collect_on_destroy_bodies, collect_on_mount_bodies,
    detect_features, rebind_seq_v3,
};
use js_events::{compile_expression_full, replace_utils_short, CompiledVars};
use js_runtime::{emit_bind_fns_v3, emit_state_class};
use std::collections::HashSet;
use std::fmt::Write as _;

/// Collect all local state variable names from a document (component-level).
/// Includes computed var names so that an expression like `doubled` compiles to
/// `S.get('doubled')` rather than a bare identifier that throws `ReferenceError`.
#[must_use]
pub(crate) fn collect_state_variables(document: &WebCoreDocument) -> HashSet<String> {
    let mut vars = HashSet::new();
    for component in document.components.values() {
        for state_var in &component.state {
            vars.insert(state_var.name.clone());
        }
        for computed_var in &component.computed {
            vars.insert(computed_var.name.clone());
        }
    }
    vars
}

/// Collect global store variable names (state vars + computed vars).
#[must_use]
pub(crate) fn collect_store_variables(document: &WebCoreDocument) -> HashSet<String> {
    document
        .store
        .iter()
        .map(|v| v.name.clone())
        .chain(document.store_computed.iter().map(|c| c.name.clone()))
        .collect()
}

/// Return the JS literal that should initialise a state variable.
///
/// String-typed vars whose default is not already quoted get wrapped in `"…"`.
/// Everything else (numbers, booleans, `null`, arrays, quoted strings) is passed through.
fn js_default_value(type_: &str, default_value: Option<&str>) -> String {
    let raw = default_value.unwrap_or("null");
    if type_ == "String" && !raw.starts_with('"') && raw != "null" {
        format!("\"{raw}\"")
    } else {
        raw.to_string()
    }
}

/// Convert a component/page name to the expected HTML filename (mirrors main.rs logic).
fn page_name_to_file(name: &str) -> String {
    js_events::page_name_to_file(name)
}

/// Convert a route pattern `/post/:slug` to a JS regex string and return param names.
fn route_to_js_regex(pattern: &str) -> (String, Vec<String>) {
    js_events::route_to_js_regex(pattern)
}

/// Escape a string for safe embedding in a JS double-quoted string literal.
fn escape_js_str(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

/// Test helper: generate the standalone runtime JS for a document (no compiled
/// expression map). Delegates to the shipped v3 emitter so the golden tests
/// exercise the real runtime.
#[cfg(test)]
pub(crate) fn generate_runtime_js(
    handlers: &[HandlerMapping],
    document: &WebCoreDocument,
) -> String {
    generate_inline_js(handlers, &[], &[], document, false).js
}

/// Output of `generate_inline_js`: the JS string plus optional source map data.
pub(crate) struct InlineJs {
    pub js: String,
    /// `(output_line, webc_span)` pairs — one per compiled expression closure.
    /// Populated only when `expr_spans` is non-empty.
    pub expr_mappings: Vec<(u32, Span)>,
}

/// Generate the full v3 runtime JS for a document.
///
/// Used both for the shared `/assets/webcore.js` (fed the union of every page's
/// expressions + handlers) and, in tests, for standalone runtime generation.
/// Includes the compiled expression map `_e` so bind functions call `_e[id]()`.
/// When `expr_spans` is non-empty, the `_e` map is emitted one-closure-per-line
/// and line mappings are returned in `InlineJs::expr_mappings`.
#[must_use]
pub(crate) fn generate_inline_js(
    handlers: &[HandlerMapping],
    compiled_exprs: &[(String, String)],
    expr_spans: &[Span],
    document: &WebCoreDocument,
    prod: bool,
) -> InlineJs {
    let state_vars = collect_state_variables(document);
    let store_vars = collect_store_variables(document);
    let (js, expr_mappings) = generate_runtime_js_with_vars_and_exprs(
        handlers,
        compiled_exprs,
        expr_spans,
        &state_vars,
        &store_vars,
        document,
        prod,
    );
    let js = if prod { rename_runtime_ids(&js) } else { js };
    InlineJs { js, expr_mappings }
}

/// Rename runtime function identifiers to short names in prod builds.
///
/// Applied after generation so the rest of the pipeline uses readable names.
/// Sorted longest-first to prevent substring collisions (e.g. `bindFor` before `bind`).
fn rename_runtime_ids(js: &str) -> String {
    const RENAMES: &[(&str, &str)] = &[
        ("bindClassBindings", "_bc"),
        ("rebindComputed", "_rc"),
        ("runDestroyHooks", "_rd"),
        ("bindValidation", "_bv"),
        ("validateField", "_vf"),
        ("matchRoute", "_mr"),
        ("bindAttrs", "_ba"),
        ("bindDefer", "_bd"),
        ("bindFor", "_bf"),
        ("bindIf", "_bi"),
        ("bind(", "_b("),             // bind() calls — after all bind* are renamed
        ("const bind=", "const _b="), // bind declaration
        ("__wcfx", "_fx"),
        ("$effect", "_ef"),
    ];
    let mut result = js.to_string();
    for (from, to) in RENAMES {
        result = result.replace(from, to);
    }
    result
}

/// v3 runtime generator: emits `_e` expression map and uses v3 bind functions.
/// Returns `(js_string, expr_mappings)` where `expr_mappings` is a vec of
/// `(output_line, span)` — one per expression closure, for source map generation.
#[allow(clippy::too_many_lines)]
#[must_use]
fn generate_runtime_js_with_vars_and_exprs(
    handlers: &[HandlerMapping],
    compiled_exprs: &[(String, String)],
    expr_spans: &[Span],
    state_vars: &HashSet<String>,
    _store_vars: &HashSet<String>,
    document: &WebCoreDocument,
    prod: bool,
) -> (String, Vec<(u32, Span)>) {
    let compiled_vars = CompiledVars::new(state_vars);

    let mut unique_handlers: std::collections::BTreeMap<&str, &HandlerMapping> =
        std::collections::BTreeMap::new();
    for handler in handlers {
        unique_handlers.insert(&handler.id, handler);
    }

    let features = detect_features(document);

    // Computed derived vars
    let mut computed_entries: Vec<String> = Vec::new();
    for component in document.components.values() {
        for cv in &component.computed {
            let compiled = replace_utils_short(&js_events::replace_store_and_local(
                &cv.expr,
                &compiled_vars,
            ));
            computed_entries.push(format!("{{name:'{}',fn:()=>{}}}", cv.name, compiled));
        }
    }
    let has_computed = !computed_entries.is_empty();

    let destroy_bodies = collect_on_destroy_bodies(document);
    let has_destroy = !destroy_bodies.is_empty();

    let needs_bind = features.has_interpolation || has_computed;

    // Build rebind sequence using v3 calls (bind(), bindIf(), etc.)
    let all_rebinds = rebind_seq_v3(&features, needs_bind);

    let mut js = String::new();

    js.push_str("// WebCore Runtime v3 (ES2022+)\n");
    js.push_str("{\n");

    // ── State class ─────────────────────────────────────────────────────────
    js.push_str(&emit_state_class(features.has_refs));
    js.push('\n');

    // ── Data imports ─────────────────────────────────────────────────────────
    for (name, json) in &document.data_imports {
        writeln!(js, "S.setQ({:?},{});", name, json).expect("write! to String is infallible");
    }

    // ── State initialisation ─────────────────────────────────────────────────
    for component in document.components.values() {
        for state_var in &component.state {
            let value = js_default_value(&state_var.type_, state_var.default_value.as_deref());
            writeln!(js, "S.set('{}',{});", state_var.name, value)
                .expect("write! to String is infallible");
        }
    }
    for store_var in &document.store {
        let value = js_default_value(&store_var.type_, store_var.default_value.as_deref());
        writeln!(js, "STORE.set('{}',{});", store_var.name, value)
            .expect("write! to String is infallible");
    }
    if !document.store_computed.is_empty() {
        let sc_store_vars: HashSet<String> =
            document.store.iter().map(|v| v.name.clone()).collect();
        for sc in &document.store_computed {
            let expr = js_events::compile_store_computed_expr(&sc.expr, &sc_store_vars);
            writeln!(
                js,
                "$effect(()=>{{try{{STORE.setQ('{}',{});}}catch(_){{}}}});",
                sc.name, expr
            )
            .expect("write! to String is infallible");
        }
    }

    js.push_str("const{max,min,abs}=Math,U={max,min,abs};\n\n");

    // ── Compiled expression map (_e) ─────────────────────────────────────────
    let mut expr_mappings: Vec<(u32, Span)> = Vec::new();
    if !compiled_exprs.is_empty() {
        if !expr_spans.is_empty() {
            // Multi-line emission: one closure per line for source map tracking
            let base_line = js.chars().filter(|&c| c == '\n').count() as u32;
            js.push_str("const _e={\n");
            for (i, (id, closure)) in compiled_exprs.iter().enumerate() {
                let output_line = base_line + 1 + i as u32;
                let span = expr_spans.get(i).copied().unwrap_or_default();
                expr_mappings.push((output_line, span));
                writeln!(js, "{id}:{closure},").expect("write! to String is infallible");
            }
            js.push_str("};\n");
        } else {
            // Single-line emission (no source maps)
            let entries = compiled_exprs
                .iter()
                .map(|(id, closure)| format!("{id}:{closure}"))
                .collect::<Vec<_>>()
                .join(",");
            writeln!(js, "const _e={{{entries}}};").expect("write! to String is infallible");
        }
    } else {
        js.push_str("const _e={};\n");
    }

    // ── Computed derived state ────────────────────────────────────────────────
    if has_computed {
        writeln!(js, "const COMPUTED=[{}];", computed_entries.join(","))
            .expect("write! to String is infallible");
        js.push_str("const rebindComputed=()=>COMPUTED.forEach(c=>S.setQ(c.name,c.fn()));\n\n");
    }

    // ── i18n runtime ─────────────────────────────────────────────────────────
    if !document.locales.is_empty() {
        let mut locale_entries: Vec<String> = document
            .locales
            .iter()
            .map(|(code, messages)| {
                let mut msg_entries: Vec<String> = messages
                    .iter()
                    .map(|(k, v)| format!("\"{}\":\"{}\"", escape_js_str(k), escape_js_str(v)))
                    .collect();
                msg_entries.sort();
                format!("\"{}\":{{{}}}", escape_js_str(code), msg_entries.join(","))
            })
            .collect();
        locale_entries.sort();
        writeln!(js, "const LOCALES={{{}}};", locale_entries.join(","))
            .expect("write! to String is infallible");
        writeln!(
            js,
            "let LOCALE=\"{}\";",
            escape_js_str(&document.default_locale)
        )
        .expect("write! to String is infallible");
        js.push_str("const t=(k,a)=>{if(a===undefined)return LOCALES[LOCALE]?.[k]??k;if(typeof a==='number'){const pk=a===1?k+'_one':k+'_other';return(LOCALES[LOCALE]?.[pk]??LOCALES[LOCALE]?.[k]??k).replace(/\\{\\{count\\}\\}/g,String(a));}return(LOCALES[LOCALE]?.[k]??k).replace(/\\{\\{0\\}\\}/g,String(a));};\n");
        write!(
            js,
            "const setLocale=l=>{{if(LOCALES[l]){{LOCALE=l;{all_rebinds}}}}};\n\n"
        )
        .expect("write! to String is infallible");
    }

    // ── QUERY_PARAMS Proxy ────────────────────────────────────────────────────
    if features.has_query_params {
        js.push_str("const QUERY_PARAMS=new Proxy({},{get:(_,k)=>new URLSearchParams(location.search).get(String(k))??\"\"});\n");
    }

    // ── v3 Reactive binding functions ─────────────────────────────────────────
    js.push_str(&emit_bind_fns_v3(&features));
    js.push('\n');

    // ── Handlers ─────────────────────────────────────────────────────────────
    let mut sorted_handlers: Vec<_> = unique_handlers.values().collect();
    sorted_handlers.sort_by(|a, b| a.id.cmp(&b.id));

    {
        let non_debounce: Vec<_> = sorted_handlers
            .iter()
            .filter(|h| !h.event_type.contains("|debounce"))
            .collect();

        let mut expr_count: std::collections::BTreeMap<String, usize> =
            std::collections::BTreeMap::new();
        let handler_compiled_exprs: Vec<String> = non_debounce
            .iter()
            .map(|h| compile_expression_full(&h.expression, &compiled_vars))
            .collect();
        for expr in &handler_compiled_exprs {
            *expr_count.entry(expr.clone()).or_insert(0) += 1;
        }

        let mut expr_to_helper: std::collections::BTreeMap<String, String> =
            std::collections::BTreeMap::new();
        let mut helper_idx = 0usize;
        for (expr, count) in &expr_count {
            if *count > 1 {
                expr_to_helper.insert(expr.clone(), format!("_wh{helper_idx}"));
                helper_idx += 1;
            }
        }
        for (expr, name) in &expr_to_helper {
            writeln!(js, "const {}=(event)=>{{{}}};", name, expr)
                .expect("write! to String is infallible");
        }

        js.push_str("const H={\n");
        for (handler, compiled) in non_debounce.iter().zip(handler_compiled_exprs.iter()) {
            if let Some(helper) = expr_to_helper.get(compiled) {
                writeln!(js, "{}:{},", handler.id, helper).expect("write! to String is infallible");
            } else {
                writeln!(js, "{}(event){{{}}},", handler.id, compiled)
                    .expect("write! to String is infallible");
            }
        }
        js.push_str("};\n\n");
    }

    // ── bind() — v3: uses _e[id]() for interpolation spans ───────────────────
    if needs_bind {
        if features.has_interpolation {
            js.push_str("const bind=()=>{");
            if has_computed {
                js.push_str("rebindComputed();");
            }
            let recompute_in_u = if has_computed {
                "rebindComputed();"
            } else {
                ""
            };
            write!(js,
                "document.querySelectorAll('[data-webcore-interpolation]').forEach(el=>{{const id=el.dataset.webcoreInterpolation,fn=_e[id],u=()=>{{{}el.textContent=String(fn?.()??'')}};$effect(u)}})}};\n\n",
                recompute_in_u
            ).expect("write! to String is infallible");
        } else {
            js.push_str("const bind=()=>rebindComputed();\n\n");
        }
    }

    // ── on:destroy hooks ─────────────────────────────────────────────────────
    if has_destroy {
        let bodies: Vec<String> = destroy_bodies
            .iter()
            .map(|b| format!("()=>{{\n{}\n}}", b.trim()))
            .collect();
        writeln!(js, "const DESTROY_HOOKS=[{}];", bodies.join(","))
            .expect("write! to String is infallible");
        js.push_str("const runDestroyHooks=()=>DESTROY_HOOKS.forEach(f=>f());\n\n");
    }

    // ── SPA navigation ───────────────────────────────────────────────────────
    if features.has_navigation {
        if features.has_param_routes {
            let routes = document
                .app
                .as_ref()
                .map(|a| &a.routes)
                .cloned()
                .unwrap_or_default();
            let mut route_entries: Vec<(String, String)> = routes.into_iter().collect();
            route_entries.sort_by(|(a, _), (b, _)| a.cmp(b));

            let mut routes_js = String::from("const ROUTES=[");
            for (path, page_name) in &route_entries {
                let (regex, params) = route_to_js_regex(path);
                let file = page_name_to_file(page_name);
                let params_js: Vec<String> = params.iter().map(|p| format!("\"{p}\"")).collect();
                write!(
                    routes_js,
                    "{{re:/{}/,file:\"{}\",params:[{}]}},",
                    regex,
                    escape_js_str(&file),
                    params_js.join(",")
                )
                .expect("write! to String is infallible");
            }
            routes_js.push_str("];\n");
            js.push_str(&routes_js);
            js.push_str("let ROUTE_PARAMS={};\n");
            js.push_str("const matchRoute=p=>{for(const r of ROUTES){const m=p.match(r.re);if(m){r.params.forEach((k,i)=>ROUTE_PARAMS[k]=m[i+1]);return r.file;}}return p==='/'?'index.html':`${p.slice(1)}/index.html`;};\n");
        } else {
            js.push_str("const toFile=p=>p==='/'?'index.html':`${p.slice(1)}/index.html`;\n");
        }
        js.push_str("const nav=async(p,init=false)=>{\n");
        if has_destroy {
            js.push_str("runDestroyHooks();\n");
        }
        let file_expr = if features.has_param_routes {
            "matchRoute(p)"
        } else {
            "toFile(p)"
        };
        writeln!(js, "const file={file_expr};").expect("write! to String is infallible");
        js.push_str("try{const html=await(await fetch('/'+file)).text();\n");
        js.push_str("const doc=new DOMParser().parseFromString(html,'text/html');\n");
        js.push_str("const main=doc.querySelector('main');\n");
        js.push_str("if(main)document.querySelector('main').replaceWith(main);\n");
        write!(js, "if(init)history.replaceState({{}},'',p);else history.pushState({{}},'',p);{all_rebinds};window.__wcAfterNav?.();}}catch(e){{location.href='/'+file}}}};\n\n").expect("write! to String is infallible");
        js.push_str("addEventListener('popstate',()=>nav(location.pathname));\n\n");
    }

    // ── Event delegation ─────────────────────────────────────────────────────
    let non_debounce_event_types: Vec<String> = {
        let mut seen = std::collections::BTreeSet::new();
        for h in &sorted_handlers {
            if !h.event_type.contains("|debounce") {
                seen.insert(h.event_type.clone());
            }
        }
        seen.into_iter().collect()
    };
    if !non_debounce_event_types.is_empty() {
        js.push_str("const D=(t,p)=>document.addEventListener(t,e=>{const el=e.target.closest('[data-webcore-e]');if(!el||!H[el.id])return;const dwe=el.dataset.webcoreE;if(dwe!==t&&!dwe.startsWith(t+'|'))return;const mods=dwe.includes('|')?dwe.split('|').slice(1):[];if(mods.includes('self')&&e.target!==el)return;if(mods.includes('stop'))e.stopPropagation();if(p||mods.includes('prevent'))e.preventDefault();if(mods.includes('once')){if(el.dataset.webcoreOnced)return;el.dataset.webcoreOnced='1';}H[el.id](e);});\n");
        for et in &non_debounce_event_types {
            let prevent = matches!(et.as_str(), "click" | "submit");
            writeln!(js, "D('{}',{});", et, if prevent { 1 } else { 0 })
                .expect("write! to String is infallible");
        }
        js.push('\n');
    }

    if features.has_navigation {
        js.push_str("document.addEventListener('click',e=>{const a=e.target.closest('a[data-webcore-nav]');if(a){e.preventDefault();nav(a.getAttribute('href'));}});\n\n");
    }

    // ── globalThis exports ────────────────────────────────────────────────────
    {
        let mut global_exports: Vec<&str> = Vec::new();
        if features.has_navigation {
            global_exports.push("webcore_navigate:nav");
        }
        if !document.locales.is_empty() {
            global_exports.push("setLocale");
        }
        if !global_exports.is_empty() {
            writeln!(
                js,
                "Object.assign(globalThis,{{{}}});",
                global_exports.join(",")
            )
            .expect("write! to String is infallible");
            js.push('\n');
        }
    }

    // ── DOMContentLoaded ─────────────────────────────────────────────────────
    let mount_bodies = collect_on_mount_bodies(document);
    let comp_listeners = collect_component_event_listeners(document);

    let init_route_params = if features.has_param_routes {
        "matchRoute(location.pathname);if(Object.keys(ROUTE_PARAMS).length)nav(location.pathname,true);"
    } else {
        ""
    };
    let transition_css_inject = if features.has_transition {
        ";document.head.insertAdjacentHTML('beforeend','<style>.webc-fade-enter{opacity:0;transition:opacity 250ms ease}.webc-fade-enter-to{opacity:1}.webc-fade-leave{opacity:1;transition:opacity 250ms ease}.webc-fade-leave-to{opacity:0}.webc-slide-enter{transform:translateY(-6px);opacity:0;transition:transform 200ms ease,opacity 200ms ease}.webc-slide-enter-to{transform:none;opacity:1}.webc-slide-leave{transition:transform 200ms ease,opacity 200ms ease}.webc-slide-leave-to{transform:translateY(-6px);opacity:0}</style>')"
    } else {
        ""
    };
    let refs_populate = if features.has_refs {
        ";document.querySelectorAll('[data-webcore-ref]').forEach(el=>{refs[el.dataset.webcoreRef]=el;})"
    } else {
        ""
    };
    let css_defer_swap =
        ";document.querySelectorAll('link[data-webcore-defer]').forEach(l=>l.media='all')";
    write!(js, "document.addEventListener('DOMContentLoaded',()=>{{{init_route_params}{transition_css_inject}{all_rebinds}{refs_populate}{css_defer_swap}").expect("write! to String is infallible");
    for body in &mount_bodies {
        write!(js, ";(()=>{{\n{}\n}})()", body.trim()).expect("write! to String is infallible");
    }
    for comp in document.components.values() {
        for hook in &comp.watch_hooks {
            write!(js, ";S.on('{}',{}=>{{{}}})", hook.var, hook.var, hook.body)
                .expect("write! to String is infallible");
        }
    }
    for listener in &comp_listeners {
        let compiled = compile_expression_full(&listener.expression, &compiled_vars);
        write!(
            js,
            ";document.addEventListener('{}',e=>{{{}}})",
            listener.event_name, compiled
        )
        .expect("write! to String is infallible");
    }
    // Debounce handlers
    {
        let mut dbt_idx = 0usize;
        let debounce_handlers: Vec<_> = sorted_handlers
            .iter()
            .filter(|h| h.event_type.contains("|debounce"))
            .collect();
        for handler in debounce_handlers {
            let (event_name, delay_ms) =
                if let Some(pipe_pos) = handler.event_type.find("|debounce") {
                    let base = &handler.event_type[..pipe_pos];
                    let after = &handler.event_type[pipe_pos + "|debounce".len()..];
                    let ms: u32 = if let Some(stripped) = after.strip_prefix('=') {
                        stripped.parse().unwrap_or(300)
                    } else {
                        300
                    };
                    (base, ms)
                } else {
                    (handler.event_type.as_str(), 300u32)
                };
            let compiled = compile_expression_full(&handler.expression, &compiled_vars);
            dbt_idx += 1;
            write!(js,
                ";(()=>{{const __el=document.getElementById('{}');if(__el){{let __dbt{};__el.addEventListener('{}',e=>{{clearTimeout(__dbt{});__dbt{}=setTimeout(()=>{{{};}}{},{})}})}}}})()",
                handler.id,
                dbt_idx,
                event_name,
                dbt_idx,
                dbt_idx,
                compiled,
                if all_rebinds.is_empty() { String::new() } else { format!(";{all_rebinds}") },
                delay_ms
            ).expect("write! to String is infallible");
        }
    }
    // HTTP fetch blocks
    if features.has_http {
        for component in document.components.values() {
            if let Some(http) = &component.http {
                let into_var = &http.into;
                let url = &http.url;
                let rb = if all_rebinds.is_empty() {
                    String::new()
                } else {
                    format!("{all_rebinds};")
                };
                write!(js,
                    ";Promise.resolve().then(async()=>{{const __r=await fetch(\"{}\");if(!__r.ok)throw new Error(__r.statusText);const __d=await __r.json();S.set('{}',__d);S.set('loading',false);}}).catch(__e=>{{S.set('error',__e instanceof Error?__e.message:String(__e));S.set('loading',false);}}).finally(()=>{{{}}})",
                    escape_js_str(url),
                    escape_js_str(into_var),
                    rb,
                ).expect("write! to String is infallible");
            }
        }
    }
    if has_destroy {
        js.push_str(";window.addEventListener('beforeunload',runDestroyHooks)");
    }
    if prod {
        js.push_str(
            ";(['data-webcore-if','data-webcore-else','data-webcore-interpolation',\
'data-webcore-ref','data-webcore-defer','data-webcore-spread']).forEach(a=>{\
document.querySelectorAll('['+a+']').forEach(el=>el.removeAttribute(a))});\
document.querySelectorAll('[data-webcore-bound]').forEach(el=>{\
const ns=[...el.attributes].filter(a=>a.name==='data-webcore-bound'||a.name.startsWith('data-webcore-attr-')||a.name.startsWith('data-webcore-style-')).map(a=>a.name);\
ns.forEach(n=>el.removeAttribute(n))});\
document.querySelectorAll('[data-webcore-class-bound]').forEach(el=>{\
const ns=[...el.attributes].filter(a=>a.name.startsWith('data-webcore-class-')&&a.name!=='data-webcore-class-bound').map(a=>a.name);\
ns.forEach(n=>el.removeAttribute(n))})"
        );
    }
    js.push_str("});\n");

    if let Some(module) = &document.wasm_module {
        let wasm_loader = format!(
            "const WASM={{}};globalThis.wasm=WASM;\
(async()=>{{try{{const m=await import('./wasm/{m}.js');\
await m.default();Object.assign(WASM,m);\
{rb};\
}}catch(e){{console.warn('[WebCore WASM]',e);}}}})();\n",
            m = escape_js_str(module),
            rb = all_rebinds,
        );
        js.push_str(&wasm_loader);
    }

    js.push_str("}\n");
    (js, expr_mappings)
}

/// Strip line comments and collapse whitespace — safe for generated JS (no multiline strings).
pub(crate) fn minify_js(js: &str) -> String {
    js.lines()
        .filter(|l| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with("//")
        })
        .map(str::trim)
        .collect()
}
