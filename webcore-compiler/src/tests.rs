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
            view: vec![],
            style: vec![crate::ast::StyleRule {
                selector: "button".into(),
                properties: vec![crate::ast::StyleProperty {
                    name: "color".into(),
                    value: "red".into(),
                    span: Span::default(),
                }],
                span: Span::default(),
            }],
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
page "home" { h1 "hi" }
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
        js.contains("webcoreValidateRequired"),
        "required check missing in runtime"
    );
    assert!(
        js.contains("webcoreValidateEmail"),
        "email check missing in runtime"
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
