//! Tests that verify JavaScript output.

#[cfg(test)]
use super::*;

#[test]
fn golden_state_initialised_in_js() {
    let js = compile_to_js(r#"
layout MainLayout { main { slot content } }
component Counter {
    state { count: Number = 0 }
    view { p "{count}" }
}
page "home" { Counter {} }
"#);
    assert!(js.contains("S.set('count',0)"), "state init missing:\n{}", js);
}

#[test]
fn golden_store_initialised_in_js() {
    let src = r#"
store {
    hits: Number = 0
    theme: String = "dark"
}
layout MainLayout { main { slot content } }
page "home" { h1 "hi" }
"#;
    let doc = parse_webc(src).expect("parse");
    assert_eq!(doc.store.len(), 2, "store should have 2 vars");
    let js = generate_runtime_js(&[], &doc);
    assert!(js.contains("STORE.set('hits',0)"), "store init missing:\n{}", js);
    assert!(js.contains("STORE.set('theme',\"dark\")"), "store string init missing:\n{}", js);
    assert!(js.contains("const STORE=new State()"), "STORE instance missing");
}

#[test]
fn golden_store_expression_compiles() {
    let (_html, js) = compile_full(r#"
store { count: Number = 0 }
layout MainLayout { main { slot content } }
page "home" { button on:click={$store.count += 1} { "+" } }
"#);
    assert!(
        js.contains("STORE.set('count',STORE.get('count')+1)"),
        "store increment expression missing:\n{}",
        js
    );
}

#[test]
fn golden_validation_js_in_runtime() {
    let js = compile_to_js(r#"
layout MainLayout { main { slot content } }
page "home" {
    form {
        input type="email" name="email" validate:email="Email invalide"
        @error "email" { span "Erreur" }
    }
}
"#);
    assert!(js.contains("validateField"), "validateField missing in runtime");
    assert!(js.contains("bindValidation"), "bindValidation missing in runtime");
    assert!(js.contains("webcoreValidateEmail"), "email check missing in runtime");
}

#[test]
fn golden_tree_shaking_no_bindfor_when_unused() {
    let js = compile_to_js(r#"
layout MainLayout { main { slot content } }
page "home" { h1 "Hello" }
"#);
    assert!(!js.contains("bindFor"), "bindFor should be absent when no @for:\n{}", js);
    assert!(!js.contains("bindValidation"), "bindValidation should be absent when no validation:\n{}", js);
    assert!(!js.contains("nav="), "nav should be absent when no navigation:\n{}", js);
}

#[test]
fn golden_tree_shaking_validation_present_when_used() {
    let js = compile_to_js(r#"
layout MainLayout { main { slot content } }
page "home" {
    form {
        input type="text" name="user" validate:required="Requis"
    }
}
"#);
    assert!(js.contains("validateField"), "validateField should be present when validate: attrs used:\n{}", js);
    assert!(js.contains("bindValidation"), "bindValidation should be present:\n{}", js);
}

#[test]
fn golden_emit_compiles_to_custom_event() {
    let (_html, js) = compile_full(r#"
layout MainLayout { main { slot content } }
page "home" {
    button on:click={emit("myEvent")} { "Fire" }
}
"#);
    assert!(js.contains("CustomEvent"), "CustomEvent missing in compiled emit:\n{}", js);
    assert!(js.contains("dispatchEvent"), "dispatchEvent missing:\n{}", js);
    assert!(js.contains("myEvent"), "event name missing:\n{}", js);
}

#[test]
fn golden_component_on_event_registers_listener() {
    let js = compile_to_js(r#"
layout MainLayout { main { slot content } }
component Notifier {
    view { button on:click={emit("ping")} { "Ping" } }
}
page "home" {
    Notifier on:ping={ping_count = 1} {}
}
"#);
    assert!(js.contains("addEventListener('ping'"), "component event listener missing:\n{}", js);
}

#[test]
fn golden_wasm_loader_in_runtime() {
    let src = r#"
layout MainLayout { main { slot content } }
page "home" { h1 "hi" }
"#;
    let mut doc = parse_webc(src).expect("parse");
    doc.wasm_module = Some("my_project_wasm".to_string());
    let js = generate_runtime_js(&[], &doc);
    assert!(js.contains("import('./wasm/my_project_wasm.js')"), "WASM import missing:\n{}", js);
    assert!(js.contains("const WASM={}"), "WASM object missing:\n{}", js);
    assert!(js.contains("globalThis.wasm=WASM"), "wasm global missing:\n{}", js);
}

#[test]
fn golden_wasm_absent_by_default() {
    let js = compile_to_js(r#"
layout MainLayout { main { slot content } }
page "home" { h1 "hi" }
"#);
    assert!(!js.contains("WASM"), "WASM should be absent when wasm_module is None:\n{}", js);
}

#[test]
fn minify_js_strips_comments_and_empty_lines() {
    let input = "// comment\nconst x=1;\n\nconst y=2;\n";
    let out = minify_js(input);
    assert!(!out.contains("//"), "comment not removed");
    assert!(!out.contains('\n'), "newline not removed");
    assert!(out.contains("const x=1;"));
    assert!(out.contains("const y=2;"));
}

#[test]
fn minify_js_runtime_is_valid_js_shell() {
    let js = compile_to_js(r#"
layout MainLayout { main { slot content } }
page "home" { h1 "hi" }
"#);
    let minified = minify_js(&js);
    assert!(minified.starts_with('{'), "block open missing");
    assert!(minified.ends_with('}'), "block close missing");
    assert!(!minified.contains("//"));
    assert!(minified.len() < js.len(), "minified should be shorter");
}

#[test]
fn golden_computed_emits_rebind_in_js() {
    let js = compile_to_js(r#"
layout MainLayout { main { slot content } }
component Calc {
    state {
        a: Number = 2
        b: Number = 3
    }
    computed { sum = a + b }
    view { p "{sum}" }
}
page "home" { Calc {} }
"#);
    assert!(js.contains("rebindComputed"), "rebindComputed missing:\n{}", js);
    assert!(js.contains("COMPUTED"), "COMPUTED array missing:\n{}", js);
    assert!(js.contains("'sum'"), "computed var name missing:\n{}", js);
    assert!(js.contains("rebindComputed()"), "bind does not call rebindComputed:\n{}", js);
    assert!(js.contains("setQ(k,v)"), "setQ missing in State class:\n{}", js);
}

#[test]
fn golden_computed_no_array_when_empty() {
    let js = compile_to_js(r#"
layout MainLayout { main { slot content } }
component Simple {
    state { count: Number = 0 }
    view { p "{count}" }
}
page "home" { Simple {} }
"#);
    assert!(!js.contains("const COMPUTED="), "COMPUTED array should be absent when no computed vars:\n{}", js);
    assert!(!js.contains("rebindComputed"), "rebindComputed should be absent when no computed vars:\n{}", js);
    assert!(js.contains(attr_names::INTERPOLATION), "bind() should wire interpolations:\n{}", js);
}

#[test]
fn golden_on_mount_body_in_domcontentloaded() {
    let js = compile_to_js(r#"
layout MainLayout { main { slot content } }
component Loader {
    state { data: String = "" }
    on:mount {
        data = "loaded"
    }
    view { p "{data}" }
}
page "home" { Loader {} }
"#);
    assert!(js.contains("DOMContentLoaded"), "DOMContentLoaded missing:\n{}", js);
    assert!(js.contains("loaded"), "on:mount body content missing:\n{}", js);
}

#[test]
fn golden_on_destroy_emits_hooks() {
    let js = compile_to_js(r#"
layout MainLayout { main { slot content } }
component Cleanup {
    on:destroy {
        clearInterval(timer)
    }
    view { p "test" }
}
page "home" { Cleanup {} }
"#);
    assert!(js.contains("DESTROY_HOOKS"), "DESTROY_HOOKS missing:\n{}", js);
    assert!(js.contains("runDestroyHooks"), "runDestroyHooks missing:\n{}", js);
    assert!(js.contains("clearInterval"), "destroy body content missing:\n{}", js);
    assert!(js.contains("beforeunload"), "beforeunload listener missing:\n{}", js);
}

#[test]
fn golden_no_destroy_hooks_when_absent() {
    let js = compile_to_js(r#"
layout MainLayout { main { slot content } }
component Simple {
    view { p "test" }
}
page "home" { Simple {} }
"#);
    assert!(!js.contains("DESTROY_HOOKS"), "DESTROY_HOOKS should be absent:\n{}", js);
    assert!(!js.contains("beforeunload"), "beforeunload should be absent:\n{}", js);
}

#[test]
fn golden_param_routes_emit_routes_array() {
    let js = compile_to_js(r#"
app MyApp {
    routes {
        "/": HomePage
        "/post/:slug": PostPage
    }
}
layout MainLayout { main { slot content } }
page "home" { p "home" }
page "post" { p "post {$route.slug}" }
"#);
    assert!(js.contains("const ROUTES=["), "ROUTES array missing: {js}");
    assert!(js.contains("ROUTE_PARAMS"), "ROUTE_PARAMS missing: {js}");
    assert!(js.contains("slug"), "slug param missing in routes: {js}");
}

#[test]
fn golden_non_param_routes_use_tofile() {
    let js = compile_to_js(r#"
app MyApp {
    routes {
        "/": HomePage
        "/about": AboutPage
    }
}
layout MainLayout { main { slot content } }
page "home" { p "home" }
page "about" { p "about" }
"#);
    assert!(js.contains("const toFile="), "toFile function missing for non-param routes: {js}");
    assert!(!js.contains("ROUTE_PARAMS"), "ROUTE_PARAMS should not be emitted for non-param routes: {js}");
}

#[test]
fn golden_query_params_generates_proxy() {
    let src = r#"
layout MainLayout { main { slot content } }
page "search" {
    p "{$query.search}"
}
"#;
    let doc = parse_webc(src).expect("parse");
    let js = generate_runtime_js(&[], &doc);
    assert!(js.contains("QUERY_PARAMS"), "QUERY_PARAMS proxy missing:\n{}", js);
    assert!(js.contains("URLSearchParams"), "URLSearchParams missing:\n{}", js);
    assert!(js.contains("$query.") || js.contains("QUERY_PARAMS"), "query param support missing:\n{}", js);
}

#[test]
fn golden_debounce_wraps_handler() {
    let src = r#"
layout MainLayout { main { slot content } }
component Search {
    state { search: String = "" }
    view {
        input on:input|debounce={search = event.target.value}
    }
}
page "home" { Search {} }
"#;
    let doc = parse_webc(src).expect("parse");
    let res = generate_html(&doc, "home", &opts()).expect("codegen");
    assert!(res.handlers.iter().any(|h| h.event_type.contains("debounce")), "no debounce handler registered: {:?}", res.handlers);
    let js = generate_runtime_js(&res.handlers, &doc);
    assert!(js.contains("setTimeout"), "setTimeout missing — debounce not applied:\n{}", js);
    assert!(js.contains("clearTimeout"), "clearTimeout missing — debounce not applied:\n{}", js);
    assert!(js.contains("S.set('search'"), "state update missing in debounce handler:\n{}", js);
}

#[test]
fn golden_http_block_generates_fetch() {
    let src = r#"
layout MainLayout { main { slot content } }
component Posts {
    state {
        posts: List = null
    }
    http {
        get:  "/api/posts"
        into: posts
    }
    view {
        @if loading { p "Loading..." }
        @if error   { p "Error: {error}" }
        @for post in posts { li "{post.title}" }
    }
}
page "home" { Posts {} }
"#;
    let doc = parse_webc(src).expect("parse");
    let comp = doc.components.get("Posts").expect("Posts component");
    assert!(comp.state.iter().any(|v| v.name == "loading"), "loading not auto-injected into state");
    assert!(comp.state.iter().any(|v| v.name == "error"), "error not auto-injected into state");
    let loading_var = comp.state.iter().find(|v| v.name == "loading").unwrap();
    assert_eq!(loading_var.default_value.as_deref(), Some("true"));

    let js = generate_runtime_js(&[], &doc);
    assert!(js.contains("S.set('loading',true)"), "loading init missing:\n{}", js);
    assert!(js.contains("S.set('error',\"\")"), "error init missing:\n{}", js);
    assert!(js.contains("fetch(\"/api/posts\")"), "fetch call missing:\n{}", js);
    assert!(js.contains("S.set('posts'"), "S.set for posts missing:\n{}", js);
    assert!(js.contains("S.set('loading',false)"), "loading=false missing:\n{}", js);
    assert!(js.contains("__r.json()"), "json() call missing:\n{}", js);
}

#[test]
fn golden_signals_state_has_dep_tracking() {
    let src = r#"
layout MainLayout { main { slot content } }
component Counter {
    state { count: Int = 0 }
    view {
        @if count > 0 { p "yes" }
    }
}
page "home" { Counter {} }
"#;
    let doc = parse_webc(src).expect("parse");
    let js = generate_runtime_js(&[], &doc);
    assert!(js.contains("__wcfx"), "__wcfx dep-tracking variable missing:\n{}", js);
    assert!(js.contains("$effect("), "$effect() call missing:\n{}", js);
    assert!(!js.contains("VARS.forEach(v=>S.on("), "old VARS.forEach subscription pattern should not be present:\n{}", js);
}

#[test]
fn golden_signals_early_exit_on_same_value() {
    let src = r#"
layout MainLayout { main { slot content } }
component Counter {
    state { count: Int = 0 }
    view { p "{count}" }
}
page "home" { Counter {} }
"#;
    let doc = parse_webc(src).expect("parse");
    let js = generate_runtime_js(&[], &doc);
    assert!(js.contains("Object.is"), "Object.is early-exit missing from State.set():\n{}", js);
}

#[test]
fn golden_signals_no_subscription_sprawl() {
    let src = r#"
layout MainLayout { main { slot content } }
component Multi {
    state {
        a: Int = 0
        b: Int = 0
        c: Int = 0
    }
    view {
        @if a > 0 { p "a positive" }
        @if b > 0 { p "b positive" }
        @if c > 0 { p "c positive" }
    }
}
page "home" { Multi {} }
"#;
    let doc = parse_webc(src).expect("parse");
    let js = generate_runtime_js(&[], &doc);
    assert!(js.contains("$effect(upd)"), "$effect(upd) missing — signals not used for @if bindings:\n{}", js);
    assert!(!js.contains("VARS.forEach(v=>S.on("), "old subscription sprawl pattern must not appear:\n{}", js);
    assert!(js.contains("__wcfx"), "__wcfx missing — signals dep tracking not emitted:\n{}", js);
}
