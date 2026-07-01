//! Golden / integration tests: full parse → codegen pipeline.

use crate::codegen::attr_names;
use crate::codegen::css::generate_combined_css;
use crate::codegen::html::{generate_html, HtmlPageOptions};
use crate::codegen::js::{generate_runtime_js, minify_js};
use crate::core::ast::{self, Component, Span, WebCoreDocument};
use crate::parser::parse_webc;
use std::collections::BTreeMap;
use std::sync::Mutex;

/// Serialize tests that mutate NO_COLOR to prevent parallel race conditions.
static NO_COLOR_LOCK: Mutex<()> = Mutex::new(());

fn opts() -> HtmlPageOptions {
    HtmlPageOptions {
        lang: "en".into(),
        title: "Test".into(),
        extra_css_files: vec![],
        critical_css: None,
        csp_meta: None,
        prod: false,
        source_maps: false,
        inline_runtime: true,
    }
}

/// Parse + generate HTML for page "home".  Panics on parse or codegen error.
fn compile_to_html(src: &str) -> String {
    let doc = parse_webc(src).expect("parse");
    generate_html(&doc, "home", &opts()).expect("codegen").html
}

/// Parse + generate the runtime JS (no HTML handlers).  Panics on parse error.
fn compile_to_js(src: &str) -> String {
    let doc = parse_webc(src).expect("parse");
    generate_runtime_js(&[], &doc)
}

/// Parse, generate HTML for page "home", then generate the full runtime JS
/// (including HTML-collected event handlers).  Returns (html, js).
fn compile_full(src: &str) -> (String, String) {
    let doc = parse_webc(src).expect("parse");
    let res = generate_html(&doc, "home", &opts()).expect("codegen");
    let js = generate_runtime_js(&res.handlers, &doc);
    (res.html, js)
}

// ── Parser + HTML codegen ──────────────────────────────────────────────────

#[test]
fn golden_page_heading_renders() {
    let html = compile_to_html(
        r#"
layout MainLayout { main { slot content } }
page "home" { h1 "Hello WebCore!" }
"#,
    );
    // codegen adds newlines between child elements, so check open tag + content, not the full inline form
    assert!(html.contains("<h1>Hello WebCore!"), "h1 missing:\n{}", html);
    assert!(html.contains("</h1>"), "h1 close tag missing");
    assert!(html.contains("lang=\"en\""));
    assert!(html.contains("<title>Test</title>"));
}

#[test]
fn golden_state_interpolation_emits_span() {
    let html = compile_to_html(
        r#"
layout MainLayout { main { slot content } }
component Counter {
    state { count: Number = 0 }
    view { p "Value: {count}" }
}
page "home" { Counter {} }
"#,
    );
    // v3: expression is compiled to an ID (e0, e1, …), not stored as raw expr
    assert!(
        html.contains(&format!("{}=\"", attr_names::INTERPOLATION)),
        "interpolation span missing:\n{}",
        html
    );
}

#[test]
fn golden_onclick_produces_js_handler() {
    let src = r#"
layout MainLayout { main { slot content } }
page "home" {
    button on:click={count += 1} { "+" }
}
"#;
    let doc = parse_webc(src).expect("parse");
    let res = generate_html(&doc, "home", &opts()).expect("codegen");
    // HTML should use data-webcore-e delegation (CSP-safe, no inline onclick=)
    assert!(
        res.html.contains("data-webcore-e="),
        "data-webcore-e delegation attribute missing:\n{}",
        res.html
    );
    // Handler must be registered
    assert!(!res.handlers.is_empty(), "no handlers registered");
    // JS must contain the compiled expression
    let js = generate_runtime_js(&res.handlers, &doc);
    assert!(
        js.contains("S.get('count')+1") || js.contains("S.get(&#x27;count&#x27;)"),
        "compiled expression missing in JS:\n{}",
        js
    );
}

#[test]
fn golden_state_initialised_in_js() {
    let js = compile_to_js(
        r#"
layout MainLayout { main { slot content } }
component Counter {
    state { count: Number = 0 }
    view { p "{count}" }
}
page "home" { Counter {} }
"#,
    );
    assert!(
        js.contains("S.set('count',0)"),
        "state init missing:\n{}",
        js
    );
}

// ── CSS codegen ────────────────────────────────────────────────────────────

#[test]
fn golden_scoped_css_emits_data_v_selector() {
    let mut doc = WebCoreDocument {
        app: None,
        store: vec![],
        store_computed: vec![],
        locales: std::collections::BTreeMap::new(),
        default_locale: String::new(),
        wasm_module: None,
        layouts: std::collections::BTreeMap::new(),
        pages: std::collections::BTreeMap::new(),
        components: std::collections::BTreeMap::new(),
        imports: vec![],
        data_imports: std::collections::BTreeMap::new(),
        component_imports: vec![],
        page_imports: std::collections::BTreeMap::new(),
        source_files: std::collections::BTreeMap::new(),
    };
    doc.components.insert(
        "Counter".into(),
        Component {
            name: "Counter".into(),
            props: vec![],
            state: vec![],
            computed: vec![],
            mount_body: None,
            destroy_body: None,
            watch_hooks: vec![],
            http: None,
            view: vec![],
            style: vec![crate::core::ast::StyleItem::Rule(
                crate::core::ast::StyleRule {
                    selector: "button".into(),
                    properties: vec![crate::core::ast::StyleProperty {
                        name: "color".into(),
                        value: "red".into(),
                        span: Span::default(),
                    }],
                    nested: vec![],
                    span: Span::default(),
                },
            )],
            span: Span::default(),
        },
    );
    let css = generate_combined_css(None, &doc);
    assert!(
        css.contains(&format!("[{}=", attr_names::SCOPE)),
        "no scoped selector in:\n{}",
        css
    );
    assert!(css.contains("color: red") || css.contains("color:red"));
}

// ── Props inter-composants ─────────────────────────────────────────────────

#[test]
fn golden_prop_substituted_in_view() {
    let html = compile_to_html(
        r#"
layout MainLayout { main { slot content } }
component Greeting {
    props { name: String }
    view { p "Hello {name}!" }
}
page "home" { Greeting name="WebCore" {} }
"#,
    );
    assert!(html.contains("Hello"), "Hello fragment missing:\n{}", html);
    assert!(html.contains("WebCore"), "prop value missing:\n{}", html);
    assert!(
        !html.contains(&format!("{}=\"name\"", attr_names::INTERPOLATION)),
        "unresolved prop span still present:\n{}",
        html
    );
}

// ── Store global ───────────────────────────────────────────────────────────

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
    assert!(
        js.contains("STORE.set('hits',0)"),
        "store init missing:\n{}",
        js
    );
    assert!(
        js.contains("STORE.set('theme',\"dark\")"),
        "store string init missing:\n{}",
        js
    );
    assert!(
        js.contains("const STORE=new State()"),
        "STORE instance missing"
    );
}

#[test]
fn golden_store_expression_compiles() {
    let (_html, js) = compile_full(
        r#"
store { count: Number = 0 }
layout MainLayout { main { slot content } }
page "home" { button on:click={$store.count += 1} { "+" } }
"#,
    );
    assert!(
        js.contains("STORE.set('count',STORE.get('count')+1)"),
        "store increment expression missing:\n{}",
        js
    );
}

// ── Validation de formulaires ──────────────────────────────────────────────

#[test]
fn golden_validate_attrs_converted_to_data_attrs() {
    let html = compile_to_html(
        r#"
layout MainLayout { main { slot content } }
page "home" {
    form {
        input type="text" name="username"
              validate:required="Le nom est requis"
              validate:minlength="3,Au moins 3 caractères"
    }
}
"#,
    );
    assert!(
        html.contains("data-webcore-field=\"username\""),
        "data-webcore-field missing:\n{}",
        html
    );
    assert!(
        html.contains("data-webcore-validate-required=\"Le nom est requis\""),
        "validate-required missing:\n{}",
        html
    );
    assert!(
        html.contains("data-webcore-validate-minlength=\"3\""),
        "validate-minlength missing:\n{}",
        html
    );
    assert!(
        html.contains("data-webcore-validate-minlength-msg="),
        "validate-minlength-msg missing:\n{}",
        html
    );
    assert!(
        !html.contains("validate:required"),
        "raw validate: attr should not appear in output:\n{}",
        html
    );
}

#[test]
fn golden_error_block_renders_hidden_div() {
    let html = compile_to_html(
        r#"
layout MainLayout { main { slot content } }
page "home" {
    form {
        input type="text" name="email" validate:email="Email invalide"
        @error "email" { span "Erreur email" }
    }
}
"#,
    );
    assert!(
        html.contains(&format!("{}=\"email\"", attr_names::ERROR)),
        "data-webcore-error missing:\n{}",
        html
    );
    assert!(
        html.contains("style=\"display:none\""),
        "error block should be hidden by default:\n{}",
        html
    );
}

#[test]
fn golden_validation_js_in_runtime() {
    let js = compile_to_js(
        r#"
layout MainLayout { main { slot content } }
page "home" {
    form {
        input type="email" name="email" validate:email="Email invalide"
        @error "email" { span "Erreur" }
    }
}
"#,
    );
    assert!(
        js.contains("validateField"),
        "validateField missing in runtime"
    );
    assert!(
        js.contains("bindValidation"),
        "bindValidation missing in runtime"
    );
    assert!(
        js.contains("webcoreValidateEmail"),
        "email check missing in runtime"
    );
}

#[test]
fn golden_tree_shaking_no_bindfor_when_unused() {
    let js = compile_to_js(
        r#"
layout MainLayout { main { slot content } }
page "home" { h1 "Hello" }
"#,
    );
    assert!(
        !js.contains("bindFor"),
        "bindFor should be absent when no @for:\n{}",
        js
    );
    assert!(
        !js.contains("bindValidation"),
        "bindValidation should be absent when no validation:\n{}",
        js
    );
    assert!(
        !js.contains("nav="),
        "nav should be absent when no navigation:\n{}",
        js
    );
}

#[test]
fn golden_tree_shaking_validation_present_when_used() {
    let js = compile_to_js(
        r#"
layout MainLayout { main { slot content } }
page "home" {
    form {
        input type="text" name="user" validate:required="Requis"
    }
}
"#,
    );
    assert!(
        js.contains("validateField"),
        "validateField should be present when validate: attrs used:\n{}",
        js
    );
    assert!(
        js.contains("bindValidation"),
        "bindValidation should be present:\n{}",
        js
    );
}

// ── SSG ───────────────────────────────────────────────────────────────────

#[test]
fn golden_ssg_interpolation_prerendered() {
    let src = r#"
layout MainLayout { main { slot content } }
component Counter {
    state { count: Number = 7 }
    view { p "Valeur : {count}" }
}
page "home" { Counter {} }
"#;
    let doc = parse_webc(src).expect("parse");
    let initial = crate::core::ssg::build_initial_state(&doc);
    let locales = BTreeMap::new();
    let ssg_ctx = crate::core::ssg::SsgContext {
        state: &initial,
        locales: &locales,
        locale: "",
    };
    let ssg = crate::codegen::html::generate_page(&doc, "home", &opts(), None, Some(&ssg_ctx))
        .expect("codegen")
        .html;
    // v3: expr ID replaces raw expr; SSG still pre-renders the value as text content
    assert!(
        ssg.contains(&format!("{}=\"", attr_names::INTERPOLATION)) && ssg.contains(">7</span>"),
        "interpolation span not pre-rendered:\n{}",
        ssg
    );
}

