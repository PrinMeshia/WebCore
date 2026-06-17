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
pub(crate) mod js_events;
mod js_runtime;

use crate::codegen::html::HandlerMapping;
use crate::core::ast::WebCoreDocument;
use js_dom::{
    collect_component_event_listeners, collect_on_destroy_bodies, collect_on_mount_bodies,
    detect_features, rebind_seq,
};
use js_events::{compile_expression_full, replace_utils_short, CompiledVars};
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

#[cfg(test)]
pub(crate) fn generate_runtime_js(
    handlers: &[HandlerMapping],
    document: &WebCoreDocument,
) -> String {
    generate_runtime_js_prod(handlers, document, false)
}

#[must_use]
pub(crate) fn generate_runtime_js_prod(
    handlers: &[HandlerMapping],
    document: &WebCoreDocument,
    prod: bool,
) -> String {
    let state_vars = collect_state_variables(document);
    let store_vars = collect_store_variables(document);
    generate_runtime_js_with_vars(handlers, &state_vars, &store_vars, document, prod)
}

#[must_use]
pub(crate) fn generate_runtime_js_with_vars(
    handlers: &[HandlerMapping],
    state_vars: &HashSet<String>,
    store_vars: &HashSet<String>,
    document: &WebCoreDocument,
    prod: bool,
) -> String {
    // Pre-compile variable regexes once for this document — avoids recompiling N regexes
    // for every expression in handlers, computed vars, listeners, etc.
    let compiled_vars = CompiledVars::new(state_vars);

    let mut unique_handlers: std::collections::BTreeMap<&str, &HandlerMapping> =
        std::collections::BTreeMap::new();
    for handler in handlers {
        unique_handlers.insert(&handler.id, handler);
    }

    // ── Feature detection (tree-shaking) ────────────────────────────────────
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

    js.push_str("// WebCore Runtime (ES2022+)\n");
    js.push_str("{\n");

    // ── State class ─────────────────────────────────────────────────────────
    js.push_str(&emit_state_class(features.has_refs));
    js.push('\n');

    // ── Data imports — emit as initial state before other state vars ─────────
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
    // Store computed: reactive via $effect — runs immediately and re-runs when store deps change.
    // A try-catch guards against null initial values (e.g. list not yet loaded via http {}).
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

    // ── VARS / STORE_VARS (only when needed by reactive binding) ─────────────
    if needs_vars {
        js.push_str(&emit_vars_array(state_vars, store_vars));
    }

    js.push_str("const{max,min,abs}=Math,U={max,min,abs};\n\n");

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
        // t(key) — simple lookup
        // t(key, n: number) — plural: looks for key_one / key_other, replaces {{count}}
        // t(key, arg) — positional: replaces {{0}} in the translation string
        js.push_str("const t=(k,a)=>{if(a===undefined)return LOCALES[LOCALE]?.[k]??k;if(typeof a==='number'){const pk=a===1?k+'_one':k+'_other';return(LOCALES[LOCALE]?.[pk]??LOCALES[LOCALE]?.[k]??k).replace(/\\{\\{count\\}\\}/g,String(a));}return(LOCALES[LOCALE]?.[k]??k).replace(/\\{\\{0\\}\\}/g,String(a));};\n");
        write!(
            js,
            "const setLocale=l=>{{if(LOCALES[l]){{LOCALE=l;{all_rebinds}}}}};\n\n"
        )
        .expect("write! to String is infallible");
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
    let mut sorted_handlers: Vec<_> = unique_handlers.values().collect();
    sorted_handlers.sort_by(|a, b| a.id.cmp(&b.id));

    // Handler deduplication: identical compiled expressions share a single _wh<n> helper,
    // reducing bundle size when the same action is wired to multiple elements.
    {
        let non_debounce: Vec<_> = sorted_handlers
            .iter()
            .filter(|h| !h.event_type.contains("|debounce"))
            .collect();

        let mut expr_count: std::collections::BTreeMap<String, usize> =
            std::collections::BTreeMap::new();
        let compiled_exprs: Vec<String> = non_debounce
            .iter()
            .map(|h| compile_expression_full(&h.expression, &compiled_vars))
            .collect();
        for expr in &compiled_exprs {
            *expr_count.entry(expr.clone()).or_insert(0) += 1;
        }

        // Assign helper names to expressions used more than once.
        // BTreeMap iteration is sorted, so helper indices (_wh0, _wh1, …) are
        // assigned in lexicographic expression order — deterministic across runs.
        let mut expr_to_helper: std::collections::BTreeMap<String, String> =
            std::collections::BTreeMap::new();
        let mut helper_idx = 0usize;
        for (expr, count) in &expr_count {
            if *count > 1 {
                expr_to_helper.insert(expr.clone(), format!("_wh{helper_idx}"));
                helper_idx += 1;
            }
        }

        // Emit shared helper functions before H{}.
        // BTreeMap iterates by key (expression) in sorted order, and helper
        // names (_wh0, _wh1, …) were assigned in that same order, so no
        // additional sort step is needed here.
        for (expr, name) in &expr_to_helper {
            writeln!(js, "const {}=(event)=>{{{}}};", name, expr)
                .expect("write! to String is infallible");
        }

        js.push_str("const H={\n");
        for (handler, compiled) in non_debounce.iter().zip(compiled_exprs.iter()) {
            if let Some(helper) = expr_to_helper.get(compiled) {
                writeln!(js, "{}:{},", handler.id, helper).expect("write! to String is infallible");
            } else {
                writeln!(js, "{}(event){{{}}},", handler.id, compiled)
                    .expect("write! to String is infallible");
            }
        }
        js.push_str("};\n\n");
    }

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
            ).expect("write! to String is infallible");
        } else {
            // only computed, no interpolations
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
        writeln!(js, "const file={file_expr};").expect("write! to String is infallible");
        js.push_str("try{const html=await(await fetch('/'+file)).text();\n");
        js.push_str("const doc=new DOMParser().parseFromString(html,'text/html');\n");
        js.push_str("const main=doc.querySelector('main');\n");
        js.push_str("if(main)document.querySelector('main').replaceWith(main);\n");
        write!(js, "if(init)history.replaceState({{}},'',p);else history.pushState({{}},'',p);{all_rebinds};window.__wcAfterNav?.();}}catch(e){{location.href='/'+file}}}};\n\n").expect("write! to String is infallible");
        js.push_str("addEventListener('popstate',()=>nav(location.pathname));\n\n");
    }

    // ── Event delegation (CSP-safe — no inline onclick= attributes) ──────────
    // D(t, p) listens at document level and dispatches via H[el.id].
    // p=1 means preventDefault (click, submit); p=0 leaves default behaviour.
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
        // D(t, p): event delegation with modifier support.
        // Modifiers (stop, prevent, once, self) are encoded in data-webcore-e="type|mod1|mod2".
        // Uses startsWith check to match both plain ("click") and modified ("click|stop") elements.
        js.push_str("const D=(t,p)=>document.addEventListener(t,e=>{const el=e.target.closest('[data-webcore-e]');if(!el||!H[el.id])return;const dwe=el.dataset.webcoreE;if(dwe!==t&&!dwe.startsWith(t+'|'))return;const mods=dwe.includes('|')?dwe.split('|').slice(1):[];if(mods.includes('self')&&e.target!==el)return;if(mods.includes('stop'))e.stopPropagation();if(p||mods.includes('prevent'))e.preventDefault();if(mods.includes('once')){if(el.dataset.webcoreOnced)return;el.dataset.webcoreOnced='1';}H[el.id](e);});\n");
        for et in &non_debounce_event_types {
            let prevent = matches!(et.as_str(), "click" | "submit");
            writeln!(js, "D('{}',{});", et, if prevent { 1 } else { 0 })
                .expect("write! to String is infallible");
        }
        js.push('\n');
    }

    // ── SPA link delegation (data-webcore-nav) ────────────────────────────────
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
    // Swap deferred stylesheet (data-webcore-defer) to media="all" — CSP-safe alternative to onload=
    let css_defer_swap =
        ";document.querySelectorAll('link[data-webcore-defer]').forEach(l=>l.media='all')";
    write!(js, "document.addEventListener('DOMContentLoaded',()=>{{{init_route_params}{transition_css_inject}{all_rebinds}{refs_populate}{css_defer_swap}").expect("write! to String is infallible");
    for body in &mount_bodies {
        write!(js, ";(()=>{{\n{}\n}})()", body.trim()).expect("write! to String is infallible");
    }
    // ── $watch hooks ─────────────────────────────────────────────────────────
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
    // Prod-only: strip data-webcore-* attributes that are no longer needed after binding.
    // data-webcore-e, data-webcore-error, and data-v must be kept (event delegation,
    // form validation re-queries, and CSS scoping respectively).
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
