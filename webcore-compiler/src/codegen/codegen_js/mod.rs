//! JavaScript Code Generator for `WebCore` Runtime
//!
//! ## Runtime naming conventions
//!
//! The generated JavaScript uses short names intentionally — the runtime is
//! embedded verbatim in every built page and the names survive minification:
//!
//! | Name    | Meaning                              |
//! |---------|--------------------------------------|
//! | `S`     | Local component `State` instance     |
//! | `STORE` | Global shared `State` instance       |
//! | `U`     | Math utilities (`max`, `min`, `abs`) |
//! | `VARS`  | Array of local state variable names  |
//! | `STORE_VARS` | Array of global store var names |
//!
//! ## Block-scoping constraint
//!
//! The entire runtime is wrapped in a `{}` JS block so that `const` declarations
//! do not pollute `globalThis`.  As a consequence, `new Function(body)` cannot
//! see block-scoped bindings — `S`, `STORE`, `U` and `t` must therefore be
//! passed explicitly as named parameters whenever a dynamic `Function` is
//! constructed (see `evalCond`).
//!
//! ## Tree-shaking
//!
//! `generate_runtime_js_with_vars` calls `detect_features()` to inspect the
//! document AST and emits each runtime helper only when it is actually needed.
//! Simple pages (no `@if`, no `@for`, no validation) get a runtime of ~300 bytes.

mod js_dom;
mod js_events;
mod js_runtime;

use crate::ast::WebCoreDocument;
use crate::codegen::codegen_html::HandlerMapping;
use js_dom::{
    collect_component_event_listeners, collect_on_destroy_bodies, collect_on_mount_bodies,
    detect_features, rebind_seq,
};
use js_events::{compile_expression_full, replace_utils_short};
use js_runtime::{emit_bind_fns, emit_evalcond, emit_state_class, emit_vars_array};
use std::collections::HashSet;
use std::fmt::Write as _;

/// Collect all local state variable names from a document (component-level).
/// Includes computed var names so that `evalCond('doubled')` resolves to
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