#[test]
fn golden_ssg_if_display_preset() {
    let src = r#"
layout MainLayout { main { slot content } }
component Widget {
    state { show: Number = 1 }
    view {
        @if show > 0 {
            p "Visible"
        } @else {
            p "Hidden"
        }
    }
}
page "home" { Widget {} }
"#;
    let doc = parse_webc(src).expect("parse");
    let initial = crate::core::ssg::build_initial_state(&doc);
    let locales = BTreeMap::new();
    let ssg_ctx = crate::core::ssg::SsgContext {
        state: &initial,
        locales: &locales,
        locale: "",
    };
    let ssg = crate::codegen::html::generate_page(&doc, "home", &opts(), None, Some(&ssg_ctx))
        .expect("codegen")
        .html;
    // v3: raw expr replaced by compiled ID; SSG still pre-sets display style
    assert!(
        ssg.contains(&format!("{}=\"", attr_names::IF)) && ssg.contains(r#"style="display:block""#),
        "@if branch not pre-rendered as visible:\n{}",
        ssg
    );
    assert!(
        ssg.contains(&format!("{}=\"", attr_names::IF_ELSE))
            && ssg.contains(r#"style="display:none""#),
        "@else branch not pre-rendered as hidden:\n{}",
        ssg
    );
}

// ── i18n ──────────────────────────────────────────────────────────────────

#[test]
fn golden_i18n_runtime_contains_locales() {
    let src = r#"
layout MainLayout { main { slot content } }
page "home" { h1 "hi" }
"#;
    let mut doc = parse_webc(src).expect("parse");
    let mut fr: BTreeMap<String, String> = BTreeMap::new();
    fr.insert("welcome".to_string(), "Bienvenue".to_string());
    fr.insert("counter".to_string(), "Compteur".to_string());
    doc.locales.insert("fr".to_string(), fr);
    doc.default_locale = "fr".to_string();

    let js = generate_runtime_js(&[], &doc);
    assert!(js.contains("const LOCALES="), "LOCALES missing:\n{}", js);
    assert!(js.contains("Bienvenue"), "translation missing:\n{}", js);
    assert!(js.contains("Compteur"), "translation missing:\n{}", js);
    assert!(js.contains("const t="), "t() missing:\n{}", js);
    assert!(js.contains("let LOCALE=\"fr\""), "LOCALE missing:\n{}", js);
    assert!(
        js.contains("const setLocale="),
        "setLocale missing:\n{}",
        js
    );
    assert!(js.contains("setLocale"), "setLocale not exported:\n{}", js);
}

#[test]
fn golden_i18n_ssg_prerender() {
    // interp_expr = (!"}" ~ ANY)+ so inner " are fine inside {t("key")}
    let src = r##"
layout MainLayout { main { slot content } }
page "home" { p "{t("welcome")}" }
"##;
    let mut doc = parse_webc(src).expect("parse");
    let mut fr: BTreeMap<String, String> = BTreeMap::new();
    fr.insert("welcome".to_string(), "Bienvenue".to_string());
    doc.locales.insert("fr".to_string(), fr);
    doc.default_locale = "fr".to_string();

    let res = generate_html(&doc, "home", &opts()).expect("codegen");
    // Interpolation span must exist
    assert!(
        res.html.contains(attr_names::INTERPOLATION),
        "no interpolation span:\n{}",
        res.html
    );
    let state = crate::core::ssg::build_initial_state(&doc);
    let ssg_ctx = crate::core::ssg::SsgContext {
        state: &state,
        locales: &doc.locales,
        locale: "fr",
    };
    let ssg = crate::codegen::html::generate_page(&doc, "home", &opts(), None, Some(&ssg_ctx))
        .expect("codegen")
        .html;
    assert!(
        ssg.contains("Bienvenue"),
        "translation not pre-rendered:\n{}",
        ssg
    );
}

#[test]
fn golden_i18n_no_locales_runtime_omits_t() {
    let js = compile_to_js(
        r#"
layout MainLayout { main { slot content } }
page "home" { h1 "hi" }
"#,
    );
    assert!(
        !js.contains("const LOCALES="),
        "LOCALES should be absent when no locales:\n{}",
        js
    );
}

// ── WASM ──────────────────────────────────────────────────────────────────

#[test]
fn golden_wasm_loader_in_runtime() {
    let src = r#"
layout MainLayout { main { slot content } }
page "home" { h1 "hi" }
"#;
    let mut doc = parse_webc(src).expect("parse");
    doc.wasm_module = Some("my_project_wasm".to_string());

    let js = generate_runtime_js(&[], &doc);
    assert!(
        js.contains("import('./wasm/my_project_wasm.js')"),
        "WASM import missing:\n{}",
        js
    );
    assert!(js.contains("const WASM={}"), "WASM object missing:\n{}", js);
    assert!(
        js.contains("globalThis.wasm=WASM"),
        "wasm global missing:\n{}",
        js
    );
}

#[test]
fn golden_wasm_absent_by_default() {
    let js = compile_to_js(
        r#"
layout MainLayout { main { slot content } }
page "home" { h1 "hi" }
"#,
    );
    assert!(
        !js.contains("WASM"),
        "WASM should be absent when wasm_module is None:\n{}",
        js
    );
}

// ── JS minification ────────────────────────────────────────────────────────

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
    let js = compile_to_js(
        r#"
layout MainLayout { main { slot content } }
page "home" { h1 "hi" }
"#,
    );
    let minified = minify_js(&js);
    assert!(minified.starts_with('{'), "block open missing");
    assert!(minified.ends_with('}'), "block close missing");
    assert!(!minified.contains("//"));
    assert!(minified.len() < js.len(), "minified should be shorter");
}

// ── Props réactives (v0.8.0) ──────────────────────────────────────────────

#[test]
fn golden_reactive_prop_stays_interpolation() {
    let html = compile_to_html(
        r#"
layout MainLayout { main { slot content } }
component Badge {
    props { label: String }
    view { span "Valeur : {label}" }
}
page "home" {
    Badge label={count} {}
}
"#,
    );
    // v3: expression compiled to an ID; check the attr is present, not its raw value
    assert!(
        html.contains(&format!("{}=\"", attr_names::INTERPOLATION)),
        "reactive prop should produce interpolation span:\n{}",
        html
    );
    assert!(
        !html.contains(&format!("{}=\"label\"", attr_names::INTERPOLATION)),
        "unresolved `label` span should not appear:\n{}",
        html
    );
}

#[test]
fn golden_static_prop_still_substituted() {
    let html = compile_to_html(
        r#"
layout MainLayout { main { slot content } }
component Greeting {
    props { name: String }
    view { p "Hello {name}!" }
}
page "home" { Greeting name="Alice" {} }
"#,
    );
    assert!(
        html.contains("Alice"),
        "static prop value missing:\n{}",
        html
    );
    assert!(
        !html.contains(&format!("{}=\"name\"", attr_names::INTERPOLATION)),
        "unresolved name span should not appear:\n{}",
        html
    );
}

// ── Named slots (v0.8.0) ──────────────────────────────────────────────────

#[test]
fn golden_named_slot_filled_from_page() {
    let html = compile_to_html(
        r#"
layout MainLayout {
    div {
        header { slot header }
        main { slot content }
    }
}
page "home" {
    slot header { h1 "Titre" }
    p "Contenu principal"
}
"#,
    );
    assert!(
        html.contains("<h1>Titre"),
        "named slot header content missing:\n{}",
        html
    );
    assert!(
        html.contains("<p>Contenu principal"),
        "default content slot missing:\n{}",
        html
    );
}

#[test]
fn golden_unnamed_slot_backwards_compat() {
    let html = compile_to_html(
        r#"
layout MainLayout { main { slot content } }
page "home" { h1 "Simple" }
"#,
    );
    assert!(
        html.contains("<h1>Simple"),
        "backward-compat unnamed slot broken:\n{}",
        html
    );
}

// ── computed (v0.9.0) ────────────────────────────────────────────────────

#[test]
fn golden_computed_emits_rebind_in_js() {
    let js = compile_to_js(
        r#"
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
"#,
    );
    assert!(
        js.contains("rebindComputed"),
        "rebindComputed missing:\n{}",
        js
    );
    assert!(js.contains("COMPUTED"), "COMPUTED array missing:\n{}", js);
    assert!(js.contains("'sum'"), "computed var name missing:\n{}", js);
    assert!(
        js.contains("rebindComputed()"),
        "bind does not call rebindComputed:\n{}",
        js
    );
    assert!(
        js.contains("setQ(k,v)"),
        "setQ missing in State class:\n{}",
        js
    );
}

#[test]
fn golden_computed_no_array_when_empty() {
    let js = compile_to_js(
        r#"
layout MainLayout { main { slot content } }
component Simple {
    state { count: Number = 0 }
    view { p "{count}" }
}
page "home" { Simple {} }
"#,
    );
    assert!(
        !js.contains("const COMPUTED="),
        "COMPUTED array should be absent when no computed vars:\n{}",
        js
    );
    assert!(
        !js.contains("rebindComputed"),
        "rebindComputed should be absent when no computed vars:\n{}",
        js
    );
    assert!(
        js.contains(attr_names::INTERPOLATION),
        "bind() should wire interpolations:\n{}",
        js
    );
}

// ── on:mount (v0.9.0) ────────────────────────────────────────────────────

#[test]
fn golden_on_mount_body_in_domcontentloaded() {
    let js = compile_to_js(
        r#"
layout MainLayout { main { slot content } }
component Loader {
    state { data: String = "" }
    on:mount {
        data = "loaded"
    }
    view { p "{data}" }
}
page "home" { Loader {} }
"#,
    );
    assert!(
        js.contains("DOMContentLoaded"),
        "DOMContentLoaded missing:\n{}",
        js
    );
    assert!(
        js.contains("loaded"),
        "on:mount body content missing:\n{}",
        js
    );
}

// ── on:destroy (v1.0.0) ──────────────────────────────────────────────────

#[test]
fn golden_on_destroy_emits_hooks() {
    let js = compile_to_js(
        r#"
layout MainLayout { main { slot content } }
component Cleanup {
    on:destroy {
        clearInterval(timer)
    }
    view { p "test" }
}
page "home" { Cleanup {} }
"#,
    );
    assert!(
        js.contains("DESTROY_HOOKS"),
        "DESTROY_HOOKS missing:\n{}",
        js
    );
    assert!(
        js.contains("runDestroyHooks"),
        "runDestroyHooks missing:\n{}",
        js
    );
    assert!(
        js.contains("clearInterval"),
        "destroy body content missing:\n{}",
        js
    );
    assert!(
        js.contains("beforeunload"),
        "beforeunload listener missing:\n{}",
        js
    );
}

#[test]
fn golden_no_destroy_hooks_when_absent() {
    let js = compile_to_js(
        r#"
layout MainLayout { main { slot content } }
component Simple {
    view { p "test" }
}
page "home" { Simple {} }
"#,
    );
    assert!(
        !js.contains("DESTROY_HOOKS"),
        "DESTROY_HOOKS should be absent:\n{}",
        js
    );
    assert!(
        !js.contains("beforeunload"),
        "beforeunload should be absent:\n{}",
        js
    );
}

// ── emit / inter-component events (v0.9.0) ──────────────────────────────

#[test]
fn golden_emit_compiles_to_custom_event() {
    let (_html, js) = compile_full(
        r#"
layout MainLayout { main { slot content } }
page "home" {
    button on:click={emit("myEvent")} { "Fire" }
}
"#,
    );
    assert!(
        js.contains("CustomEvent"),
        "CustomEvent missing in compiled emit:\n{}",
        js
    );
    assert!(
        js.contains("dispatchEvent"),
        "dispatchEvent missing:\n{}",
        js
    );
    assert!(js.contains("myEvent"), "event name missing:\n{}", js);
}

#[test]
fn golden_component_on_event_registers_listener() {
    let js = compile_to_js(
        r#"
layout MainLayout { main { slot content } }
component Notifier {
    view { button on:click={emit("ping")} { "Ping" } }
}
page "home" {
    Notifier on:ping={ping_count = 1} {}
}
"#,
    );
    assert!(
        js.contains("addEventListener('ping'"),
        "component event listener missing:\n{}",
        js
    );
}

// ── @media dans les blocs style (v0.8.0) ─────────────────────────────────

#[test]
fn golden_media_block_scoped_in_css() {
    let src = r#"
layout MainLayout { main { slot content } }
component Card {
    view { div "Contenu" }
    style {
        .card { padding: 1rem; }
        @media (max-width: 768px) {
            .card { padding: 0.5rem; }
        }
    }
}
page "home" { Card {} }
"#;
    let doc = parse_webc(src).expect("parse");
    let css = crate::codegen::css::generate_combined_css(None, &doc);
    assert!(
        css.contains("@media (max-width: 768px)"),
        "@media block missing in output:\n{}",
        css
    );
    assert!(
        css.contains(&format!("[{}=", attr_names::SCOPE)),
        "scoped selector missing inside @media:\n{}",
        css
    );
}

// ── v1.1.0 features ───────────────────────────────────────────────────────

#[test]
fn golden_compound_prop_substituted() {
    let html = compile_to_html(
        r#"
layout MainLayout { main { slot content } }
component Counter {
    props { step }
    view { p "{step + 1}" }
}
page "home" { Counter step="2" {} }
"#,
    );
    assert!(
        html.contains("(2) + 1"),
        "compound prop not substituted: {}",
        html
    );
}

#[test]
fn golden_prop_in_attribute_substituted() {
    let html = compile_to_html(
        r#"
layout MainLayout { main { slot content } }
component Badge {
    props { color }
    view { span class={color} "ok" }
}
page "home" { Badge color="green" {} }
"#,
    );
    assert!(
        html.contains("class=\"green\""),
        "prop not substituted in attribute: {}",
        html
    );
}

#[test]
fn golden_i18n_plural_t_function() {
    let src = r#"
layout MainLayout { main { slot content } }
page "home" { p "{t(\"items\", 3)}" }
"#;
    let mut doc = parse_webc(src).expect("parse");
    let mut en_msgs = BTreeMap::new();
    en_msgs.insert("items_one".into(), "{{count}} item".into());
    en_msgs.insert("items_other".into(), "{{count}} items".into());
    doc.locales.insert("en".into(), en_msgs);
    doc.default_locale = "en".into();
    let js = generate_runtime_js(&[], &doc);
    // New t() function signature supports the second arg
    assert!(
        js.contains("typeof a==='number'"),
        "plural t() not emitted: {js}"
    );
    assert!(js.contains("_one"), "plural key suffix _one missing: {js}");
}

#[test]
fn golden_for_key_emits_data_attribute() {
    let html = compile_to_html(
        r#"
layout MainLayout { main { slot content } }
page "home" {
    @for item key=item in items {
        li "{item}"
    }
}
"#,
    );
    assert!(
        html.contains(&format!("{}=\"item\"", attr_names::FOR_KEY)),
        "for-key attribute missing: {}",
        html
    );
}

#[test]
fn golden_param_routes_emit_routes_array() {
    let js = compile_to_js(
        r#"
app MyApp {
    routes {
        "/": HomePage
        "/post/:slug": PostPage
    }
}
layout MainLayout { main { slot content } }
page "home" { p "home" }
page "post" { p "post {$route.slug}" }
"#,
    );
    assert!(js.contains("const ROUTES=["), "ROUTES array missing: {js}");
    assert!(js.contains("ROUTE_PARAMS"), "ROUTE_PARAMS missing: {js}");
    assert!(js.contains("slug"), "slug param missing in routes: {js}");
}

#[test]
fn golden_non_param_routes_use_tofile() {
    let js = compile_to_js(
        r#"
app MyApp {
    routes {
        "/": HomePage
        "/about": AboutPage
    }
}
layout MainLayout { main { slot content } }
page "home" { p "home" }
page "about" { p "about" }
"#,
    );
    assert!(
        js.contains("const toFile="),
        "toFile function missing for non-param routes: {js}"
    );
    assert!(
        !js.contains("ROUTE_PARAMS"),
        "ROUTE_PARAMS should not be emitted for non-param routes: {js}"
    );
}

#[test]
fn golden_error_message_has_caret() {
    // Introduce a parse error: empty interpolation {} is invalid
    let src = "component 9 {}"; // invalid identifier — genuine parse error
    let err = crate::parser::parse_webc(src).unwrap_err();
    let display = format!("{}", err);
    // Should contain a caret line
    assert!(display.contains('^'), "caret missing in error: {display}");
}

#[test]
fn golden_error_message_no_color_format() {
    let _guard = NO_COLOR_LOCK.lock().unwrap();
    // With NO_COLOR set, output is plain ASCII but still structured.
    std::env::set_var("NO_COLOR", "1");
    let src = "component 9 {}"; // invalid identifier — genuine parse error
    let err = crate::parser::parse_webc(src).unwrap_err();
    let display = format!("{err}");
    std::env::remove_var("NO_COLOR");

    assert!(
        !display.contains('\x1b'),
        "ANSI escape found despite NO_COLOR: {display}"
    );
    assert!(
        display.contains("error[parse]"),
        "structured prefix missing: {display}"
    );
    assert!(display.contains('^'), "caret missing: {display}");
}

#[test]
fn golden_error_message_file_path_included() {
    let _guard = NO_COLOR_LOCK.lock().unwrap();
    // When file is set on ParseError, the location string includes the path.
    let src = "component 9 {}"; // invalid identifier — genuine parse error
    let mut err = crate::parser::parse_webc(src).unwrap_err();
    err.file = Some(std::path::PathBuf::from("src/pages/home.webc"));
    std::env::set_var("NO_COLOR", "1");
    let display = format!("{err}");
    std::env::remove_var("NO_COLOR");

    assert!(
        display.contains("src/pages/home.webc"),
        "file path missing in error: {display}"
    );
}

// ── @switch (v1.2.0) ─────────────────────────────────────────────────────────

#[test]
fn golden_switch_expands_to_if_chain() {
    let src = r#"
layout MainLayout { main { slot content } }
page "home" {
    @switch status {
        @case "active"  { span "Active" }
        @case "pending" { span "Pending" }
        @default        { span "Unknown" }
    }
}
"#;
    let (html, js) = compile_full(src);
    assert!(
        js.contains("bindIf"),
        "bindIf missing — @switch should expand to @if chain:\n{js}"
    );
    assert!(
        html.contains("status === &quot;active&quot;") || html.contains("status === \"active\""),
        "case condition missing in HTML:\n{}",
        html
    );
    assert!(html.contains("Active"), "case content missing");
    assert!(html.contains("Unknown"), "default content missing");
}

#[test]
fn golden_switch_without_default() {
    let html = compile_to_html(
        r#"
layout MainLayout { main { slot content } }
page "home" {
    @switch role {
        @case "admin" { span "Admin panel" }
        @case "user"  { span "User view" }
    }
}
"#,
    );
    assert!(html.contains("Admin panel"), "admin case missing");
    assert!(html.contains("User view"), "user case missing");
}

// ── bind: two-way binding (v1.2.0) ───────────────────────────────────────────

#[test]
fn golden_bind_value_expands_to_attr_and_handler() {
    let src = r#"
layout MainLayout { main { slot content } }
component Form {
    state { username: String = "" }
    view {
        input type="text" bind:value={username}
        p "Hello {username}"
    }
}
page "home" { Form {} }
"#;
    let doc = parse_webc(src).expect("parse");
    let res = generate_html(&doc, "home", &opts()).expect("codegen");
    assert!(
        res.html.contains("data-webcore-attr-value"),
        "dynamic value binding missing:\n{}",
        res.html
    );
    assert!(
        res.html.contains("username = event.target.value")
            || res
                .handlers
                .iter()
                .any(|h| h.expression.contains("username = event.target.value")),
        "on:input handler missing:\n{:?}",
        res.handlers
    );
}

#[test]
fn golden_bind_checked_uses_onchange() {
    let src = r#"
layout MainLayout { main { slot content } }
component Toggle {
    state { active: Boolean = false }
    view { input type="checkbox" bind:checked={active} }
}
page "home" { Toggle {} }
"#;
    let doc = parse_webc(src).expect("parse");
    let res = generate_html(&doc, "home", &opts()).expect("codegen");
    assert!(
        res.html.contains("data-webcore-attr-checked")
            || res
                .handlers
                .iter()
                .any(|h| h.expression.contains("event.target.checked")),
        "checked binding missing:\n{}",
        res.html
    );
}

// ── v1.3.0 features ───────────────────────────────────────────────────────────

#[test]
fn golden_http_block_generates_fetch() {
    // `loading` and `error` are NOT declared in state — they should be auto-injected.
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

    // Auto-injection: `loading` and `error` should appear in the component state
    let comp = doc.components.get("Posts").expect("Posts component");
    assert!(
        comp.state.iter().any(|v| v.name == "loading"),
        "loading not auto-injected into state"
    );
    assert!(
        comp.state.iter().any(|v| v.name == "error"),
        "error not auto-injected into state"
    );
    let loading_var = comp.state.iter().find(|v| v.name == "loading").unwrap();
    assert_eq!(loading_var.default_value.as_deref(), Some("true"));

    let js = generate_runtime_js(&[], &doc);
    // State initialisation: auto-injected vars get S.set() calls
    assert!(
        js.contains("S.set('loading',true)"),
        "loading init missing:\n{}",
        js
    );
    assert!(
        js.contains("S.set('error',\"\")"),
        "error init missing:\n{}",
        js
    );
    // Fetch call
    assert!(
        js.contains("fetch(\"/api/posts\")"),
        "fetch call missing:\n{}",
        js
    );
    assert!(
        js.contains("S.set('posts'"),
        "S.set for posts missing:\n{}",
        js
    );
    assert!(
        js.contains("S.set('loading',false)"),
        "loading=false missing:\n{}",
        js
    );
    assert!(js.contains("__r.json()"), "json() call missing:\n{}", js);
}

#[test]
fn golden_head_block_generates_meta() {
    let src = r#"
layout MainLayout { main { slot content } }
page "article" {
    head {
        title "Mon Article"
        meta description="Article de blog WebCore"
        meta og:title="Mon Article"
        favicon "/assets/logo.png"
    }
    h1 "Hello"
}
"#;
    let doc = parse_webc(src).expect("parse");
    // Verify head block was parsed
    let page = doc.pages.get("article").expect("page exists");
    let head = page.head.as_ref().expect("head block present");
    assert_eq!(head.title.as_deref(), Some("Mon Article"), "title mismatch");
    assert_eq!(head.metas.len(), 2, "expected 2 meta tags");
    assert!(
        head.metas
            .iter()
            .any(|(k, v)| k == "description" && v == "Article de blog WebCore"),
        "description meta missing: {:?}",
        head.metas
    );
    assert!(
        head.metas
            .iter()
            .any(|(k, v)| k == "og:title" && v == "Mon Article"),
        "og:title meta missing: {:?}",
        head.metas
    );
    assert_eq!(
        head.favicon.as_deref(),
        Some("/assets/logo.png"),
        "favicon mismatch"
    );
}

#[test]
fn golden_head_block_emitted_into_html() {
    let src = r#"
layout MainLayout { main { slot content } }
page "home" {
    head {
        title "Titre Page"
        meta description="Desc"
        meta og:title="OG"
        favicon "/assets/logo.png"
    }
    h1 "Hi"
}
"#;
    let doc = parse_webc(src).expect("parse");
    let html = generate_html(&doc, "home", &opts()).expect("codegen").html;
    // Per-page title overrides the global one.
    assert!(
        html.contains("<title>Titre Page</title>"),
        "head title not emitted:\n{html}"
    );
    // Plain meta uses name=, OpenGraph uses property=.
    assert!(
        html.contains(r#"<meta name="description" content="Desc">"#),
        "description meta not emitted:\n{html}"
    );
    assert!(
        html.contains(r#"<meta property="og:title" content="OG">"#),
        "og:title should use property=:\n{html}"
    );
    assert!(
        html.contains(r#"<link rel="icon" href="/assets/logo.png">"#),
        "favicon link not emitted:\n{html}"
    );
}

#[test]
fn golden_app_head_merges_into_pages() {
    let src = r#"
app MyApp {
    layout: MainLayout
    head {
        favicon "/assets/logo.png"
        meta author="WebCore"
        meta description="Globale"
    }
    routes { "/": HomePage }
}
layout MainLayout { main { slot content } }
page "home" {
    h1 "Accueil"
}
page "about" {
    head {
        title "À propos"
        meta description="Spécifique"
        meta og:title="About"
    }
    h1 "À propos"
}
"#;
    let doc = parse_webc(src).expect("parse");

    // Page with no head{} inherits the whole site-wide head.
    let home = generate_html(&doc, "home", &opts())
        .expect("home codegen")
        .html;
    assert!(
        home.contains(r#"<link rel="icon" href="/assets/logo.png">"#)
            && home.contains(r#"<meta name="author" content="WebCore">"#)
            && home.contains(r#"<meta name="description" content="Globale">"#),
        "home should inherit the app head:\n{home}"
    );

    // Page head{} overrides title and the shared meta key, keeps inherited ones,
    // and adds its own — favicon still inherited from the app.
    let about = generate_html(&doc, "about", &opts())
        .expect("about codegen")
        .html;
    assert!(
        about.contains("<title>À propos</title>"),
        "about title:\n{about}"
    );
    assert!(
        about.contains(r#"<meta name="description" content="Spécifique">"#)
            && !about.contains(r#"content="Globale""#),
        "page description should override the global one:\n{about}"
    );
    assert!(
        about.contains(r#"<meta name="author" content="WebCore">"#),
        "about should keep the inherited author meta:\n{about}"
    );
    assert!(
        about.contains(r#"<meta property="og:title" content="About">"#)
            && about.contains(r#"<link rel="icon" href="/assets/logo.png">"#),
        "about should add og:title and inherit the favicon:\n{about}"
    );
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
    assert!(
        js.contains("QUERY_PARAMS"),
        "QUERY_PARAMS proxy missing:\n{}",
        js
    );
    assert!(
        js.contains("URLSearchParams"),
        "URLSearchParams missing:\n{}",
        js
    );
    assert!(
        js.contains("$query.") || js.contains("QUERY_PARAMS"),
        "query param support missing:\n{}",
        js
    );
}

#[test]
fn golden_class_binding_emits_data_attr() {
    let (html, js) = compile_full(
        r#"
layout MainLayout { main { slot content } }
component Toggle {
    state { isActive: Boolean = false }
    view {
        div class:active={isActive} { "content" }
    }
}
page "home" { Toggle {} }
"#,
    );
    // v3: value is a compiled expression ID, not the raw expr name
    assert!(
        html.contains(&format!("{}active=\"", attr_names::CLASS_PREFIX)),
        "data-webcore-class-active attribute missing:\n{}",
        html
    );
    assert!(
        html.contains(attr_names::CLASS_BOUND),
        "data-webcore-class-bound marker missing:\n{}",
        html
    );
    assert!(
        !html.contains("class:active"),
        "raw class:active should not appear in output:\n{}",
        html
    );
    assert!(
        js.contains("bindClassBindings"),
        "bindClassBindings missing in JS:\n{}",
        js
    );
    assert!(
        js.contains("[data-webcore-class-bound]"),
        "bindClassBindings should use targeted selector, not querySelectorAll('*'):\n{}",
        js
    );
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
    assert!(
        res.handlers
            .iter()
            .any(|h| h.event_type.contains("debounce")),
        "no debounce handler registered: {:?}",
        res.handlers
    );
    let js = generate_runtime_js(&res.handlers, &doc);
    assert!(
        js.contains("setTimeout"),
        "setTimeout missing — debounce not applied:\n{}",
        js
    );
    assert!(
        js.contains("clearTimeout"),
        "clearTimeout missing — debounce not applied:\n{}",
        js
    );
    assert!(
        js.contains("S.set('search'"),
        "state update missing in debounce handler:\n{}",
        js
    );
}

// ── v1.4.0 features ───────────────────────────────────────────────────────────

#[test]
fn golden_ref_attr_emits_data_ref() {
    let src = r#"
layout MainLayout { main { slot content } }
component Search {
    on:mount { refs.inp.focus(); }
    view { input ref:searchInput=true }
}
page "home" { Search {} }
"#;
    let doc = parse_webc(src).expect("parse");
    let res = generate_html(&doc, "home", &opts()).expect("codegen");
    assert!(
        res.html
            .contains(&format!("{}=\"searchInput\"", attr_names::REF)),
        "data-webcore-ref missing:\n{}",
        res.html
    );
    let js = generate_runtime_js(&res.handlers, &doc);
    assert!(
        js.contains("const refs={}"),
        "refs object missing in JS:\n{}",
        js
    );
    assert!(
        js.contains(attr_names::REF),
        "refs population code missing in JS:\n{}",
        js
    );
}

#[test]
fn golden_style_binding_emits_data_style() {
    let src = r#"
layout MainLayout { main { slot content } }
component Styled {
    state { color: String = "red" }
    view { div style:color={color} { "Text" } }
}
page "home" { Styled {} }
"#;
    let doc = parse_webc(src).expect("parse");
    let res = generate_html(&doc, "home", &opts()).expect("codegen");
    // v3: value is a compiled expression ID, not the raw expr name
    assert!(
        res.html
            .contains(&format!("{}color=\"", attr_names::STYLE_PREFIX)),
        "data-webcore-style-color missing:\n{}",
        res.html
    );
    let js = generate_runtime_js(&res.handlers, &doc);
    assert!(
        js.contains(attr_names::STYLE_PREFIX),
        "style binding JS code missing:\n{}",
        js
    );
}

#[test]
fn golden_slot_default_content_used_when_unfilled() {
    let src = r#"
app DashApp { layout: DashLayout }
layout DashLayout {
    aside { slot sidebar { p "Default nav" } }
    main  { slot content }
}
component App { view { p "Main" } }
page "dash" { App {} }
"#;
    let doc = parse_webc(src).expect("parse");
    let opts_dash = HtmlPageOptions {
        lang: "en".into(),
        title: "Test".into(),
        extra_css_files: vec![],
        critical_css: None,
        csp_meta: None,
        prod: false,
        source_maps: false,
        inline_runtime: true,
    };
    let res = generate_html(&doc, "dash", &opts_dash).expect("codegen");
    assert!(
        res.html.contains("Default nav"),
        "slot default content missing:\n{}",
        res.html
    );
}

#[test]
fn golden_transition_attr_emits_data_transition() {
    let src = r#"
layout MainLayout { main { slot content } }
component Modal {
    state { open: Boolean = false }
    view {
        button on:click={open = true} { "Open" }
        @if open {
            div webc:transition="fade" { "Modal content" }
        }
    }
}
page "home" { Modal {} }
"#;
    let doc = parse_webc(src).expect("parse");
    let res = generate_html(&doc, "home", &opts()).expect("codegen");
    assert!(
        res.html
            .contains(&format!("{}=\"fade\"", attr_names::TRANSITION)),
        "data-webcore-transition missing:\n{}",
        res.html
    );
    let js = generate_runtime_js(&res.handlers, &doc);
    assert!(
        js.contains("webc-fade-enter"),
        "transition CSS classes missing in JS:\n{}",
        js
    );
}

// ── v1.5.0 features ───────────────────────────────────────────────────────────

#[test]
fn golden_webc_img_injects_lazy_decoding() {
    let src = r#"
layout MainLayout { main { slot content } }
page "home" {
    img webc:img=true src="/photo.png" alt="Photo"
}
"#;
    let doc = parse_webc(src).expect("parse");
    let res = generate_html(&doc, "home", &opts()).expect("codegen");
    assert!(
        res.html.contains("loading=\"lazy\""),
        "loading=lazy missing:\n{}",
        res.html
    );
    assert!(
        res.html.contains("decoding=\"async\""),
        "decoding=async missing:\n{}",
        res.html
    );
    // webc:img should not appear as a real attribute in the output
    assert!(
        !res.html.contains("webc:img"),
        "webc:img directive leaked into output:\n{}",
        res.html
    );
}

#[test]
fn golden_webc_img_missing_alt_does_not_crash() {
    // img with webc:img but no alt — should build without panic,
    // a warning is emitted to stderr but HTML is still generated.
    let src = r#"
layout MainLayout { main { slot content } }
page "home" {
    img webc:img=true src="/photo.png"
}
"#;
    let doc = parse_webc(src).expect("parse");
    let res = generate_html(&doc, "home", &opts()).expect("codegen");
    // The img tag must still be present in the output
    assert!(
        res.html.contains("<img"),
        "img tag missing from output:\n{}",
        res.html
    );
    assert!(
        res.html.contains("loading=\"lazy\""),
        "loading=lazy missing in no-alt case:\n{}",
        res.html
    );
}

#[test]
fn unit_fnv1a_hash_stable() {
    // FNV-1a of b"hello" must produce a known stable 8-char hex string.
    // Manually computed: FNV-1a 32-bit of [104,101,108,108,111]
    //   2166136261 ^ 104 = 2166136225  * 16777619 = ...
    let hash = crate::cli::assets::fnv1a_hash(b"hello");
    assert_eq!(hash.len(), 8, "hash must be 8 hex chars");
    // Verify it's consistent (same input → same output)
    assert_eq!(
        hash,
        crate::cli::assets::fnv1a_hash(b"hello"),
        "hash must be deterministic"
    );
    // Different inputs must differ
    assert_ne!(
        hash,
        crate::cli::assets::fnv1a_hash(b"world"),
        "hash must depend on content"
    );
}

// ── Circular component reference detection ────────────────────────────────────

#[test]
fn check_circular_component_reference_detected() {
    // A uses B, B uses A — both exist so check_elements won't catch it
    let src = r#"
layout MainLayout { main { slot content } }
component A { view { B {} } }
component B { view { A {} } }
page "home" { A {} }
"#;
    let doc = parse_webc(src).expect("parse");
    // Verify the view trees contain the circular refs (structural check)
    let a = doc.components.get("A").expect("component A");
    assert!(a
        .view
        .iter()
        .any(|e| matches!(e, ast::Element::Component { name, .. } if name == "B")));
    let b = doc.components.get("B").expect("component B");
    assert!(b
        .view
        .iter()
        .any(|e| matches!(e, ast::Element::Component { name, .. } if name == "A")));
}

// ── v2.0.0 fine-grained signals ───────────────────────────────────────────────

#[test]
fn golden_signals_state_has_dep_tracking() {
    // A component with one state var and one @if directive should emit
    // the __wcfx dep-tracking mechanism and use $effect() for the binding,
    // without falling back to the old VARS.forEach(v=>S.on(...)) pattern.
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
    assert!(
        js.contains("__wcfx"),
        "__wcfx dep-tracking variable missing:\n{}",
        js
    );
    assert!(js.contains("$effect("), "$effect() call missing:\n{}", js);
    assert!(
        !js.contains("VARS.forEach(v=>S.on("),
        "old VARS.forEach subscription pattern should not be present:\n{}",
        js
    );
}

#[test]
fn golden_signals_early_exit_on_same_value() {
    // The State class must use Object.is for early-exit when the value is unchanged.
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
    assert!(
        js.contains("Object.is"),
        "Object.is early-exit missing from State.set():\n{}",
        js
    );
}

#[test]
fn golden_signals_no_subscription_sprawl() {
    // With multiple @if blocks, each should use $effect() individually
    // rather than subscribing every binding to every state variable.
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
    // Each @if binding uses $effect rather than subscribing to all vars
    assert!(
        js.contains("$effect(upd)"),
        "$effect(upd) missing — signals not used for @if bindings:\n{}",
        js
    );
    // Confirm no sprawl: old pattern absent
    assert!(
        !js.contains("VARS.forEach(v=>S.on("),
        "old subscription sprawl pattern must not appear:\n{}",
        js
    );
    // __wcfx must be present for dep tracking
    assert!(
        js.contains("__wcfx"),
        "__wcfx missing — signals dep tracking not emitted:\n{}",
        js
    );
}

// ── Error-path coverage ───────────────────────────────────────────────────────

#[test]
fn error_missing_layout_returns_err() {
    // No layout declared → generate_html should return Err
    let src = r#"page "home" { h1 "Hello" }"#;
    let doc = parse_webc(src).expect("parse");
    let result = generate_html(&doc, "home", &opts());
    assert!(result.is_err(), "expected error for missing layout");
    // Extract the error via match to avoid requiring Debug on HtmlGenerationResult
    let msg = match result {
        Err(e) => format!("{e}"),
        Ok(_) => panic!("expected Err"),
    };
    assert!(
        msg.to_lowercase().contains("layout"),
        "error should mention 'layout': {msg}"
    );
}

#[test]
fn error_missing_page_returns_err() {
    // Layout exists but requested page does not
    let src = r#"layout MainLayout { main { slot content } }"#;
    let doc = parse_webc(src).expect("parse");
    let result = generate_html(&doc, "home", &opts());
    assert!(result.is_err(), "expected error for missing page 'home'");
}

#[test]
fn empty_braces_parse_as_literal() {
    // `{}` is no longer interpolation syntax — it is literal text and must parse.
    let src = r#"page "home" { p "value: {}" }"#;
    assert!(
        parse_webc(src).is_ok(),
        "literal {{}} should parse as text, not error"
    );
}

#[test]
fn literal_braces_need_no_escaping() {
    // Unescaped braces in code-sample strings parse and render literally;
    // only `{ident}` with no leading space remains an interpolation.
    let src = r#"
layout MainLayout { main { slot content } }
component Demo {
    state { count: Number = 0 }
    view {
        pre "component App { state { x } }"
        p "padded { not interp } and {count}"
    }
}
page "home" { Demo {} }
"#;
    let doc = parse_webc(src).expect("literal braces should parse");
    let html = generate_html(&doc, "home", &opts()).expect("codegen").html;
    assert!(
        html.contains("component App { state { x } }"),
        "literal braces not rendered verbatim:\n{html}"
    );
    assert!(
        html.contains("padded { not interp } and"),
        "space-padded braces should stay literal:\n{html}"
    );
    // `{count}` (no leading space) is still a real interpolation span.
    assert!(
        html.contains(attr_names::INTERPOLATION),
        "{{count}} should still interpolate:\n{html}"
    );
}

// ============================================================
// Feature 3: Bundle analysis markers
// ============================================================

#[test]
fn bundle_analysis_detects_bindfor() {
    // A document with @for should emit `bindFor` in the runtime JS.
    let src = r#"
        page "home" {
            @for item in items {
                p "{item}"
            }
        }
    "#;
    let doc = parse_webc(src).expect("parse");
    let js = generate_runtime_js(&[], &doc);
    assert!(
        js.contains("bindFor"),
        "expected bindFor marker in JS when @for loop is used"
    );
}

#[test]
fn bundle_analysis_tree_shaken_when_unused() {
    // A document with no @for should NOT emit `bindFor`.
    let src = r#"
        page "home" {
            p "Hello"
        }
    "#;
    let doc = parse_webc(src).expect("parse");
    let js = generate_runtime_js(&[], &doc);
    assert!(
        !js.contains("bindFor"),
        "bindFor should be tree-shaken when @for is not used"
    );
}

// ============================================================
// Feature 1: Error aggregation (CompileErrors wrapper)
// ============================================================

#[test]
fn compile_errors_display_shows_count() {
    use crate::core::error::{CompileError, CompileErrors};
    let errors = CompileErrors(vec![
        CompileError::MissingPage {
            name: "home".into(),
        },
        CompileError::MissingComponent {
            name: "Counter".into(),
        },
    ]);
    let display = format!("{}", errors);
    assert!(
        display.contains("2 error(s) found."),
        "error count missing from display: {display}"
    );
}

#[test]
fn compile_errors_from_single_error() {
    use crate::core::error::{CompileError, CompileErrors};
    let single = CompileError::MissingLayout {
        name: "Main".into(),
        available: vec![],
    };
    let multi: CompileErrors = single.into();
    assert_eq!(multi.0.len(), 1);
    assert!(
        format!("{}", multi).contains("1 error(s) found."),
        "expected '1 error(s) found.' in display"
    );
}

// ============================================================
// Feature 2: CSS nesting — parse and flatten
// ============================================================

#[test]
fn golden_css_nesting_flattened() {
    // A component with a nested `&:hover` rule should produce a flat CSS rule.
    let src = r#"
        component Button {
            style {
                button {
                    color: blue;
                    &:hover {
                        color: darkblue;
                    }
                }
            }
        }
    "#;
    let doc = parse_webc(src).expect("parse");
    let css = generate_combined_css(None, &doc);

    // The base rule must be present
    assert!(
        css.contains("color: blue"),
        "base property missing from CSS:\n{css}"
    );
    // The nested rule must be flattened (no literal `&` in output)
    assert!(
        !css.contains('&'),
        "& should be replaced in flattened output:\n{css}"
    );
    // The flattened hover rule must be present
    assert!(
        css.contains(":hover") && css.contains("color: darkblue"),
        "flattened :hover rule missing from CSS:\n{css}"
    );
}

#[test]
fn golden_css_nesting_parse_and_roundtrip() {
    // Nested rules should parse without error and the nested selector is stored.
    let src = r#"
        component Card {
            style {
                .card {
                    background: white;
                    &:focus {
                        outline: 2px solid blue;
                    }
                    &::before {
                        content: "";
                    }
                }
            }
        }
    "#;
    let doc = parse_webc(src).expect("parse");
    // There should be exactly one component with one style rule containing 2 nested rules.
    let comp = doc.components.values().next().expect("component");
    let rule = match comp.style.first().expect("style item") {
        crate::core::ast::StyleItem::Rule(r) => r,
        _ => panic!("expected Rule"),
    };
    assert_eq!(
        rule.nested.len(),
        2,
        "expected 2 nested rules, got: {:?}",
        rule.nested
    );
    assert!(
        rule.nested[0].selector.starts_with("&:focus"),
        "first nested selector wrong"
    );
    assert!(
        rule.nested[1].selector.starts_with("&::before"),
        "second nested selector wrong"
    );

    let css = generate_combined_css(None, &doc);
    assert!(!css.contains('&'), "& leaked into output CSS:\n{css}");
    assert!(css.contains(":focus"), ":focus missing from output:\n{css}");
    assert!(
        css.contains("::before"),
        "::before missing from output:\n{css}"
    );
}

// ── v2.2.0 / v2.3.0 new tests ────────────────────────────────────────────────

#[test]
fn golden_keyframes_emitted_unscoped() {
    let src = r#"
component Spin {
    style {
        @keyframes spin {
            from { transform: rotate(0deg) }
            to { transform: rotate(360deg) }
        }
        .icon { animation: spin 1s linear infinite }
    }
    view { div class="icon" "X" }
}
layout MainLayout { main { slot content } }
page "home" { main { Spin {} } }
"#;
    let doc = parse_webc(src).expect("parse");
    let css = generate_combined_css(None, &doc);
    assert!(css.contains("@keyframes spin"), "keyframes emitted: {css}");
    assert!(css.contains("from"), "from step present");
    assert!(css.contains("to"), "to step present");
    // Must NOT be scoped (keyframes are global)
    assert!(
        !css.contains("@keyframes spin[data-v"),
        "keyframes must not be scoped"
    );
}

#[test]
fn golden_script_tag_has_defer() {
    let src = r#"
component Counter {
    state { count: Int = 0 }
    view { button on:click={count += 1} "{count}" }
}
layout MainLayout { main { slot content } }
page "home" { main { Counter {} } }
"#;
    let html = compile_to_html(src);
    assert!(html.contains("defer"), "script must have defer: {html}");
}

#[test]
fn golden_preload_hint_in_head() {
    // v3: JS is inlined per-page; no preload hint needed — check for inline <script> instead
    let src = r#"
component Counter {
    state { count: Int = 0 }
    view { button on:click={count += 1} "{count}" }
}
layout MainLayout { main { slot content } }
page "home" { main { Counter {} } }
"#;
    let html = compile_to_html(src);
    assert!(
        html.contains("<script"),
        "inline script missing for page with JS: {html}"
    );
}

#[test]
fn golden_unstyled_component_has_no_data_v() {
    let src = r#"
component NoStyle {
    view { span "hello" }
}
layout MainLayout { main { slot content } }
page "home" { main { NoStyle {} } }
"#;
    let html = compile_to_html(src);
    assert!(
        !html.contains("data-v="),
        "unstyled component should not emit data-v: {html}"
    );
}

#[test]
fn golden_styled_component_has_data_v() {
    let src = r#"
component WithStyle {
    style { .box { color: red } }
    view { div class="box" "hello" }
}
layout MainLayout { main { slot content } }
page "home" { main { WithStyle {} } }
"#;
    let html = compile_to_html(src);
    assert!(
        html.contains("data-v="),
        "styled component must emit data-v: {html}"
    );
}

#[test]
fn golden_zero_js_page_has_no_script() {
    let src = r#"
layout MainLayout { main { slot content } }
page "home" {
    main {
        h1 "Hello World"
        p "Static content only"
    }
}
"#;
    let html = compile_to_html(src);
    assert!(
        !html.contains("<script"),
        "static page must not have script tag: {html}"
    );
    assert!(
        !html.contains("preload"),
        "static page must not have preload hint: {html}"
    );
}

#[test]
fn golden_nesting_bomb_rejected() {
    // 150 levels of nesting — should fail
    let mut src = "layout MainLayout { main { slot content } }\npage \"home\" {\n".to_string();
    for _ in 0..150 {
        src.push_str("div {\n");
    }
    src.push_str("\"deep\"\n");
    for _ in 0..150 {
        src.push_str("}\n");
    }
    src.push_str("}\n");
    let result = parse_webc(&src);
    assert!(result.is_err(), "deeply nested content should be rejected");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("depth") || err.contains("nesting"),
        "error should mention depth: {err}"
    );
}

#[test]
fn golden_nesting_within_limit_ok() {
    let mut src = "layout MainLayout { main { slot content } }\npage \"home\" {\n".to_string();
    for _ in 0..50 {
        src.push_str("div {\n");
    }
    src.push_str("\"ok\"\n");
    for _ in 0..50 {
        src.push_str("}\n");
    }
    src.push_str("}\n");
    let result = parse_webc(&src);
    assert!(result.is_ok(), "50 levels of nesting should be fine");
}

// ── v2.1.0 features ───────────────────────────────────────────────────────────

#[test]
fn golden_watch_emits_s_on() {
    let src = r#"
component Counter {
    state { count: Int = 0 }
    $watch count => { console.log(count) }
    view { p "{count}" }
}
layout MainLayout { main { slot content } }
page "home" { main { Counter {} } }
"#;
    let js = compile_to_js(src);
    assert!(js.contains("S.on('count'"), "watch should emit S.on: {js}");
    assert!(
        js.contains("console.log"),
        "watch body should be included: {js}"
    );
}

#[test]
fn golden_onclick_literal_object_parses() {
    let src = r#"
component Foo {
    state { x: Int = 0 }
    view { button on:click={x = {val: 1}.val} "click" }
}
layout MainLayout { main { slot content } }
page "home" { main { Foo {} } }
"#;
    // Should parse without error
    let doc = parse_webc(src).expect("on:click with literal object should parse");
    assert!(doc.components.contains_key("Foo"));
}

#[test]
fn golden_for_key_braced_expression() {
    let src = r#"
component List {
    state { items: Array = null }
    view {
        @for item key={item} in items {
            li "{item}"
        }
    }
}
layout MainLayout { main { slot content } }
page "home" { main { List {} } }
"#;
    let html = compile_to_html(src);
    assert!(
        html.contains("data-webcore-for-key"),
        "for key should emit data attr: {html}"
    );
}

#[test]
fn golden_ssg_length_expression() {
    let src = r#"
component Items {
    state { items: Array = null }
    view { p "Count: {items.length}" }
}
layout MainLayout { main { slot content } }
page "home" { main { Items {} } }
"#;
    // Should parse without error
    let doc = parse_webc(src).expect("parse");
    assert!(doc.components.contains_key("Items"));
}

#[test]
fn golden_prop_validation_warns_unknown() {
    // This is a compile-time warning test — no assert on output,
    // just verify it doesn't panic/error
    let src = r#"
component Box {
    props { color: String }
    view { div class={color} "box" }
}
layout MainLayout { main { slot content } }
page "home" { main { Box color="red" unknown="bad" {} } }
"#;
    let doc = parse_webc(src).expect("parse");
    // The warning is emitted on stderr — just verify codegen doesn't crash
    let _ = generate_html(&doc, "home", &opts());
}

// ═══ v2.4.0 — Critical CSS inline ═══════════════════════════════════════════

#[test]
fn golden_critical_css_inlined_in_shell() {
    let src = r#"
component Card {
    view { div class="card" "hello" }
    style { .card { padding: 1rem; } }
}
layout MainLayout { main { slot content } }
page "home" { main { Card {} } }
"#;
    let doc = parse_webc(src).expect("parse");
    let options = HtmlPageOptions {
        lang: "en".into(),
        title: "Test".into(),
        extra_css_files: vec![],
        critical_css: Some(".card{padding:1rem}".into()),
        csp_meta: None,
        prod: false,
        source_maps: false,
        inline_runtime: true,
    };
    let res = generate_html(&doc, "home", &options).expect("codegen");
    assert!(
        res.html.contains("<style>.card{padding:1rem}</style>"),
        "critical CSS should be inlined in <style>: {}",
        res.html
    );
    assert!(
        res.html.contains(r#"media="print" data-webcore-defer"#),
        "full stylesheet should load deferred via data-webcore-defer: {}",
        res.html
    );
    assert!(
        res.html
            .contains("<noscript><link rel=\"stylesheet\" href=\"/assets/theme.css\"></noscript>"),
        "noscript fallback expected: {}",
        res.html
    );
    assert!(
        !res.html
            .contains("<link rel=\"stylesheet\" href=\"/assets/theme.css\">\n"),
        "blocking stylesheet link should be absent when critical CSS is inlined: {}",
        res.html
    );
}

#[test]
fn golden_no_critical_css_in_dev() {
    let html = compile_to_html(
        r#"
layout MainLayout { main { slot content } }
page "home" { p "hi" }
"#,
    );
    assert!(
        html.contains("<link rel=\"stylesheet\" href=\"/assets/theme.css\">"),
        "dev mode keeps the blocking stylesheet link: {html}"
    );
    assert!(
        !html.contains("media=\"print\""),
        "no deferred swap in dev: {html}"
    );
}

#[test]
fn golden_collect_page_components_recursive() {
    let src = r#"
component Inner {
    view { span "deep" }
    style { span { color: red; } }
}
component Outer {
    view { div { Inner {} } }
    style { div { padding: 1rem; } }
}
layout MainLayout { main { slot content } }
page "home" { main { Outer {} } }
"#;
    let doc = parse_webc(src).expect("parse");
    let used = crate::codegen::html::collect_page_components(&doc, "home");
    assert!(
        used.contains("Outer"),
        "Outer should be collected: {used:?}"
    );
    assert!(
        used.contains("Inner"),
        "nested Inner should be collected: {used:?}"
    );
}

// ═══ v2.4.0 — SSG collections ═══════════════════════════════════════════════

#[test]
fn golden_route_each_collection_parses() {
    let src = r#"
app Blog {
    layout: MainLayout
    routes {
        "/": HomePage
        "/post/:slug": PostPage each posts
    }
}
"#;
    let doc = parse_webc(src).expect("parse route with each");
    let app = doc.app.expect("app");
    assert_eq!(app.routes.get("/post/:slug"), Some(&"PostPage".to_string()));
    assert_eq!(
        app.collections.get("/post/:slug"),
        Some(&"posts".to_string())
    );
    assert!(
        !app.collections.contains_key("/"),
        "non-collection routes unaffected"
    );
}

#[test]
fn golden_expand_collection_basic() {
    let items = r#"[{"slug":"hello-world","title":"Hello"},{"slug":"second-post","title":"Two"}]"#;
    let entries = crate::cli::loader::expand_collection("/post/:slug", items).expect("expand");
    assert_eq!(entries.len(), 2);
    assert_eq!(
        entries[0],
        (
            "post/hello-world/index.html".to_string(),
            "slug".to_string(),
            "hello-world".to_string()
        )
    );
    assert_eq!(entries[1].0, "post/second-post/index.html");
}

#[test]
fn golden_expand_collection_numeric_param() {
    let items = r#"[{"id":1},{"id":42}]"#;
    let entries = crate::cli::loader::expand_collection("/user/:id", items).expect("expand");
    assert_eq!(entries[0].0, "user/1/index.html");
    assert_eq!(entries[1].0, "user/42/index.html");
}

#[test]
fn golden_expand_collection_rejects_traversal() {
    let items = r#"[{"slug":"../../etc"}]"#;
    let err = crate::cli::loader::expand_collection("/post/:slug", items).unwrap_err();
    assert!(
        err.contains("unsafe"),
        "traversal value must be rejected: {err}"
    );

    let items = r#"[{"slug":"a/b"}]"#;
    assert!(crate::cli::loader::expand_collection("/post/:slug", items).is_err());

    let items = r#"[{"slug":""}]"#;
    assert!(crate::cli::loader::expand_collection("/post/:slug", items).is_err());
}

#[test]
fn golden_expand_collection_missing_field() {
    let items = r#"[{"title":"no slug here"}]"#;
    let err = crate::cli::loader::expand_collection("/post/:slug", items).unwrap_err();
    assert!(err.contains("missing field 'slug'"), "{err}");
}

#[test]
fn golden_expand_collection_requires_param_route() {
    let err = crate::cli::loader::expand_collection("/posts", "[]").unwrap_err();
    assert!(err.contains("no ':param'"), "{err}");
}

#[test]
fn golden_ssg_prerenders_route_param() {
    let src = r#"
layout MainLayout { main { slot content } }
page "home" { h1 "{$route.slug}" }
"#;
    let doc = parse_webc(src).expect("parse");
    let mut state: BTreeMap<String, String> = BTreeMap::new();
    state.insert("$route.slug".to_string(), "hello-world".to_string());
    let locales = BTreeMap::new();
    let ssg_ctx = crate::core::ssg::SsgContext {
        state: &state,
        locales: &locales,
        locale: "fr",
    };
    let out = crate::codegen::html::generate_page(&doc, "home", &opts(), None, Some(&ssg_ctx))
        .expect("codegen")
        .html;
    assert!(
        out.contains(">hello-world</span>"),
        "route param should be pre-rendered: {out}"
    );
}

// ═══ v2.5.0 — CSP stricte & event delegation ═════════════════════════════════

#[test]
fn golden_csp_meta_emitted_when_set() {
    let src = r#"
layout MainLayout { main { slot content } }
page "home" { p "hello" }
"#;
    let doc = parse_webc(src).expect("parse");
    let options = HtmlPageOptions {
        lang: "en".into(),
        title: "Test".into(),
        extra_css_files: vec![],
        critical_css: None,
        csp_meta: Some("default-src 'self'; script-src 'self'".into()),
        prod: false,
        source_maps: false,
        inline_runtime: true,
    };
    let res = generate_html(&doc, "home", &options).expect("codegen");
    assert!(
        res.html.contains(r#"http-equiv="Content-Security-Policy""#),
        "CSP meta tag missing:\n{}",
        res.html
    );
    assert!(
        res.html.contains("script-src &#x27;self&#x27;"),
        "CSP value missing (should be HTML-escaped):\n{}",
        res.html
    );
}

#[test]
fn golden_event_delegation_no_inline_onclick() {
    let src = r#"
layout MainLayout { main { slot content } }
page "home" {
    button on:click={count += 1} { "+" }
    form on:submit={doSubmit()} { input type="text" name="q" }
}
"#;
    let doc = parse_webc(src).expect("parse");
    let res = generate_html(&doc, "home", &opts()).expect("codegen");
    // No inline event handlers
    assert!(
        !res.html.contains("onclick="),
        "onclick= should be absent (using delegation):\n{}",
        res.html
    );
    assert!(
        !res.html.contains("onsubmit="),
        "onsubmit= should be absent (using delegation):\n{}",
        res.html
    );
    // data-webcore-e attributes present
    assert!(
        res.html.contains("data-webcore-e=\"click\""),
        "data-webcore-e click attribute missing:\n{}",
        res.html
    );
    assert!(
        res.html.contains("data-webcore-e=\"submit\""),
        "data-webcore-e submit attribute missing:\n{}",
        res.html
    );
    // JS should emit D() delegation setup
    let js = generate_runtime_js(&res.handlers, &doc);
    assert!(
        js.contains("const D="),
        "delegation function D() missing in JS:\n{}",
        js
    );
    assert!(
        js.contains("D('click',1)"),
        "D('click') delegation call missing:\n{}",
        js
    );
}

#[test]
fn golden_spa_link_uses_data_webcore_nav() {
    let src = r#"
app MyApp {
    routes { "/": HomePage "/about": AboutPage }
}
layout MainLayout { main { slot content } }
page "home" { link to="/about" { "About" } }
"#;
    let doc = parse_webc(src).expect("parse");
    let res = generate_html(&doc, "home", &opts()).expect("codegen");
    assert!(
        res.html.contains("data-webcore-nav"),
        "data-webcore-nav attribute missing on SPA link:\n{}",
        res.html
    );
    assert!(
        !res.html.contains("onclick=\"webcore_navigate"),
        "inline onclick navigation should be absent:\n{}",
        res.html
    );
    // JS should set up delegation for data-webcore-nav links
    let js = generate_runtime_js(&res.handlers, &doc);
    assert!(
        js.contains("a[data-webcore-nav]"),
        "data-webcore-nav delegation missing in JS:\n{}",
        js
    );
}

#[test]
fn golden_css_defer_swap_in_domcontentloaded() {
    let src = r#"
layout MainLayout { main { slot content } }
page "home" { p "hi" }
"#;
    let doc = parse_webc(src).expect("parse");
    let options = HtmlPageOptions {
        lang: "en".into(),
        title: "Test".into(),
        extra_css_files: vec![],
        critical_css: Some(".p{color:red}".into()),
        csp_meta: None,
        prod: false,
        source_maps: false,
        inline_runtime: true,
    };
    let res = generate_html(&doc, "home", &options).expect("codegen");
    // Deferred link uses data-webcore-defer (not onload=)
    assert!(
        res.html.contains("data-webcore-defer"),
        "data-webcore-defer attribute missing on deferred CSS link:\n{}",
        res.html
    );
    assert!(
        !res.html.contains("onload="),
        "onload= should be absent (CSP-unsafe):\n{}",
        res.html
    );
    // JS should swap media to 'all' in DOMContentLoaded
    let js = generate_runtime_js(&res.handlers, &doc);
    assert!(
        js.contains("data-webcore-defer"),
        "css defer swap missing in JS DOMContentLoaded:\n{}",
        js
    );
}

// ── Fix 1: zero-JS page with critical_css must still include webcore.js ─────

#[test]
fn golden_critical_css_on_static_page_includes_script() {
    // A page with no handlers/state would normally skip webcore.js.
    // But critical_css injects a deferred <link> whose media swap needs JS,
    // so webcore.js must be present even on otherwise zero-JS pages.
    let src = r#"
layout MainLayout { main { slot content } }
page "home" { main { h1 "Static" } }
"#;
    let doc = parse_webc(src).expect("parse");
    let options = HtmlPageOptions {
        lang: "en".into(),
        title: "Test".into(),
        extra_css_files: vec![],
        critical_css: Some("h1{color:red}".into()),
        csp_meta: None,
        prod: false,
        source_maps: false,
        inline_runtime: true,
    };
    let res = generate_html(&doc, "home", &options).expect("codegen");
    // v3: JS is inlined per-page; the DOMContentLoaded handler swaps media="print"→"all"
    assert!(
        res.html.contains("<script"),
        "inline script must be present when critical_css is set (defer swap needs JS):\n{}",
        res.html
    );
}

// ── Fix 2: component with only event handlers triggers needs_js ───────────

#[test]
fn golden_component_with_only_event_handler_includes_script() {
    // A component that has no state/computed but does have an on:click handler
    // must still pull in webcore.js. Previously document_needs_js() would miss this.
    let src = r#"
component Button {
    view { button on:click="doThing" "Click" }
}
layout MainLayout { main { slot content } }
page "home" { main { Button {} } }
"#;
    let html = compile_to_html(src);
    assert!(
        html.contains("<script"),
        "page using a component with on:click must include webcore.js:\n{html}"
    );
}

// ── Fix 3: CSS injection via </style> in critical CSS is escaped ──────────

#[test]
fn golden_critical_css_style_tag_injection_escaped() {
    let src = r#"
layout MainLayout { main { slot content } }
page "home" { main { p "hi" } }
"#;
    let doc = parse_webc(src).expect("parse");
    let options = HtmlPageOptions {
        lang: "en".into(),
        title: "Test".into(),
        extra_css_files: vec![],
        // Adversarial CSS that attempts to break out of the <style> block.
        critical_css: Some("a{content:\"</style><script>alert(1)</script>\"}".into()),
        csp_meta: None,
        prod: false,
        source_maps: false,
        inline_runtime: true,
    };
    let res = generate_html(&doc, "home", &options).expect("codegen");
    assert!(
        !res.html.contains("</style><script>"),
        "</style> must be escaped in inlined critical CSS:\n{}",
        res.html
    );
    assert!(
        res.html.contains("<\\/style>"),
        "escaped form <\\/style> should be present:\n{}",
        res.html
    );
}

// ── Fix 5: .length on array with quoted commas uses JSON parser ────────────

#[test]
fn golden_ssg_array_length_with_quoted_commas() {
    use crate::core::ssg::eval_expr_with_locale;
    use std::collections::BTreeMap;
    let mut state = BTreeMap::new();
    // Value contains a comma inside a quoted string — naive split gives 3, correct is 2.
    state.insert("items".to_string(), r#"["a,b","c"]"#.to_string());
    let result = eval_expr_with_locale("items.length", &state, &BTreeMap::new(), "en");
    assert_eq!(
        result,
        Some("2".to_string()),
        "array length should be 2 (quoted comma must not be counted as separator)"
    );
}

// ── Fix 6: .length on a Unicode string counts chars not bytes ─────────────

#[test]
fn golden_ssg_string_length_unicode() {
    use crate::core::ssg::eval_expr_with_locale;
    use std::collections::BTreeMap;
    let mut state = BTreeMap::new();
    // "café" = 4 chars, 5 UTF-8 bytes (é is 2 bytes).
    state.insert("name".to_string(), "café".to_string());
    let result = eval_expr_with_locale("name.length", &state, &BTreeMap::new(), "en");
    assert_eq!(
        result,
        Some("4".to_string()),
        "string .length should be char count (4), not byte count (5)"
    );
}

// ── v2.6.0 features ───────────────────────────────────────────────────────────

#[test]
fn golden_fragment_renders_children_without_wrapper() {
    let html = compile_to_html(
        r#"
layout MainLayout { main { slot content } }
page "home" {
    <>
        h1 "Title"
        p "Body"
    </>
}
"#,
    );
    assert!(
        html.contains("<h1>Title"),
        "h1 missing in fragment output:\n{}",
        html
    );
    assert!(
        html.contains("<p>Body"),
        "p missing in fragment output:\n{}",
        html
    );
    // Fragment must NOT introduce a wrapper tag
    assert!(
        !html.contains("<fragment"),
        "fragment wrapper tag must not appear:\n{}",
        html
    );
    assert!(
        !html.contains("<>"),
        "raw <> must not appear in output:\n{}",
        html
    );
}

#[test]
fn golden_fragment_in_component_view() {
    let html = compile_to_html(
        r#"
layout MainLayout { main { slot content } }
component Pair {
    view {
        <>
            span "A"
            span "B"
        </>
    }
}
page "home" { Pair {} }
"#,
    );
    assert!(html.contains(">A"), "span A missing:\n{}", html);
    assert!(html.contains(">B"), "span B missing:\n{}", html);
    assert!(
        !html.contains("<fragment"),
        "no wrapper tag expected:\n{}",
        html
    );
}

#[test]
fn golden_default_prop_used_when_not_supplied() {
    let html = compile_to_html(
        r#"
layout MainLayout { main { slot content } }
component Badge {
    props { label: String = "Default" }
    view { span "{label}" }
}
page "home" { Badge {} }
"#,
    );
    // No label prop supplied → should use the default "Default"
    assert!(
        html.contains("Default"),
        "default prop value not rendered:\n{}",
        html
    );
    assert!(
        !html.contains(&format!("{}=\"label\"", attr_names::INTERPOLATION)),
        "unresolved label span should not appear:\n{}",
        html
    );
}

#[test]
fn golden_default_prop_overridden_by_caller() {
    let html = compile_to_html(
        r#"
layout MainLayout { main { slot content } }
component Badge {
    props { label: String = "Default" }
    view { span "{label}" }
}
page "home" { Badge label="Custom" {} }
"#,
    );
    assert!(
        html.contains("Custom"),
        "caller-supplied prop value missing:\n{}",
        html
    );
    assert!(
        !html.contains("Default"),
        "default value should be overridden:\n{}",
        html
    );
}

#[test]
fn golden_event_modifier_stop_encoded_in_data_attr() {
    let src = r#"
layout MainLayout { main { slot content } }
page "home" {
    button on:click|stop={doThing()} { "Stop" }
}
"#;
    let doc = parse_webc(src).expect("parse");
    let res = generate_html(&doc, "home", &opts()).expect("codegen");
    assert!(
        res.html.contains("data-webcore-e=\"click|stop\""),
        "stop modifier not encoded in data-webcore-e:\n{}",
        res.html
    );
    // Handler must still be registered with base event type
    assert!(
        res.handlers.iter().any(|h| h.event_type == "click"),
        "handler event_type should be base 'click': {:?}",
        res.handlers
    );
}

#[test]
fn golden_event_modifier_prevent_encoded_in_data_attr() {
    let src = r#"
layout MainLayout { main { slot content } }
page "home" {
    form on:submit|prevent={handleSubmit()} { input }
}
"#;
    let doc = parse_webc(src).expect("parse");
    let res = generate_html(&doc, "home", &opts()).expect("codegen");
    assert!(
        res.html.contains("data-webcore-e=\"submit|prevent\""),
        "prevent modifier not encoded in data-webcore-e:\n{}",
        res.html
    );
}

#[test]
fn golden_event_modifier_updates_d_function() {
    let src = r#"
layout MainLayout { main { slot content } }
page "home" {
    button on:click|stop={count += 1} { "Stop" }
}
"#;
    let doc = parse_webc(src).expect("parse");
    let res = generate_html(&doc, "home", &opts()).expect("codegen");
    let js = generate_runtime_js(&res.handlers, &doc);
    // D() must check data-webcore-e for modifiers (startsWith check)
    assert!(
        js.contains("startsWith(t+'|')"),
        "D() must use startsWith for modifier matching:\n{}",
        js
    );
    assert!(
        js.contains("mods.includes('stop')"),
        "D() must handle stop modifier:\n{}",
        js
    );
}

#[test]
fn golden_event_multiple_modifiers() {
    let src = r#"
layout MainLayout { main { slot content } }
page "home" {
    button on:click|stop|prevent={action()} { "Click" }
}
"#;
    let doc = parse_webc(src).expect("parse");
    let res = generate_html(&doc, "home", &opts()).expect("codegen");
    assert!(
        res.html.contains("data-webcore-e=\"click|stop|prevent\""),
        "multiple modifiers not encoded:\n{}",
        res.html
    );
    // Only one D('click') call should be emitted (not one per modifier combination)
    let js = generate_runtime_js(&res.handlers, &doc);
    let d_click_count = js.matches("D('click',").count();
    assert_eq!(
        d_click_count, 1,
        "should emit exactly one D('click') call: {js}"
    );
}

// ── @for nested scope ─────────────────────────────────────────────────────

#[test]
fn golden_nested_for_generates_inner_template_with_outer_var_interpolation() {
    // Verifies that nested @for loops produce correct HTML: inner template has
    // data-webcore-for-in referencing the outer var's property, and the inner
    // template content has interpolation spans for both inner and outer vars.
    let (html, _js) = compile_full(
        r#"
layout MainLayout { main { slot content } }
component NestedFor {
    state { sections: List = null }
    view {
        @for section in sections {
            div {
                h2 "{section.title}"
                @for item in section.items {
                    span "{item} — {section.title}"
                }
            }
        }
    }
}
page "home" { NestedFor {} }
"#,
    );
    // Outer template
    assert!(
        html.contains("data-webcore-for=\"section\""),
        "outer for var missing:\n{html}"
    );
    assert!(
        html.contains("data-webcore-in=\"sections\""),
        "outer for-in missing:\n{html}"
    );
    // Inner template nested inside the outer
    assert!(
        html.contains("data-webcore-for=\"item\""),
        "inner for var missing:\n{html}"
    );
    assert!(
        html.contains("data-webcore-in=\"section.items\""),
        "inner for-in missing:\n{html}"
    );
    // Both vars have interpolation spans in the inner template content
    // v3: values are compiled expression IDs, not raw exprs — just check attr presence
    let interp = attr_names::INTERPOLATION;
    let count = html.matches(&format!("{interp}=\"")).count();
    assert!(
        count >= 2,
        "expected ≥2 interpolation spans (item + section.title), got {count}:\n{html}"
    );
}

#[test]
fn golden_nested_for_bindffor_emits_context_passing_runtime() {
    // Verifies that the JS runtime includes the new bindFor signature and
    // context-passing machinery for nested @for support.
    let (_html, js) = compile_full(
        r#"
layout MainLayout { main { slot content } }
component NestedFor {
    state { items: List = null }
    view {
        @for outer in items {
            @for inner in outer.children {
                p "{inner} in {outer.name}"
            }
        }
    }
}
page "home" { NestedFor {} }
"#,
    );
    assert!(
        js.contains("bindFor=(root=document)"),
        "bindFor should accept root parameter:\n{js}"
    );
    assert!(
        js.contains("_wc_ctx"),
        "context propagation (_wc_ctx) missing from bindFor:\n{js}"
    );
    assert!(
        js.contains("isConnected"),
        "isConnected guard missing from bindFor:\n{js}"
    );
    assert!(
        js.contains("bindFor(cont)"),
        "recursive bindFor(cont) call missing:\n{js}"
    );
}

// ── webc fmt ──────────────────────────────────────────────────────────────────

#[test]
fn fmt_roundtrip_simple_component() {
    use crate::cli::fmt::{format_webc, FmtOptions};
    let source = r#"layout MainLayout { main { slot content } }
component Counter {
    state {
        count: Number = 0
    }
    view {
        div {
            p "{count}"
            button on:click={count += 1} { "+" }
        }
    }
}
page "home" { Counter {} }
"#;
    let opts = FmtOptions::default();
    let formatted = format_webc(source, &opts).expect("format failed");
    // Formatted output must re-parse and compile to the same HTML
    let (html_orig, _) = compile_full(source);
    let (html_fmt, _) = compile_full(&formatted);
    assert_eq!(
        html_orig, html_fmt,
        "round-trip: formatted source produces different HTML"
    );
}

#[test]
fn fmt_roundtrip_page_with_for_and_if() {
    use crate::cli::fmt::{format_webc, FmtOptions};
    let source = r#"layout MainLayout { main { slot content } }
component ListComp {
    state {
        items: List = null
        show: Boolean = true
    }
    view {
        @if show {
            @for item in items {
                li "{item}"
            }
        }
    }
}
page "home" { ListComp {} }
"#;
    let opts = FmtOptions::default();
    let formatted = format_webc(source, &opts).expect("format failed");
    let (html_orig, _) = compile_full(source);
    let (html_fmt, _) = compile_full(&formatted);
    assert_eq!(
        html_orig, html_fmt,
        "round-trip: formatted source produces different HTML"
    );
}

#[test]
fn fmt_idempotent_already_formatted() {
    use crate::cli::fmt::{format_webc, FmtOptions};
    // A source that is already formatted; formatting it again must produce the same output.
    let source = r#"component Badge {
    props {
        label: String = "Default"
    }
    view {
        span class="badge" { "{label}" }
    }
    style {
        .badge { padding: 2px 6px; border-radius: 4px; }
    }
}
"#;
    let opts = FmtOptions::default();
    let first = format_webc(source, &opts).expect("first format failed");
    let second = format_webc(&first, &opts).expect("second format failed");
    assert_eq!(first, second, "formatter is not idempotent");
}

#[test]
fn golden_void_elements_have_no_closing_tag() {
    let src = r#"
layout MainLayout { main { slot content } }
page "home" {
    input type="text" placeholder="Nom"
    img src="/a.png" alt="A"
    br
    hr
}
"#;
    let doc = parse_webc(src).expect("parse");
    let res = generate_html(&doc, "home", &opts()).expect("codegen");
    for tag in ["</input>", "</img>", "</br>", "</hr>"] {
        assert!(
            !res.html.contains(tag),
            "void element closing tag {tag} emitted:\n{}",
            res.html
        );
    }
    assert!(res.html.contains("<input"), "input missing:\n{}", res.html);
}

#[test]
fn golden_inline_text_has_no_trailing_newline() {
    let src = r#"
layout MainLayout { main { slot content } }
page "home" {
    span "abc"
}
"#;
    let doc = parse_webc(src).expect("parse");
    let res = generate_html(&doc, "home", &opts()).expect("codegen");
    assert!(
        res.html.contains("<span>abc</span>"),
        "trailing whitespace inside inline element:\n{}",
        res.html
    );
}

// ═══ i18n — comportement runtime de t() (exécuté via node) ══════════════════

/// Execute the emitted `t()` against real locale data with Node.js and assert
/// plural selection (_one/_other), fallbacks (missing plural form, missing
/// key) and parameter substitution ({{count}}, {{0}}).
#[test]
fn golden_i18n_t_runtime_behaviour() {
    if std::process::Command::new("node")
        .arg("--version")
        .output()
        .is_err()
    {
        eprintln!("note: node not found, skipping t() behaviour test");
        return;
    }

    let src = r#"
layout MainLayout { main { slot content } }
page "home" { p "x" }
"#;
    let mut doc = parse_webc(src).expect("parse");
    let mut fr: BTreeMap<String, String> = BTreeMap::new();
    fr.insert("greeting".into(), "Bonjour {{0}}".into());
    fr.insert("items_one".into(), "{{count}} objet".into());
    fr.insert("items_other".into(), "{{count}} objets".into());
    fr.insert("only_base".into(), "Total : {{count}}".into());
    doc.locales.insert("fr".into(), fr);
    doc.default_locale = "fr".into();

    let js = generate_runtime_js(&[], &doc);
    // Extract the three self-contained i18n statements from the runtime.
    let mut script = String::new();
    for needle in ["const LOCALES=", "let LOCALE=", "const t="] {
        let line = js
            .lines()
            .find(|l| l.starts_with(needle))
            .unwrap_or_else(|| panic!("{needle} not emitted:\n{js}"));
        script.push_str(line);
        script.push('\n');
    }
    script.push_str(
        r#"
const eq=(got,want,msg)=>{if(got!==want){console.error(msg+': got '+JSON.stringify(got)+', want '+JSON.stringify(want));process.exit(1);}};
eq(t('greeting','Ana'),'Bonjour Ana','positional {{0}} substitution');
eq(t('greeting'),'Bonjour {{0}}','no-arg returns raw template');
eq(t('items',1),'1 objet','plural _one');
eq(t('items',3),'3 objets','plural _other');
eq(t('items',0),'0 objets','plural zero uses _other');
eq(t('only_base',5),'Total : 5','plural falls back to base key');
eq(t('noplural',2),'noplural','plural with no entry falls back to key');
eq(t('missing'),'missing','missing key falls back to key');
"#,
    );

    let path = std::env::temp_dir().join(format!("webcore-t-test-{}.js", std::process::id()));
    std::fs::write(&path, &script).expect("write t() test script");
    let out = std::process::Command::new("node")
        .arg(&path)
        .output()
        .expect("run node");
    std::fs::remove_file(&path).ok();
    assert!(
        out.status.success(),
        "t() behaviour mismatch:\n{}\n--- script ---\n{script}",
        String::from_utf8_lossy(&out.stderr)
    );
}

// ═══ v2.8.0 — Axe 2 : méthodes réactives sur List ════════════════════════════

#[test]
fn golden_list_push_compiles_to_spread_append() {
    use crate::codegen::js::js_events::{compile_list_method, CompiledVars};
    use std::collections::HashSet;
    // Both items and draft are known state vars
    let vars: HashSet<String> = ["items", "draft"].iter().map(|s| s.to_string()).collect();
    let cv = CompiledVars::new(&vars);
    let result = compile_list_method("items.push(draft)", &cv).expect("should match");
    assert_eq!(
        result, "S.set('items',[...S.get('items'),S.get('draft')])",
        "push compilation mismatch"
    );
}

#[test]
fn golden_list_push_with_literal_compiles() {
    use crate::codegen::js::js_events::{compile_list_method, CompiledVars};
    use std::collections::HashSet;
    let vars: HashSet<String> = ["todos"].iter().map(|s| s.to_string()).collect();
    let cv = CompiledVars::new(&vars);
    let result = compile_list_method("todos.push(\"buy milk\")", &cv).expect("should match");
    assert_eq!(
        result, "S.set('todos',[...S.get('todos'),\"buy milk\"])",
        "push with literal mismatch"
    );
}

#[test]
fn golden_list_remove_compiles_to_filter() {
    use crate::codegen::js::js_events::{compile_list_method, CompiledVars};
    use std::collections::HashSet;
    let vars: HashSet<String> = ["items"].iter().map(|s| s.to_string()).collect();
    let cv = CompiledVars::new(&vars);
    let result = compile_list_method("items.remove(0)", &cv).expect("should match");
    assert_eq!(
        result, "S.set('items',S.get('items').filter((_,_i)=>_i!==(0)))",
        "remove compilation mismatch"
    );
}

#[test]
fn golden_list_remove_with_index_var() {
    use crate::codegen::js::js_events::{compile_list_method, CompiledVars};
    use std::collections::HashSet;
    let vars: HashSet<String> = ["items", "idx"].iter().map(|s| s.to_string()).collect();
    let cv = CompiledVars::new(&vars);
    let result = compile_list_method("items.remove(idx)", &cv).expect("should match");
    assert!(
        result.contains("S.get('idx')"),
        "index variable should be replaced: {result}"
    );
    assert!(
        result.starts_with("S.set('items',"),
        "result should set items: {result}"
    );
}

#[test]
fn golden_list_clear_compiles_to_empty_array() {
    use crate::codegen::js::js_events::{compile_list_method, CompiledVars};
    use std::collections::HashSet;
    let vars: HashSet<String> = ["items"].iter().map(|s| s.to_string()).collect();
    let cv = CompiledVars::new(&vars);
    let result = compile_list_method("items.clear()", &cv).expect("should match");
    assert_eq!(result, "S.set('items',[])", "clear compilation mismatch");
}

#[test]
fn golden_list_method_ignored_for_unknown_var() {
    use crate::codegen::js::js_events::{compile_list_method, CompiledVars};
    use std::collections::HashSet;
    let vars: HashSet<String> = HashSet::new();
    let cv = CompiledVars::new(&vars);
    // unknown var → should return None (fall through to general expression path)
    assert!(
        compile_list_method("unknownVar.push(x)", &cv).is_none(),
        "should return None for unknown var"
    );
}

#[test]
fn golden_store_list_push_compiles_correctly() {
    use crate::codegen::js::js_events::{compile_list_method, CompiledVars};
    use std::collections::HashSet;
    let vars: HashSet<String> = HashSet::new();
    let cv = CompiledVars::new(&vars);
    // item is not in vars → passes through as bare identifier
    let result = compile_list_method("$store.cart.push(item)", &cv).expect("should match");
    assert!(
        result.starts_with("STORE.set('cart',"),
        "store push should use STORE.set: {result}"
    );
    assert!(
        result.contains("STORE.get('cart')"),
        "store push should read STORE.get: {result}"
    );
    let result2 = compile_list_method("$store.cart.push(\"apple\")", &cv).expect("should match");
    assert!(
        result2.starts_with("STORE.set('cart',"),
        "store push with literal should use STORE.set: {result2}"
    );
    assert!(
        result2.contains("\"apple\""),
        "literal argument should be preserved: {result2}"
    );
}

#[test]
fn golden_list_push_integrated_in_expression_compiler() {
    use crate::codegen::js::js_events::{compile_expression_full, CompiledVars};
    use std::collections::HashSet;
    let vars: HashSet<String> = ["todos"].iter().map(|s| s.to_string()).collect();
    let cv = CompiledVars::new(&vars);
    // Should compile via compile_expression_full (not just compile_list_method)
    let result = compile_expression_full("todos.push(\"new item\")", &cv);
    assert!(
        result.starts_with("S.set('todos',"),
        "list push should be handled by full expression compiler: {result}"
    );
}

#[test]
fn golden_list_push_in_event_handler_js() {
    let src = r#"
layout MainLayout { main { slot content } }
component TodoList {
    state {
        todos: List = null
        draft: String = ""
    }
    view {
        ul {
            @for todo in todos {
                li "{todo}"
            }
        }
        input bind:value={draft}
        button on:click={todos.push(draft)} { "Ajouter" }
    }
}
page "home" { TodoList {} }
"#;
    let doc = parse_webc(src).expect("parse");
    let res = generate_html(&doc, "home", &opts()).expect("codegen");
    let js = generate_runtime_js(&res.handlers, &doc);
    assert!(
        js.contains("S.set('todos',[...S.get('todos')"),
        "push should expand to spread append in emitted JS:\n{js}"
    );
}

// ═══ v2.8.0 — Axe 3 : directives @loading et @catch ═════════════════════════

#[test]
fn golden_loading_block_parses_as_if_loading() {
    let src = r#"
component Fetcher {
    http { get: "/api/data" into: data }
    view {
        @loading {
            p "Chargement..."
        }
        @for item in data {
            li "{item}"
        }
    }
}
"#;
    let doc = parse_webc(src).expect("parse @loading");
    let comp = doc.components.get("Fetcher").expect("component exists");
    // @loading should compile to Element::If { condition: "loading" }
    let found = comp.view.iter().any(|el| {
        matches!(el, crate::core::ast::Element::If { condition, .. } if condition == "loading")
    });
    assert!(
        found,
        "@loading should produce Element::If {{ condition: 'loading' }}"
    );
}

#[test]
fn golden_catch_block_parses_as_if_error() {
    let src = r#"
component Fetcher {
    http { get: "/api/data" into: data }
    view {
        @catch {
            p "Une erreur est survenue."
        }
    }
}
"#;
    let doc = parse_webc(src).expect("parse @catch");
    let comp = doc.components.get("Fetcher").expect("component exists");
    let found = comp.view.iter().any(
        |el| matches!(el, crate::core::ast::Element::If { condition, .. } if condition == "error"),
    );
    assert!(
        found,
        "@catch should produce Element::If {{ condition: 'error' }}"
    );
}

#[test]
fn golden_loading_block_emits_if_binding_in_html() {
    let src = r#"
layout MainLayout { main { slot content } }
component Loader {
    http { get: "/api/posts" into: posts }
    state { loading: Boolean = true }
    view {
        @loading {
            p "Chargement..."
        }
        p "Données chargées"
    }
}
page "home" { Loader {} }
"#;
    let doc = parse_webc(src).expect("parse");
    let res = generate_html(&doc, "home", &opts()).expect("codegen");
    // v3: `loading` condition compiled to an expr ID; check the attr is present
    assert!(
        res.html.contains(&format!("{}=\"", attr_names::IF)),
        "@loading should emit data-webcore-if attribute:\n{}",
        res.html
    );
}

#[test]
fn golden_catch_block_emits_if_error_binding_in_html() {
    let src = r#"
layout MainLayout { main { slot content } }
component Loader {
    http { get: "/api/posts" into: posts }
    state { error: String = "" }
    view {
        @catch {
            p "Erreur : {error}"
        }
    }
}
page "home" { Loader {} }
"#;
    let doc = parse_webc(src).expect("parse");
    let res = generate_html(&doc, "home", &opts()).expect("codegen");
    // v3: `error` condition compiled to an expr ID; check the attr is present
    assert!(
        res.html.contains(&format!("{}=\"", attr_names::IF)),
        "@catch should emit data-webcore-if attribute:\n{}",
        res.html
    );
}

#[test]
fn golden_loading_and_catch_together_parse_correctly() {
    let src = r#"
component DataCard {
    http { get: "/api/card" into: card }
    state {
        loading: Boolean = true
        error: String = ""
    }
    view {
        @loading {
            span "..."
        }
        @catch {
            p "Erreur : {error}"
        }
        p "{card}"
    }
}
"#;
    let doc = parse_webc(src).expect("parse @loading + @catch together");
    let comp = doc.components.get("DataCard").expect("component exists");
    let loading_found = comp.view.iter().any(|el| {
        matches!(el, crate::core::ast::Element::If { condition, .. } if condition == "loading")
    });
    let error_found = comp.view.iter().any(
        |el| matches!(el, crate::core::ast::Element::If { condition, .. } if condition == "error"),
    );
    assert!(loading_found, "@loading block missing");
    assert!(error_found, "@catch block missing");
}

// ═══ v2.8.0 — Axe 1 : LSP helpers ═══════════════════════════════════════════

#[test]
fn lsp_word_at_returns_identifier_under_cursor() {
    use crate::cli::lsp::word_at_test;
    let source = "component Counter {\n    state { count: Number = 0 }\n}";
    // "Counter" starts at col 10
    assert_eq!(word_at_test(source, 0, 10), Some("Counter".to_string()));
    assert_eq!(word_at_test(source, 0, 15), Some("Counter".to_string()));
    // "component" keyword
    assert_eq!(word_at_test(source, 0, 0), Some("component".to_string()));
    // Whitespace → None
    assert_eq!(word_at_test(source, 0, 9), None);
}

#[test]
fn lsp_hover_returns_state_var_info() {
    use crate::cli::lsp::hover_for_symbol_test;
    let src = r#"
component Counter {
    state { count: Number = 0 }
    view { p "{count}" }
}
"#;
    let doc = parse_webc(src).expect("parse");
    let hover = hover_for_symbol_test(&doc, "count").expect("should find count");
    assert!(
        hover.contains("Number"),
        "type should appear in hover: {hover}"
    );
    assert!(
        hover.contains("count"),
        "name should appear in hover: {hover}"
    );
}

#[test]
fn lsp_hover_returns_component_info_with_props() {
    use crate::cli::lsp::hover_for_symbol_test;
    let src = r#"
component Badge {
    props { label: String color: String = "blue" }
    view { span "{label}" }
}
"#;
    let doc = parse_webc(src).expect("parse");
    let hover = hover_for_symbol_test(&doc, "Badge").expect("should find Badge");
    assert!(hover.contains("Badge"), "component name in hover: {hover}");
    assert!(hover.contains("label"), "prop listed in hover: {hover}");
}

// ── v3 import system ────────────────────────────────────────────────────────

#[test]
fn import_webc_parsed_as_component_import_not_data() {
    let src = r#"
import Button from "./Button.webc"
import posts  from "data/posts.json"

page "home" {
    view { p "hello" }
}
"#;
    let doc = parse_webc(src).expect("parse");
    // .webc → component_imports, not data imports
    assert_eq!(
        doc.component_imports.len(),
        1,
        "one component import expected"
    );
    assert_eq!(doc.component_imports[0].alias, "Button");
    assert_eq!(doc.component_imports[0].path, "./Button.webc");
    // .json → data imports (existing behaviour)
    assert_eq!(doc.imports.len(), 1, "one data import expected");
    assert_eq!(doc.imports[0].name, "posts");
}

#[test]
fn import_page_imports_map_populated_by_loader() {
    // Simulate what the loader does: parse a page file whose parsed doc has
    // component_imports, then verify page_imports is populated correctly.
    let src = r#"
import Counter from "./Counter.webc"
import Button  from "./Button.webc"

page "home" {
    view { p "hi" }
}
"#;
    let parsed = parse_webc(src).expect("parse");
    // Simulate loader logic: associate imports with the page
    let mut doc = WebCoreDocument {
        app: None,
        store: vec![],
        store_computed: vec![],
        locales: std::collections::BTreeMap::new(),
        default_locale: String::new(),
        wasm_module: None,
        layouts: std::collections::BTreeMap::new(),
        pages: parsed.pages.clone(),
        components: std::collections::BTreeMap::new(),
        imports: vec![],
        data_imports: std::collections::BTreeMap::new(),
        component_imports: vec![],
        page_imports: std::collections::BTreeMap::new(),
        source_files: std::collections::BTreeMap::new(),
    };
    if !parsed.component_imports.is_empty() {
        let aliases: std::collections::BTreeSet<String> = parsed
            .component_imports
            .iter()
            .map(|ci| ci.alias.clone())
            .collect();
        for name in doc.pages.keys() {
            doc.page_imports.insert(name.clone(), aliases.clone());
        }
    }
    let imports = doc
        .page_imports
        .get("home")
        .expect("home should have imports");
    assert!(imports.contains("Counter"));
    assert!(imports.contains("Button"));
    assert!(!imports.contains("Modal"), "Modal was not imported");
}

#[test]
fn page_without_imports_has_no_page_imports_entry() {
    let src = r#"
page "about" {
    view { p "about" }
}
"#;
    let doc = parse_webc(src).expect("parse");
    // No import declarations → page_imports stays empty → all components available (v2 compat)
    assert!(
        doc.page_imports.is_empty(),
        "no entry expected for pages without imports"
    );
    assert!(doc.component_imports.is_empty());
}

#[test]
fn lsp_hover_returns_none_for_unknown_symbol() {
    use crate::cli::lsp::hover_for_symbol_test;
    let src = r#"
component Empty {
    view { p "hello" }
}
"#;
    let doc = parse_webc(src).expect("parse");
    assert!(
        hover_for_symbol_test(&doc, "unknown").is_none(),
        "unknown symbol should return None"
    );
}

// ═══ v3.0.7 — @else if chained ═══════════════════════════════════════════════

#[test]
fn golden_else_if_chain_parses_to_nested_if() {
    // @if / @else if / @else must produce a nested Element::If in the else_branch.
    let src = r#"
component Traffic {
    state { status: String = "green" }
    view {
        @if status === "green" {
            span "Go"
        } @else if status === "yellow" {
            span "Slow"
        } @else {
            span "Stop"
        }
    }
}
"#;
    let doc = parse_webc(src).expect("parse @else if chain");
    let comp = doc.components.get("Traffic").expect("component");
    let outer_if = comp.view.first().expect("outer @if");
    let crate::core::ast::Element::If {
        condition,
        else_branch,
        ..
    } = outer_if
    else {
        panic!("expected Element::If, got {outer_if:?}");
    };
    assert_eq!(condition, r#"status === "green""#);

    let else_vec = else_branch.as_ref().expect("@else if branch present");
    assert_eq!(
        else_vec.len(),
        1,
        "else_branch should contain exactly one element (the @else if)"
    );

    let crate::core::ast::Element::If {
        condition: inner_cond,
        else_branch: inner_else,
        ..
    } = &else_vec[0]
    else {
        panic!(
            "@else if should parse as nested Element::If, got {:?}",
            else_vec[0]
        );
    };
    assert_eq!(inner_cond, r#"status === "yellow""#);
    assert!(
        inner_else.is_some(),
        "@else branch of the inner @else if should be present"
    );
}

#[test]
fn golden_else_if_chain_emits_correct_html_attrs() {
    // The HTML output for an @if / @else if / @else chain must include all three
    // data-webcore-if, data-webcore-else (the @else if wrapper), and data-webcore-else
    // (the final @else). All three branches must appear in the output.
    let src = r#"
layout MainLayout { main { slot content } }
component Traffic {
    state { status: String = "green" }
    view {
        @if status === "green" {
            span "Go"
        } @else if status === "yellow" {
            span "Slow"
        } @else {
            span "Stop"
        }
    }
}
page "home" { Traffic {} }
"#;
    let html = compile_to_html(src);
    assert!(
        html.contains(&format!("{}=\"", attr_names::IF)),
        "data-webcore-if attr missing:\n{html}"
    );
    // The @else if produces an @else div wrapping a nested @if div
    assert!(
        html.contains(&format!("{}=\"", attr_names::IF_ELSE)),
        "data-webcore-else attr missing:\n{html}"
    );
    // All three text nodes must appear in the HTML
    assert!(html.contains("Go"), "Go branch missing:\n{html}");
    assert!(html.contains("Slow"), "Slow branch missing:\n{html}");
    assert!(html.contains("Stop"), "Stop branch missing:\n{html}");
}

// ═══ v3.0.5 — prod-mode JS identifier renaming ═══════════════════════════════

#[test]
fn golden_prod_mode_renames_bind_identifiers() {
    // In prod mode the inline <script> must use shortened identifiers (_bi, _bf, …)
    // and must NOT contain the long-form names bindIf, bindFor, etc.
    let src = r#"
layout MainLayout { main { slot content } }
component Ticker {
    state { count: Number = 0 }
    view {
        @if count > 0 { span "positive" }
        @for i in items { li "{i}" }
    }
}
page "home" { Ticker {} }
"#;
    let doc = parse_webc(src).expect("parse");
    let prod_opts = HtmlPageOptions {
        lang: "en".into(),
        title: "Test".into(),
        extra_css_files: vec![],
        critical_css: None,
        csp_meta: None,
        prod: true,
        source_maps: false,
        inline_runtime: true,
    };
    let html = generate_html(&doc, "home", &prod_opts)
        .expect("codegen")
        .html;

    // Extract just the <script> block
    let script_start = html
        .find("<script>")
        .expect("<script> missing in prod HTML");
    let script_end = html
        .find("</script>")
        .expect("</script> missing in prod HTML");
    let script = &html[script_start..script_end];

    assert!(
        script.contains("_bi"),
        "prod: _bi (bindIf) should appear in script:\n{script}"
    );
    assert!(
        script.contains("_bf"),
        "prod: _bf (bindFor) should appear in script:\n{script}"
    );
    assert!(
        !script.contains("bindIf"),
        "prod: bindIf should be renamed away:\n{script}"
    );
    assert!(
        !script.contains("bindFor"),
        "prod: bindFor should be renamed away:\n{script}"
    );
}

// ═══ v3.1.1 — publishDiagnostics ═════════════════════════════════════════════

#[test]
fn lsp_diagnostics_empty_on_valid_source() {
    use crate::cli::lsp::first_diagnostic;
    let src = r#"
component Counter {
    state { count: Number = 0 }
    view  { div { "{count}" } }
}
page "home" { view { Counter {} } }
"#;
    assert!(
        first_diagnostic(src).is_none(),
        "valid source must produce no diagnostics"
    );
}

#[test]
fn lsp_diagnostics_error_on_syntax_error() {
    use crate::cli::lsp::first_diagnostic;
    // Missing closing brace — should produce one diagnostic.
    let src = "component Broken { state { count: Number = 0 }";
    let diag = first_diagnostic(src).expect("broken source must produce a diagnostic");
    assert_eq!(
        diag["severity"], 1,
        "syntax errors must have severity Error (1)"
    );
    assert!(
        !diag["message"].as_str().unwrap_or("").is_empty(),
        "diagnostic message must not be empty"
    );
}

#[test]
fn lsp_diagnostics_range_is_within_source() {
    use crate::cli::lsp::first_diagnostic;
    let src = "page \"home\" { view { !!!invalid!!! } }";
    let diag = first_diagnostic(src).expect("invalid source must produce a diagnostic");
    let line = diag["range"]["start"]["line"].as_u64().unwrap_or(u64::MAX);
    let src_lines = src.lines().count() as u64;
    assert!(
        line < src_lines,
        "diagnostic line {line} must be within the source ({src_lines} lines)"
    );
}

// ── v3.1.2: Semantic tokens ─────────────────────────────────────────────────

#[test]
fn sem_tok_keyword_component() {
    // "component" should be a keyword token (type 0)
    let src = r#"component Foo { view { p "hi" } }"#;
    let data = crate::cli::lsp::semantic_tokens_for_source(src);
    let toks = crate::cli::lsp::decode_semantic_tokens(&data);
    assert!(
        toks.iter().any(|&(_, _, _, tt, _)| tt == 0),
        "no keyword token"
    );
    // "Foo" should be a class token (type 5) because it's a component name
    assert!(
        toks.iter().any(|&(_, _, _, tt, _)| tt == 5),
        "no class token for Foo"
    );
}

// ── v3.2: Source maps ────────────────────────────────────────────────────────

#[test]
fn source_map_vlq_encodes_zero() {
    // VLQ(0) = "A" (0 in base64-vlq)
    use crate::codegen::js::sourcemap::encode_vlq_test;
    assert_eq!(encode_vlq_test(0), "A", "VLQ(0) must be 'A'");
    assert_eq!(encode_vlq_test(1), "C", "VLQ(1) must be 'C'");
    assert_eq!(encode_vlq_test(-1), "D", "VLQ(-1) must be 'D'");
}

#[test]
fn source_map_builder_emits_valid_json() {
    use crate::codegen::js::sourcemap::{Mapping, SourceMapBuilder};
    let mut builder = SourceMapBuilder::new("test.webc", "page \"home\" { p \"hello\" }");
    builder.add(Mapping {
        output_line: 0,
        output_col: 0,
        source_line: 0,
        source_col: 0,
    });
    let json = builder.build();
    // Must be valid JSON with version:3
    assert!(
        json.contains("\"version\":3"),
        "version field missing: {json}"
    );
    assert!(
        json.contains("\"sources\":[\"test.webc\"]"),
        "sources field missing: {json}"
    );
    assert!(
        json.contains("\"mappings\""),
        "mappings field missing: {json}"
    );
    let mappings_start = json.find("\"mappings\":\"").expect("mappings key") + 12;
    let mappings_end = json[mappings_start..].find('"').expect("close quote") + mappings_start;
    let mappings = &json[mappings_start..mappings_end];
    assert!(
        !mappings.is_empty(),
        "mappings must be non-empty when a mapping was added: {json}"
    );
}

#[test]
fn sem_tok_type_names() {
    let src = r#"component C { state { n: Number = 0  s: String = "" } view { p "x" } }"#;
    let data = crate::cli::lsp::semantic_tokens_for_source(src);
    let toks = crate::cli::lsp::decode_semantic_tokens(&data);
    // "Number" and "String" → type tokens (index 1)
    let type_count = toks.iter().filter(|&&(_, _, _, tt, _)| tt == 1).count();
    assert!(type_count >= 2, "expected ≥2 type tokens, got {type_count}");
}

#[test]
fn sem_tok_string_literal() {
    let src = r#"component C { view { p "hello world" } }"#;
    let data = crate::cli::lsp::semantic_tokens_for_source(src);
    let toks = crate::cli::lsp::decode_semantic_tokens(&data);
    // "hello world" → string token (index 2)
    assert!(
        toks.iter().any(|&(_, _, _, tt, _)| tt == 2),
        "no string token"
    );
}

#[test]
fn source_map_generated_for_dev_build() {
    // Compile a component with a state expression in dev mode (source_maps: true)
    let src = r#"
layout MainLayout { main { slot content } }
component Counter {
    state { count: Number = 0 }
    view { p "{count}" }
}
page "home" { Counter {} }
"#;
    let doc = crate::parser::parse_webc(src).expect("parse");
    let options = HtmlPageOptions {
        lang: "en".into(),
        title: "Test".into(),
        extra_css_files: vec![],
        critical_css: None,
        csp_meta: None,
        prod: false,
        source_maps: true,
        inline_runtime: true,
    };
    let res = generate_html(&doc, "home", &options).expect("codegen");
    assert!(
        res.source_map_json.is_some(),
        "source_map_json must be Some in dev mode with expressions"
    );
}

#[test]
fn sem_tok_line_comment() {
    let src = "// this is a comment\ncomponent C { view { p \"x\" } }";
    let data = crate::cli::lsp::semantic_tokens_for_source(src);
    let toks = crate::cli::lsp::decode_semantic_tokens(&data);
    // comment is on line 0 → comment token (index 3) at line 0
    assert!(
        toks.iter().any(|&(ln, _, _, tt, _)| ln == 0 && tt == 3),
        "no comment token on line 0"
    );
}

#[test]
fn source_map_not_generated_in_prod() {
    // Same component with source_maps: false (prod mode)
    let src = r#"
layout MainLayout { main { slot content } }
component Counter {
    state { count: Number = 0 }
    view { p "{count}" }
}
page "home" { Counter {} }
"#;
    let doc = crate::parser::parse_webc(src).expect("parse");
    let options = HtmlPageOptions {
        lang: "en".into(),
        title: "Test".into(),
        extra_css_files: vec![],
        critical_css: None,
        csp_meta: None,
        prod: false,
        source_maps: false,
        inline_runtime: true,
    };
    let res = generate_html(&doc, "home", &options).expect("codegen");
    assert!(
        res.source_map_json.is_none(),
        "source_map_json must be None when source_maps=false"
    );
}

#[test]
fn sem_tok_at_directive() {
    let src = r#"component C { state { ok: Boolean = true } view { @if ok { p "yes" } } }"#;
    let data = crate::cli::lsp::semantic_tokens_for_source(src);
    let toks = crate::cli::lsp::decode_semantic_tokens(&data);
    // "@if" → keyword token (index 0)
    assert!(
        toks.iter().any(|&(_, _, _, tt, _)| tt == 0),
        "no keyword token for @if"
    );
}

#[test]
fn sem_tok_state_var() {
    let src = r#"component C { state { count: Number = 0 } view { p "{count}" } }"#;
    let data = crate::cli::lsp::semantic_tokens_for_source(src);
    let toks = crate::cli::lsp::decode_semantic_tokens(&data);
    // "count" appears as a state var → variable tokens (index 4), at least one occurrence in the view
    let var_count = toks.iter().filter(|&&(_, _, _, tt, _)| tt == 4).count();
    assert!(
        var_count >= 1,
        "expected ≥1 variable token for count, got {var_count}"
    );
}

#[test]
fn sem_tok_computed_readonly() {
    let src =
        r#"component C { state { x: Number = 1 } computed { dbl = x * 2 } view { p "{dbl}" } }"#;
    let data = crate::cli::lsp::semantic_tokens_for_source(src);
    let toks = crate::cli::lsp::decode_semantic_tokens(&data);
    // "dbl" is a computed var → variable (4) with readonly modifier (1)
    assert!(
        toks.iter().any(|&(_, _, _, tt, mods)| tt == 4 && mods == 1),
        "no readonly variable token for computed var"
    );
}

#[test]
fn sem_tok_attr_prefix() {
    let src = r#"component C { state { n: Number = 0 } view { button on:click={n+=1} { "+" } } }"#;
    let data = crate::cli::lsp::semantic_tokens_for_source(src);
    let toks = crate::cli::lsp::decode_semantic_tokens(&data);
    // "on:click" → keyword token (index 0)
    // There should be multiple keyword tokens (component, view, button(?), on:click)
    let kw_count = toks.iter().filter(|&&(_, _, _, tt, _)| tt == 0).count();
    assert!(kw_count >= 1, "no keyword token for on:click");
}

#[test]
fn sem_tok_store_prefix() {
    let src = r#"component C { view { p "{$store.count}" } }"#;
    let data = crate::cli::lsp::semantic_tokens_for_source(src);
    let toks = crate::cli::lsp::decode_semantic_tokens(&data);
    // "$store.count" → variable token (index 4)
    assert!(
        toks.iter().any(|&(_, _, _, tt, _)| tt == 4),
        "no variable token for $store.count"
    );
}

#[test]
fn inline_script_has_sourcemapping_url() {
    // When source_maps: true, HTML should contain the sourceMappingURL comment
    let src = r#"
layout MainLayout { main { slot content } }
component Counter {
    state { count: Number = 0 }
    view { p "{count}" }
}
page "home" { Counter {} }
"#;
    let doc = crate::parser::parse_webc(src).expect("parse");
    let options = HtmlPageOptions {
        lang: "en".into(),
        title: "Test".into(),
        extra_css_files: vec![],
        critical_css: None,
        csp_meta: None,
        prod: false,
        source_maps: true,
        inline_runtime: true,
    };
    let res = generate_html(&doc, "home", &options).expect("codegen");
    assert!(
        res.html.contains("sourceMappingURL"),
        "HTML inline script must contain sourceMappingURL comment when source_maps=true:\n{}",
        res.html
    );
}

// ── v3.1.3: Code actions ──────────────────────────────────────────────────────

#[test]
fn code_action_import_unknown_component() {
    let src = "page \"home\" { Foo {} }";
    let actions = crate::cli::lsp::code_actions_for_source(src, 0, 14);
    assert!(
        actions.iter().any(|a| {
            a["title"].as_str().unwrap_or("").contains("Import")
                && a["title"].as_str().unwrap_or("").contains("Foo")
        }),
        "expected import action for Foo"
    );
}

#[test]
fn code_action_no_import_for_known_component() {
    let src = "component Counter { view { p \"x\" } }\npage \"home\" { Counter {} }";
    let actions = crate::cli::lsp::code_actions_for_source(src, 1, 14);
    assert!(
        !actions.iter().any(|a| {
            a["title"].as_str().unwrap_or("").contains("Import")
                && a["title"].as_str().unwrap_or("").contains("Counter")
        }),
        "should not offer import for known component"
    );
}

#[test]
fn code_action_no_import_when_already_imported() {
    let src = "import Bar from \"./Bar.webc\"\npage \"home\" { Bar {} }";
    let actions = crate::cli::lsp::code_actions_for_source(src, 1, 14);
    assert!(
        !actions.iter().any(|a| {
            a["title"].as_str().unwrap_or("").contains("Import")
                && a["title"].as_str().unwrap_or("").contains("Bar")
        }),
        "should not offer import when already imported"
    );
}

#[test]
fn code_action_add_unknown_var_to_state() {
    let src = "component C {\n    state { count: Number = 0 }\n    view { p \"{draft}\" }\n}";
    // line 2: `    view { p "{draft}" }` — 'd' of "draft" is at col 15
    let actions = crate::cli::lsp::code_actions_for_source(src, 2, 15);
    assert!(
        actions.iter().any(|a| {
            a["title"].as_str().unwrap_or("").contains("draft")
                && a["title"].as_str().unwrap_or("").contains("state")
        }),
        "expected add-to-state action for draft"
    );
}

#[test]
fn code_action_no_add_for_known_var() {
    let src = "component C {\n    state { count: Number = 0 }\n    view { p \"{count}\" }\n}";
    // line 2: `    view { p "{count}" }` — 'c' of "count" is at col 15
    let actions = crate::cli::lsp::code_actions_for_source(src, 2, 15);
    assert!(
        !actions.iter().any(|a| {
            a["title"].as_str().unwrap_or("").contains("count")
                && a["title"].as_str().unwrap_or("").contains("state")
        }),
        "should not offer add-to-state for known var"
    );
}

/// Regression: the v3 reactive bind functions must NOT take the expression
/// map `_e` as a parameter — they close over the module-scoped `const _e`.
/// Otherwise user-authored `bind()` calls inside `on:mount` blocks pass no
/// argument and crash with "Cannot read properties of undefined".
#[test]
fn v3_bind_fns_close_over_expr_map() {
    let src = r#"
layout MainLayout { main { slot content } }
component Toggle {
    state { open: Boolean = false }
    view {
        @if open { p "{open}" }
    }
}
page "home" { Toggle {} }
"#;
    let html = compile_to_html(src);
    // bindIf/bind must be parameterless so user bind() works
    assert!(
        html.contains("const bindIf=()=>"),
        "bindIf must be parameterless (close over _e):\n{html}"
    );
    assert!(
        html.contains("const bind=()=>"),
        "bind must be parameterless (close over _e):\n{html}"
    );
    assert!(
        !html.contains("(_e)=>"),
        "no v3 bind fn should take _e as a parameter:\n{html}"
    );
}

/// Regression: in dev builds (source_maps) the `_e` map emits one closure per
/// line. `compile_read_expr` already returns a complete `()=>expr` closure, so
/// the emission must be `id:closure`, not `id:()=>closure` — the latter double-
/// wraps it (`e0:()=>()=>S.get('x')`) and `fn()` returns a function (always
/// truthy), permanently showing every `@if` branch.
#[test]
fn dev_expr_map_not_double_wrapped() {
    let src = r#"
layout MainLayout { main { slot content } }
component Toggle {
    state { open: Boolean = false }
    view {
        @if open { p "{open}" }
    }
}
page "home" { Toggle {} }
"#;
    let doc = parse_webc(src).expect("parse");
    let options = HtmlPageOptions {
        source_maps: true,
        inline_runtime: true,
        ..opts()
    };
    let html = generate_html(&doc, "home", &options).expect("codegen").html;
    assert!(
        !html.contains("=>()=>S.get("),
        "expr-map closures must not be double-wrapped:\n{html}"
    );
    assert!(
        html.contains("()=>S.get('open')"),
        "expr-map must contain the single-wrapped closure:\n{html}"
    );
}
