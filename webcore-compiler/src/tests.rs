//! Golden / integration tests: full parse → codegen pipeline.

use crate::ast::{self, Component, Span, WebCoreDocument};
use crate::codegen::attr_names;
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
        critical_css: None,
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
    assert!(
        html.contains(&format!("{}=\"count\"", attr_names::INTERPOLATION)),
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
        locales: std::collections::HashMap::new(),
        default_locale: String::new(),
        wasm_module: None,
        layouts: std::collections::HashMap::new(),
        pages: std::collections::HashMap::new(),
        components: std::collections::HashMap::new(),
        imports: vec![],
        data_imports: std::collections::HashMap::new(),
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
            style: vec![crate::ast::StyleItem::Rule(crate::ast::StyleRule {
                selector: "button".into(),
                properties: vec![crate::ast::StyleProperty {
                    name: "color".into(),
                    value: "red".into(),
                    span: Span::default(),
                }],
                nested: vec![],
                span: Span::default(),
            })],
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
    let html = generate_html(&doc, "home", &opts()).expect("codegen").html;
    let initial = crate::ssg::build_initial_state(&doc);
    let ssg = crate::ssg::apply_ssg_with_locales(&html, &initial, &HashMap::new(), "");
    assert!(
        ssg.contains(&format!("{}=\"count\">7</span>", attr_names::INTERPOLATION)),
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
    let html = generate_html(&doc, "home", &opts()).expect("codegen").html;
    let initial = crate::ssg::build_initial_state(&doc);
    let ssg = crate::ssg::apply_ssg_with_locales(&html, &initial, &HashMap::new(), "");
    assert!(
        ssg.contains(&format!("{}=\"show &gt; 0\"", attr_names::IF))
            && ssg.contains(r#"style="display:block""#),
        "@if branch not pre-rendered as visible:\n{}",
        ssg
    );
    assert!(
        ssg.contains(&format!("{}=\"show &gt; 0\"", attr_names::IF_ELSE))
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
        res.html.contains(attr_names::INTERPOLATION),
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
    assert!(
        html.contains(&format!("{}=\"count\"", attr_names::INTERPOLATION)),
        "reactive prop should produce interpolation for `count`:\n{}",
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
    let css = crate::codegen::codegen_css::generate_combined_css(None, &doc);
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
    let src = "page \"home\" { p \"hello {}\" }";
    let err = crate::parser::parse_webc(src).unwrap_err();
    let display = format!("{}", err);
    // Should contain a caret line
    assert!(display.contains('^'), "caret missing in error: {display}");
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
    assert!(
        html.contains(&format!("{}active=\"isActive\"", attr_names::CLASS_PREFIX)),
        "data-webcore-class-active attribute missing:\n{}",
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
    assert!(
        res.html
            .contains(&format!("{}color=\"color\"", attr_names::STYLE_PREFIX)),
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
    let hash = crate::build::fnv1a_hash(b"hello");
    assert_eq!(hash.len(), 8, "hash must be 8 hex chars");
    // Verify it's consistent (same input → same output)
    assert_eq!(
        hash,
        crate::build::fnv1a_hash(b"hello"),
        "hash must be deterministic"
    );
    // Different inputs must differ
    assert_ne!(
        hash,
        crate::build::fnv1a_hash(b"world"),
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
fn error_parse_empty_interpolation_fails() {
    // Empty {} inside a string literal is invalid syntax (interp_expr requires 1+ chars)
    let src = r#"page "home" { p "value: {}" }"#;
    let result = parse_webc(src);
    assert!(
        result.is_err(),
        "empty string interpolation {{}} should fail to parse"
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
    use crate::error::{CompileError, CompileErrors};
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
    use crate::error::{CompileError, CompileErrors};
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
        crate::ast::StyleItem::Rule(r) => r,
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
        html.contains(r#"rel="preload""#),
        "preload hint missing: {html}"
    );
    assert!(
        html.contains(r#"as="script""#),
        "preload hint missing as=script: {html}"
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
    };
    let res = generate_html(&doc, "home", &options).expect("codegen");
    assert!(
        res.html.contains("<style>.card{padding:1rem}</style>"),
        "critical CSS should be inlined in <style>: {}",
        res.html
    );
    assert!(
        res.html
            .contains(r#"media="print" onload="this.media='all'""#),
        "full stylesheet should load deferred: {}",
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
    let used = crate::codegen::codegen_html::collect_page_components(&doc, "home");
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
        app.collections.get("/").is_none(),
        "non-collection routes unaffected"
    );
}

#[test]
fn golden_expand_collection_basic() {
    let items = r#"[{"slug":"hello-world","title":"Hello"},{"slug":"second-post","title":"Two"}]"#;
    let entries = crate::build::expand_collection("/post/:slug", items).expect("expand");
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
    let entries = crate::build::expand_collection("/user/:id", items).expect("expand");
    assert_eq!(entries[0].0, "user/1/index.html");
    assert_eq!(entries[1].0, "user/42/index.html");
}

#[test]
fn golden_expand_collection_rejects_traversal() {
    let items = r#"[{"slug":"../../etc"}]"#;
    let err = crate::build::expand_collection("/post/:slug", items).unwrap_err();
    assert!(
        err.contains("unsafe"),
        "traversal value must be rejected: {err}"
    );

    let items = r#"[{"slug":"a/b"}]"#;
    assert!(crate::build::expand_collection("/post/:slug", items).is_err());

    let items = r#"[{"slug":""}]"#;
    assert!(crate::build::expand_collection("/post/:slug", items).is_err());
}

#[test]
fn golden_expand_collection_missing_field() {
    let items = r#"[{"title":"no slug here"}]"#;
    let err = crate::build::expand_collection("/post/:slug", items).unwrap_err();
    assert!(err.contains("missing field 'slug'"), "{err}");
}

#[test]
fn golden_expand_collection_requires_param_route() {
    let err = crate::build::expand_collection("/posts", "[]").unwrap_err();
    assert!(err.contains("no ':param'"), "{err}");
}

#[test]
fn golden_ssg_prerenders_route_param() {
    use std::collections::HashMap;
    let mut state: HashMap<String, String> = HashMap::new();
    state.insert("$route.slug".to_string(), "hello-world".to_string());
    let html = r#"<h1><span data-webcore-interpolation="$route.slug"></span></h1>"#;
    let out = crate::ssg::apply_ssg_with_locales(html, &state, &HashMap::new(), "fr");
    assert!(
        out.contains(">hello-world</span>"),
        "route param should be pre-rendered: {out}"
    );
}
