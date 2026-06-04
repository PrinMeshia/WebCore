//! Golden / integration tests: full parse → codegen pipeline.

use crate::ast::{Component, Span, WebCoreDocument};
use crate::codegen::codegen_css::generate_combined_css;
use crate::codegen::codegen_html::{generate_html, HtmlPageOptions};
use crate::codegen::codegen_js::{generate_runtime_js, minify_js};
use crate::parser::parse_webc;
use std::collections::HashMap;

fn opts() -> HtmlPageOptions {
    HtmlPageOptions {
        lang: "en".into(),
        title: "Test".into(),
        extra_css_files: vec![],
    }
}

// ── Parser + HTML codegen ──────────────────────────────────────────────────

#[test]
fn golden_page_heading_renders() {
    let src = r#"
layout MainLayout { main { slot content } }
page "home" { h1 "Hello WebCore!" }
"#;
    let doc = parse_webc(src).expect("parse");
    let res = generate_html(&doc, "home", &opts()).expect("codegen");
    // codegen adds newlines between child elements, so check open tag + content, not the full inline form
    assert!(
        res.html.contains("<h1>Hello WebCore!"),
        "h1 missing:\n{}",
        res.html
    );
    assert!(res.html.contains("</h1>"), "h1 close tag missing");
    assert!(res.html.contains("lang=\"en\""));
    assert!(res.html.contains("<title>Test</title>"));
}

#[test]
fn golden_state_interpolation_emits_span() {
    let src = r#"
layout MainLayout { main { slot content } }
component Counter {
    state { count: Number = 0 }
    view { p "Value: {count}" }
}
page "home" { Counter {} }
"#;
    let doc = parse_webc(src).expect("parse");
    let res = generate_html(&doc, "home", &opts()).expect("codegen");
    assert!(
        res.html.contains("data-webcore-interpolation=\"count\""),
        "interpolation span missing:\n{}",
        res.html
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
    // HTML should have an onclick attribute
    assert!(
        res.html.contains("onclick="),
        "onclick missing:\n{}",
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
    let src = r#"
layout MainLayout { main { slot content } }
component Counter {
    state { count: Number = 0 }
    view { p "{count}" }
}
page "home" { Counter {} }
"#;
    let doc = parse_webc(src).expect("parse");
    let js = generate_runtime_js(&[], &doc);
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
        locales: std::collections::HashMap::new(),
        default_locale: String::new(),
        wasm_module: None,
        layouts: std::collections::HashMap::new(),
        pages: std::collections::HashMap::new(),
        components: std::collections::HashMap::new(),
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
            view: vec![],
            style: vec![crate::ast::StyleItem::Rule(crate::ast::StyleRule {
                selector: "button".into(),
                properties: vec![crate::ast::StyleProperty {
                    name: "color".into(),
                    value: "red".into(),
                    span: Span::default(),
                }],
                span: Span::default(),
            })],
            span: Span::default(),
        },
    );
    let css = generate_combined_css(None, &doc);
    assert!(css.contains("[data-v="), "no scoped selector in:\n{}", css);
    assert!(css.contains("color: red") || css.contains("color:red"));
}

// ── Props inter-composants ─────────────────────────────────────────────────