/// Collect global store variable names
#[must_use]
pub(crate) fn collect_store_variables(document: &WebCoreDocument) -> HashSet<String> {
    document.store.iter().map(|v| v.name.clone()).collect()
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

#[must_use]
pub(crate) fn generate_runtime_js(
    handlers: &[HandlerMapping],
    document: &WebCoreDocument,
) -> String {
    let state_vars = collect_state_variables(document);
    let store_vars = collect_store_variables(document);
    generate_runtime_js_with_vars(handlers, &state_vars, &store_vars, document)
}

#[must_use]
pub(crate) fn generate_runtime_js_with_vars(
    handlers: &[HandlerMapping],
    state_vars: &HashSet<String>,
    store_vars: &HashSet<String>,
    document: &WebCoreDocument,
) -> String {
    let mut unique_handlers: std::collections::HashMap<String, &HandlerMapping> =
        std::collections::HashMap::new();
    for handler in handlers {
        unique_handlers.insert(handler.id.clone(), handler);
    }

    // ── Feature detection (tree-shaking) ────────────────────────────────────
    let features = detect_features(document);

    // Computed derived vars
    let mut computed_entries: Vec<String> = Vec::new();
    for component in document.components.values() {
        for cv in &component.computed {
            let compiled =
                replace_utils_short(&js_events::replace_store_and_local(&cv.expr, state_vars));
            computed_entries.push(format!("{{name:'{}',fn:()=>{}}}", cv.name, compiled));
        }
    }
    let has_computed = !computed_entries.is_empty();

    // Destroy hooks
    let destroy_bodies = collect_on_destroy_bodies(document);
    let has_destroy = !destroy_bodies.is_empty();

    // bind() is needed when there are interpolations or computed vars
    let needs_bind = features.has_interpolation || has_computed;
    // evalCond is needed when bind (with interpolations) or any conditional directive exists
    let needs_eval_cond = features.has_interpolation
        || features.has_if
        || features.has_for
        || features.has_dynamic_attrs
        || features.has_style_binding;
    // VARS/STORE_VARS needed whenever any reactive listener subscribes to them
    let needs_vars = needs_eval_cond || needs_bind;

    // Build the "rebind all" sequence used in nav(), setLocale(), WASM loader, DOMContentLoaded
    let all_rebinds = rebind_seq(&features, needs_bind);

    let mut js = String::new();

    js.push_str("// WebCore Runtime (ES2024+)\n");
    js.push_str("{\n");

    // ── State class ─────────────────────────────────────────────────────────
    js.push_str(&emit_state_class(features.has_refs));
    js.push('\n');

    // ── Data imports — emit as initial state before other state vars ─────────
    for (name, json) in &document.data_imports {
        writeln!(js, "S.setQ({:?},{});", name, json).unwrap();
    }

    // ── State initialisation ─────────────────────────────────────────────────
    for component in document.components.values() {
        for state_var in &component.state {
            let value = js_default_value(&state_var.type_, state_var.default_value.as_deref());
            writeln!(js, "S.set('{}',{});", state_var.name, value).unwrap();
        }
    }
    for store_var in &document.store {
        let value = js_default_value(&store_var.type_, store_var.default_value.as_deref());
        writeln!(js, "STORE.set('{}',{});", store_var.name, value).unwrap();
    }

    // ── VARS / STORE_VARS (only when needed by reactive binding) ─────────────
    if needs_vars {
        js.push_str(&emit_vars_array(state_vars, store_vars));
    }

    js.push_str("const{max,min,abs}=Math,U={max,min,abs};\n\n");

    // ── Computed derived state ────────────────────────────────────────────────
    if has_computed {
        writeln!(js, "const COMPUTED=[{}];", computed_entries.join(",")).unwrap();
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
        writeln!(js, "const LOCALES={{{}}};", locale_entries.join(",")).unwrap();
        writeln!(
            js,
            "let LOCALE=\"{}\";",
            escape_js_str(&document.default_locale)
        )
        .unwrap();
        // t(key) — simple lookup
        // t(key, n: number) — plural: looks for key_one / key_other, replaces {{count}}
        // t(key, arg) — positional: replaces {{0}} in the translation string
        js.push_str("const t=(k,a)=>{if(a===undefined)return LOCALES[LOCALE]?.[k]??k;if(typeof a==='number'){const pk=a===1?k+'_one':k+'_other';return(LOCALES[LOCALE]?.[pk]??LOCALES[LOCALE]?.[k]??k).replace(/\\{\\{count\\}\\}/g,String(a));}return(LOCALES[LOCALE]?.[k]??k).replace(/\\{\\{0\\}\\}/g,String(a));};\n");
        write!(
            js,
            "const setLocale=l=>{{if(LOCALES[l]){{LOCALE=l;{all_rebinds}}}}};\n\n"
        )
        .unwrap();
    }

    // ── QUERY_PARAMS Proxy (tree-shaken: only when $query. used) ─────────────
    if features.has_query_params {
        js.push_str("const QUERY_PARAMS=new Proxy({},{get:(_,k)=>new URLSearchParams(location.search).get(String(k))??\"\"});\n");
    }

    // ── evalCond (tree-shaken away when unused) ───────────────────────────────
    if needs_eval_cond {
        js.push_str(&emit_evalcond(&features, !document.locales.is_empty()));
    }

    // ── Reactive binding functions (tree-shaken) ──────────────────────────────
    js.push_str(&emit_bind_fns(&features));
    js.push('\n');

    // ── Handlers ─────────────────────────────────────────────────────────────
    // Debounce handlers are wired up in DOMContentLoaded; regular handlers go in H.
    js.push_str("const H={\n");
    let mut sorted_handlers: Vec<_> = unique_handlers.values().collect();
    sorted_handlers.sort_by(|a, b| a.id.cmp(&b.id));
    for handler in &sorted_handlers {
        // Skip debounce handlers — they get wired up in DOMContentLoaded instead
        if handler.event_type.contains("|debounce") {
            continue;
        }
        let compiled = compile_expression_full(&handler.expression, state_vars);
        writeln!(js, "{}(){{{}}},", handler.id, compiled).unwrap();
    }
    js.push_str("};\n\n");

    // ── bind() — re-evaluate computed, then wire interpolation spans ──────────
    if needs_bind {
        if features.has_interpolation {
            js.push_str("const bind=()=>{");
            if has_computed {
                js.push_str("rebindComputed();");
            }
            // u re-runs rebindComputed so computed vars (e.g. doubled) are fresh
            // before the span's textContent is updated. setQ is side-effect-free
            // (no listener cascade), so this cannot loop.
            let recompute_in_u = if has_computed {
                "rebindComputed();"
            } else {
                ""
            };
            write!(js,
                "document.querySelectorAll('[data-webcore-interpolation]').forEach(el=>{{const e=el.dataset.webcoreInterpolation,u=()=>{{{}el.textContent=String(evalCond(e)??'')}};$effect(u)}})}};\n\n",
                recompute_in_u
            ).unwrap();
        } else {
            // only computed, no interpolations
            js.push_str("const bind=()=>rebindComputed();\n\n");
        }
    }

    // ── on:destroy hooks ─────────────────────────────────────────────────────
    if has_destroy {
        let bodies: Vec<String> = destroy_bodies
            .iter()
            .map(|b| format!("()=>{{{}}}", b.trim()))
            .collect();
        writeln!(js, "const DESTROY_HOOKS=[{}];", bodies.join(",")).unwrap();
        js.push_str("const runDestroyHooks=()=>DESTROY_HOOKS.forEach(f=>f());\n\n");
    }

    // ── SPA navigation (tree-shaken when unused) ──────────────────────────────
    if features.has_navigation {
        if features.has_param_routes {
            // Build ROUTES array with regex patterns for parameterized routes
            let routes = document
                .app
                .as_ref()
                .map(|a| &a.routes)
                .cloned()
                .unwrap_or_default();
            let mut route_entries: Vec<(String, String)> = routes.into_iter().collect();
            route_entries.sort_by_key(|(path, _)| path.clone());

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
                .unwrap();
            }
            routes_js.push_str("];\n");
            js.push_str(&routes_js);
            js.push_str("let ROUTE_PARAMS={};\n");
            js.push_str("const matchRoute=p=>{for(const r of ROUTES){const m=p.match(r.re);if(m){r.params.forEach((k,i)=>ROUTE_PARAMS[k]=m[i+1]);return r.file;}}return p==='/'?'index.html':`${p.slice(1)}/index.html`;};\n");
        } else {
            js.push_str("const toFile=p=>p==='/'?'index.html':`${p.slice(1)}/index.html`;\n");
        }
        // init=true → replaceState (no duplicate history entry on first load)
        js.push_str("const nav=async(p,init=false)=>{\n");
        if has_destroy {
            js.push_str("runDestroyHooks();\n");
        }
        let file_expr = if features.has_param_routes {
            "matchRoute(p)"
        } else {
            "toFile(p)"
        };
        writeln!(js, "const file={file_expr};").unwrap();
        js.push_str("try{const html=await(await fetch('/'+file)).text();\n");
        js.push_str("const doc=new DOMParser().parseFromString(html,'text/html');\n");
        js.push_str("const main=doc.querySelector('main');\n");
        js.push_str("if(main)document.querySelector('main').replaceWith(main);\n");
        write!(js, "if(init)history.replaceState({{}},'',p);else history.pushState({{}},'',p);{all_rebinds}}}catch(e){{location.href='/'+file}}}};\n\n").unwrap();
        js.push_str("addEventListener('popstate',()=>nav(location.pathname));\n\n");
    }

    // ── globalThis exports ────────────────────────────────────────────────────
    let i18n_export = if document.locales.is_empty() {
        ""
    } else {
        ",setLocale"
    };
    let nav_export = if features.has_navigation {
        "webcore_navigate:nav,"
    } else {
        ""
    };
    writeln!(js, "Object.assign(globalThis,{{{nav_export}webcore_handle_event:(t,id)=>H[id]?.(){i18n_export},").unwrap();
    js.push_str(
        "...Object.fromEntries(['click','submit','change','input'].map(e=>[`webcore_handle_${e}`,id=>H[id]?.()]))});\n\n",
    );

    // ── DOMContentLoaded ─────────────────────────────────────────────────────
    let mount_bodies = collect_on_mount_bodies(document);
    let comp_listeners = collect_component_event_listeners(document);

    // For param routes: populate ROUTE_PARAMS then, if the current URL is a
    // parameterised path (slug, id, …), asynchronously load the correct page.
    // The dev server may have served index.html as SPA fallback for /post/hello,
    // so we need to fetch post.html and swap <main> with the right content.
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
    write!(js, "document.addEventListener('DOMContentLoaded',()=>{{{init_route_params}{transition_css_inject}{all_rebinds}{refs_populate}").unwrap();
    for body in &mount_bodies {
        write!(js, ";(()=>{{{}}})()", body.trim()).unwrap();
    }
    // ── $watch hooks ─────────────────────────────────────────────────────────
    for comp in document.components.values() {
        for hook in &comp.watch_hooks {
            write!(js, ";S.on('{}',{}=>{{{}}})", hook.var, hook.var, hook.body).unwrap();
        }
    }
    for listener in &comp_listeners {
        let compiled = compile_expression_full(&listener.expression, state_vars);
        write!(
            js,
            ";document.addEventListener('{}',e=>{{{}}})",
            listener.event_name, compiled
        )
        .unwrap();
    }
    // ── Debounce event listeners ──────────────────────────────────────────────
    // Wire up debounce handlers: find the element by id and attach a debounced listener.
    {
        let mut dbt_idx = 0usize;
        let debounce_handlers: Vec<_> = sorted_handlers
            .iter()
            .filter(|h| h.event_type.contains("|debounce"))
            .collect();
        for handler in debounce_handlers {
            // event_type is like "input|debounce=300"
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
            let compiled = compile_expression_full(&handler.expression, state_vars);
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
            ).unwrap();
        }
    }
    // ── HTTP fetch blocks ─────────────────────────────────────────────────────
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
                    ";(async()=>{{try{{const __r=await fetch(\"{}\");if(!__r.ok)throw new Error(__r.statusText);const __d=await __r.json();S.set('{}',__d);S.set('loading',false);{}}}catch(__e){{S.set('error',__e.message);S.set('loading',false);{}}}}})()",
                    escape_js_str(url),
                    escape_js_str(into_var),
                    rb,
                    rb,
                ).unwrap();
            }
        }
    }
    if has_destroy {
        js.push_str(";window.addEventListener('beforeunload',runDestroyHooks)");
    }
    js.push_str("});\n");

    // ── WASM async loader (only when module detected) ─────────────────────────
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

    js
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