#[test]
fn golden_prop_substituted_in_view() {
    let src = r#"
layout MainLayout { main { slot content } }
component Greeting {
    props { name: String }
    view { p "Hello {name}!" }
}
page "home" { Greeting name="WebCore" {} }
"#;
    let doc = parse_webc(src).expect("parse");
    let res = generate_html(&doc, "home", &opts()).expect("codegen");
    // The prop value must appear in the output (fragments may be separated by whitespace)
    assert!(
        res.html.contains("Hello"),
        "Hello fragment missing:\n{}",
        res.html
    );
    assert!(
        res.html.contains("WebCore"),
        "prop value missing:\n{}",
        res.html
    );
    // Should NOT leave the raw interpolation span for a resolved prop
    assert!(
        !res.html.contains("data-webcore-interpolation=\"name\""),
        "unresolved prop span still present:\n{}",
        res.html
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
    let src = r#"
store { count: Number = 0 }
layout MainLayout { main { slot content } }
page "home" { button on:click={$store.count += 1} { "+" } }
"#;
    let doc = parse_webc(src).expect("parse");
    let res = crate::codegen::codegen_html::generate_html(&doc, "home", &opts()).expect("codegen");
    let js = generate_runtime_js(&res.handlers, &doc);
    assert!(
        js.contains("STORE.set('count',STORE.get('count')+1)"),
        "store increment expression missing:\n{}",
        js
    );
}

// ── Validation de formulaires ──────────────────────────────────────────────

#[test]
fn golden_validate_attrs_converted_to_data_attrs() {
    let src = r#"
layout MainLayout { main { slot content } }
page "home" {
    form {
        input type="text" name="username"
              validate:required="Le nom est requis"
              validate:minlength="3,Au moins 3 caractères"
    }
}
"#;
    let doc = parse_webc(src).expect("parse");
    let res = generate_html(&doc, "home", &opts()).expect("codegen");
    assert!(
        res.html.contains("data-webcore-field=\"username\""),
        "data-webcore-field missing:\n{}",
        res.html
    );
    assert!(
        res.html
            .contains("data-webcore-validate-required=\"Le nom est requis\""),
        "validate-required missing:\n{}",
        res.html
    );
    assert!(
        res.html.contains("data-webcore-validate-minlength=\"3\""),
        "validate-minlength missing:\n{}",
        res.html
    );
    assert!(
        res.html.contains("data-webcore-validate-minlength-msg="),
        "validate-minlength-msg missing:\n{}",
        res.html
    );
    assert!(
        !res.html.contains("validate:required"),
        "raw validate: attr should not appear in output:\n{}",
        res.html
    );
}

#[test]
fn golden_error_block_renders_hidden_div() {
    let src = r#"
layout MainLayout { main { slot content } }
page "home" {
    form {
        input type="text" name="email" validate:email="Email invalide"
        @error "email" { span "Erreur email" }
    }
}
"#;
    let doc = parse_webc(src).expect("parse");
    let res = generate_html(&doc, "home", &opts()).expect("codegen");
    assert!(
        res.html.contains("data-webcore-error=\"email\""),
        "data-webcore-error missing:\n{}",
        res.html
    );
    assert!(
        res.html.contains("style=\"display:none\""),
        "error block should be hidden by default:\n{}",
        res.html
    );
}

#[test]
fn golden_validation_js_in_runtime() {
    let src = r#"
layout MainLayout { main { slot content } }
page "home" {
    form {
        input type="email" name="email" validate:email="Email invalide"
        @error "email" { span "Erreur" }
    }
}
"#;
    let doc = parse_webc(src).expect("parse");
    let js = generate_runtime_js(&[], &doc);
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
    let src = r#"
layout MainLayout { main { slot content } }
page "home" { h1 "Hello" }
"#;
    let doc = parse_webc(src).expect("parse");
    let js = generate_runtime_js(&[], &doc);
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
    let src = r#"
layout MainLayout { main { slot content } }
page "home" {
    form {
        input type="text" name="user" validate:required="Requis"
    }
}
"#;
    let doc = parse_webc(src).expect("parse");
    let js = generate_runtime_js(&[], &doc);
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
    let res = generate_html(&doc, "home", &opts()).expect("codegen");
    let initial = crate::ssg::build_initial_state(&doc);
    let ssg = crate::ssg::apply_ssg_with_locales(&res.html, &initial, &HashMap::new(), "");
    assert!(
        ssg.contains(r#"data-webcore-interpolation="count">7</span>"#),
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
    let res = generate_html(&doc, "home", &opts()).expect("codegen");
    let initial = crate::ssg::build_initial_state(&doc);
    let ssg = crate::ssg::apply_ssg_with_locales(&res.html, &initial, &HashMap::new(), "");
    // @if branch should be visible (show = 1 > 0): check that the if-div has display:block
    assert!(
        ssg.contains(r#"data-webcore-if="show &gt; 0""#)
            && ssg.contains(r#"style="display:block""#),
        "@if branch not pre-rendered as visible:\n{}",
        ssg
    );
    // @else branch should be hidden: the else-div has display:none
    assert!(
        ssg.contains(r#"data-webcore-else="show &gt; 0""#)
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
    let mut fr: HashMap<String, String> = HashMap::new();
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
    let mut fr: HashMap<String, String> = HashMap::new();
    fr.insert("welcome".to_string(), "Bienvenue".to_string());
    doc.locales.insert("fr".to_string(), fr);
    doc.default_locale = "fr".to_string();

    let res = generate_html(&doc, "home", &opts()).expect("codegen");
    // Interpolation span must exist
    assert!(
        res.html.contains("data-webcore-interpolation"),
        "no interpolation span:\n{}",
        res.html
    );
    let state = crate::ssg::build_initial_state(&doc);
    let ssg = crate::ssg::apply_ssg_with_locales(&res.html, &state, &doc.locales, "fr");
    assert!(
        ssg.contains("Bienvenue"),
        "translation not pre-rendered:\n{}",
        ssg
    );
}

#[test]
fn golden_i18n_no_locales_runtime_omits_t() {
    let src = r#"
layout MainLayout { main { slot content } }
page "home" { h1 "hi" }
"#;
    let doc = parse_webc(src).expect("parse");
    let js = generate_runtime_js(&[], &doc);
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
    let src = r#"
layout MainLayout { main { slot content } }
page "home" { h1 "hi" }
"#;
    let doc = parse_webc(src).expect("parse");
    let js = generate_runtime_js(&[], &doc);
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
    let src = r#"
layout MainLayout { main { slot content } }
page "home" { h1 "hi" }
"#;
    let doc = parse_webc(src).expect("parse");
    let js = generate_runtime_js(&[], &doc);
    let minified = minify_js(&js);
    // Must still be wrapped in a block
    assert!(minified.starts_with('{'), "block open missing");
    assert!(minified.ends_with('}'), "block close missing");
    // No comment lines
    assert!(!minified.contains("//"));
    // Shorter than original
    assert!(minified.len() < js.len(), "minified should be shorter");
}

// ── Props réactives (v0.8.0) ──────────────────────────────────────────────

#[test]
fn golden_reactive_prop_stays_interpolation() {
    let src = r#"
layout MainLayout { main { slot content } }
component Badge {
    props { label: String }
    view { span "Valeur : {label}" }
}
page "home" {
    Badge label={count} {}
}
"#;
    let doc = parse_webc(src).expect("parse");
    let res = generate_html(&doc, "home", &opts()).expect("codegen");
    // Dynamic prop: the interpolation span should reference `count`, not `label`
    assert!(
        res.html.contains("data-webcore-interpolation=\"count\""),
        "reactive prop should produce interpolation for `count`:\n{}",
        res.html
    );
    assert!(
        !res.html.contains("data-webcore-interpolation=\"label\""),
        "unresolved `label` span should not appear:\n{}",
        res.html
    );
}

#[test]
fn golden_static_prop_still_substituted() {
    let src = r#"
layout MainLayout { main { slot content } }
component Greeting {
    props { name: String }
    view { p "Hello {name}!" }
}
page "home" { Greeting name="Alice" {} }
"#;
    let doc = parse_webc(src).expect("parse");
    let res = generate_html(&doc, "home", &opts()).expect("codegen");
    assert!(
        res.html.contains("Alice"),
        "static prop value missing:\n{}",
        res.html
    );
    assert!(
        !res.html.contains("data-webcore-interpolation=\"name\""),
        "unresolved name span should not appear:\n{}",
        res.html
    );
}

// ── Named slots (v0.8.0) ──────────────────────────────────────────────────

#[test]
fn golden_named_slot_filled_from_page() {
    let src = r#"
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
"#;
    let doc = parse_webc(src).expect("parse");
    let res = generate_html(&doc, "home", &opts()).expect("codegen");
    assert!(
        res.html.contains("<h1>Titre"),
        "named slot header content missing:\n{}",
        res.html
    );
    assert!(
        res.html.contains("<p>Contenu principal"),
        "default content slot missing:\n{}",
        res.html
    );
}

#[test]
fn golden_unnamed_slot_backwards_compat() {
    let src = r#"
layout MainLayout { main { slot content } }
page "home" { h1 "Simple" }
"#;
    let doc = parse_webc(src).expect("parse");
    let res = generate_html(&doc, "home", &opts()).expect("codegen");
    assert!(
        res.html.contains("<h1>Simple"),
        "backward-compat unnamed slot broken:\n{}",
        res.html
    );
}

// ── computed (v0.9.0) ────────────────────────────────────────────────────

#[test]
fn golden_computed_emits_rebind_in_js() {
    let src = r#"
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
"#;
    let doc = parse_webc(src).expect("parse");
    let js = generate_runtime_js(&[], &doc);
    assert!(
        js.contains("rebindComputed"),
        "rebindComputed missing:\n{}",
        js
    );
    assert!(js.contains("COMPUTED"), "COMPUTED array missing:\n{}", js);
    assert!(js.contains("'sum'"), "computed var name missing:\n{}", js);
    // bind() must call rebindComputed
    assert!(
        js.contains("rebindComputed()"),
        "bind does not call rebindComputed:\n{}",
        js
    );
    // State class must have setQ
    assert!(
        js.contains("setQ(k,v)"),
        "setQ missing in State class:\n{}",
        js
    );
}

#[test]
fn golden_computed_no_array_when_empty() {
    let src = r#"
layout MainLayout { main { slot content } }
component Simple {
    state { count: Number = 0 }
    view { p "{count}" }
}
page "home" { Simple {} }
"#;
    let doc = parse_webc(src).expect("parse");
    let js = generate_runtime_js(&[], &doc);
    // When no computed vars: both COMPUTED array and rebindComputed are tree-shaken away
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
    // bind() still present for interpolations, but without rebindComputed call
    assert!(
        js.contains("data-webcore-interpolation"),
        "bind() should wire interpolations:\n{}",
        js
    );
}

// ── on:mount (v0.9.0) ────────────────────────────────────────────────────

#[test]
fn golden_on_mount_body_in_domcontentloaded() {
    let src = r#"
layout MainLayout { main { slot content } }
component Loader {
    state { data: String = "" }
    on:mount {
        data = "loaded"
    }
    view { p "{data}" }
}
page "home" { Loader {} }
"#;
    let doc = parse_webc(src).expect("parse");
    let js = generate_runtime_js(&[], &doc);
    // Mount body should appear inside DOMContentLoaded
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
    let src = r#"
layout MainLayout { main { slot content } }
component Cleanup {
    on:destroy {
        clearInterval(timer)
    }
    view { p "test" }
}
page "home" { Cleanup {} }
"#;
    let doc = parse_webc(src).expect("parse");
    let js = generate_runtime_js(&[], &doc);
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
    let src = r#"
layout MainLayout { main { slot content } }
component Simple {
    view { p "test" }
}
page "home" { Simple {} }
"#;
    let doc = parse_webc(src).expect("parse");
    let js = generate_runtime_js(&[], &doc);
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
    let src = r#"
layout MainLayout { main { slot content } }
page "home" {
    button on:click={emit("myEvent")} { "Fire" }
}
"#;
    let doc = parse_webc(src).expect("parse");
    let res = generate_html(&doc, "home", &opts()).expect("codegen");
    let js = generate_runtime_js(&res.handlers, &doc);
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
    let src = r#"
layout MainLayout { main { slot content } }
component Notifier {
    view { button on:click={emit("ping")} { "Ping" } }
}
page "home" {
    Notifier on:ping={ping_count = 1} {}
}
"#;
    let doc = parse_webc(src).expect("parse");
    let js = generate_runtime_js(&[], &doc);
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
    let css = crate::codegen::codegen_css::generate_combined_css(None, &doc);
    assert!(
        css.contains("@media (max-width: 768px)"),
        "@media block missing in output:\n{}",
        css
    );
    // Scoped selector must appear inside the @media block
    assert!(
        css.contains("[data-v="),
        "scoped selector missing inside @media:\n{}",
        css
    );
}

// ── v1.1.0 features ───────────────────────────────────────────────────────

#[test]
fn golden_compound_prop_substituted() {
    let src = r#"
layout MainLayout { main { slot content } }
component Counter {
    props { step }
    view { p "{step + 1}" }
}
page "home" { Counter step="2" {} }
"#;
    let doc = parse_webc(src).expect("parse");
    let res = generate_html(&doc, "home", &opts()).expect("gen");
    // Static prop "step"="2" should be substituted inside the compound expression
    assert!(
        res.html.contains("(2) + 1"),
        "compound prop not substituted: {}",
        res.html
    );
}

#[test]
fn golden_prop_in_attribute_substituted() {
    let src = r#"
layout MainLayout { main { slot content } }
component Badge {
    props { color }
    view { span class={color} "ok" }
}
page "home" { Badge color="green" {} }
"#;
    let doc = parse_webc(src).expect("parse");
    let res = generate_html(&doc, "home", &opts()).expect("gen");
    // Static prop "color"="green" → attribute becomes class="green"
    assert!(
        res.html.contains("class=\"green\""),
        "prop not substituted in attribute: {}",
        res.html
    );
}

#[test]
fn golden_i18n_plural_t_function() {
    let src = r#"
layout MainLayout { main { slot content } }
page "home" { p "{t(\"items\", 3)}" }
"#;
    let mut doc = parse_webc(src).expect("parse");
    let mut en_msgs = HashMap::new();
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
    assert!(
        js.contains("_one"),
        "plural key suffix _one missing: {js}"
    );
}

#[test]
fn golden_for_key_emits_data_attribute() {
    let src = r#"
layout MainLayout { main { slot content } }
page "home" {
    @for item key=item in items {
        li "{item}"
    }
}
"#;
    let doc = parse_webc(src).expect("parse");
    let res = generate_html(&doc, "home", &opts()).expect("gen");
    assert!(
        res.html.contains("data-webcore-for-key=\"item\""),
        "for-key attribute missing: {}",
        res.html
    );
}

#[test]
fn golden_param_routes_emit_routes_array() {
    let src = r#"
app MyApp {
    routes {
        "/": HomePage
        "/post/:slug": PostPage
    }
}
layout MainLayout { main { slot content } }
page "home" { p "home" }
page "post" { p "post {$route.slug}" }
"#;
    let doc = parse_webc(src).expect("parse");
    let js = generate_runtime_js(&[], &doc);
    assert!(
        js.contains("const ROUTES=["),
        "ROUTES array missing: {js}"
    );
    assert!(
        js.contains("ROUTE_PARAMS"),
        "ROUTE_PARAMS missing: {js}"
    );
    assert!(
        js.contains("slug"),
        "slug param missing in routes: {js}"
    );
}

#[test]
fn golden_non_param_routes_use_tofile() {
    let src = r#"
app MyApp {
    routes {
        "/": HomePage
        "/about": AboutPage
    }
}
layout MainLayout { main { slot content } }
page "home" { p "home" }
page "about" { p "about" }
"#;
    let doc = parse_webc(src).expect("parse");
    let js = generate_runtime_js(&[], &doc);
    // No parameterized routes — should use simple toFile() approach
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
    let src = "page \"home\" { p \"hello {}\" }";
    let err = crate::parser::parse_webc(src).unwrap_err();
    let display = format!("{}", err);
    // Should contain a caret line
    assert!(display.contains('^'), "caret missing in error: {display}");
}

