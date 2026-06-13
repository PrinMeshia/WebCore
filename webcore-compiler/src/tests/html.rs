//! Tests that verify HTML output.

#[cfg(test)]
use super::*;

#[test]
fn golden_page_heading_renders() {
    let html = compile_to_html(r#"
layout MainLayout { main { slot content } }
page "home" { h1 "Hello WebCore!" }
"#);
    assert!(html.contains("<h1>Hello WebCore!"), "h1 missing:\n{}", html);
    assert!(html.contains("</h1>"), "h1 close tag missing");
    assert!(html.contains("lang=\"en\""));
    assert!(html.contains("<title>Test</title>"));
}

#[test]
fn golden_state_interpolation_emits_span() {
    let html = compile_to_html(r#"
layout MainLayout { main { slot content } }
component Counter {
    state { count: Number = 0 }
    view { p "Value: {count}" }
}
page "home" { Counter {} }
"#);
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
    assert!(res.html.contains("onclick="), "onclick missing:\n{}", res.html);
    assert!(!res.handlers.is_empty(), "no handlers registered");
    let js = generate_runtime_js(&res.handlers, &doc);
    assert!(
        js.contains("S.get('count')+1") || js.contains("S.get(&#x27;count&#x27;)"),
        "compiled expression missing in JS:\n{}",
        js
    );
}

#[test]
fn golden_validate_attrs_converted_to_data_attrs() {
    let html = compile_to_html(r#"
layout MainLayout { main { slot content } }
page "home" {
    form {
        input type="text" name="username"
              validate:required="Le nom est requis"
              validate:minlength="3,Au moins 3 caractères"
    }
}
"#);
    assert!(html.contains("data-webcore-field=\"username\""), "data-webcore-field missing:\n{}", html);
    assert!(html.contains("data-webcore-validate-required=\"Le nom est requis\""), "validate-required missing:\n{}", html);
    assert!(html.contains("data-webcore-validate-minlength=\"3\""), "validate-minlength missing:\n{}", html);
    assert!(html.contains("data-webcore-validate-minlength-msg="), "validate-minlength-msg missing:\n{}", html);
    assert!(!html.contains("validate:required"), "raw validate: attr should not appear in output:\n{}", html);
}

#[test]
fn golden_error_block_renders_hidden_div() {
    let html = compile_to_html(r#"
layout MainLayout { main { slot content } }
page "home" {
    form {
        input type="text" name="email" validate:email="Email invalide"
        @error "email" { span "Erreur email" }
    }
}
"#);
    assert!(html.contains(&format!("{}=\"email\"", attr_names::ERROR)), "data-webcore-error missing:\n{}", html);
    assert!(html.contains("style=\"display:none\""), "error block should be hidden by default:\n{}", html);
}

#[test]
fn golden_prop_substituted_in_view() {
    let html = compile_to_html(r#"
layout MainLayout { main { slot content } }
component Greeting {
    props { name: String }
    view { p "Hello {name}!" }
}
page "home" { Greeting name="WebCore" {} }
"#);
    assert!(html.contains("Hello"), "Hello fragment missing:\n{}", html);
    assert!(html.contains("WebCore"), "prop value missing:\n{}", html);
    assert!(!html.contains(&format!("{}=\"name\"", attr_names::INTERPOLATION)), "unresolved prop span still present:\n{}", html);
}

#[test]
fn golden_reactive_prop_stays_interpolation() {
    let html = compile_to_html(r#"
layout MainLayout { main { slot content } }
component Badge {
    props { label: String }
    view { span "Valeur : {label}" }
}
page "home" {
    Badge label={count} {}
}
"#);
    assert!(html.contains(&format!("{}=\"count\"", attr_names::INTERPOLATION)), "reactive prop should produce interpolation for `count`:\n{}", html);
    assert!(!html.contains(&format!("{}=\"label\"", attr_names::INTERPOLATION)), "unresolved `label` span should not appear:\n{}", html);
}

#[test]
fn golden_static_prop_still_substituted() {
    let html = compile_to_html(r#"
layout MainLayout { main { slot content } }
component Greeting {
    props { name: String }
    view { p "Hello {name}!" }
}
page "home" { Greeting name="Alice" {} }
"#);
    assert!(html.contains("Alice"), "static prop value missing:\n{}", html);
    assert!(!html.contains(&format!("{}=\"name\"", attr_names::INTERPOLATION)), "unresolved name span should not appear:\n{}", html);
}

#[test]
fn golden_named_slot_filled_from_page() {
    let html = compile_to_html(r#"
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
"#);
    assert!(html.contains("<h1>Titre"), "named slot header content missing:\n{}", html);
    assert!(html.contains("<p>Contenu principal"), "default content slot missing:\n{}", html);
}

#[test]
fn golden_unnamed_slot_backwards_compat() {
    let html = compile_to_html(r#"
layout MainLayout { main { slot content } }
page "home" { h1 "Simple" }
"#);
    assert!(html.contains("<h1>Simple"), "backward-compat unnamed slot broken:\n{}", html);
}

#[test]
fn golden_compound_prop_substituted() {
    let html = compile_to_html(r#"
layout MainLayout { main { slot content } }
component Counter {
    props { step }
    view { p "{step + 1}" }
}
page "home" { Counter step="2" {} }
"#);
    assert!(html.contains("(2) + 1"), "compound prop not substituted: {}", html);
}

#[test]
fn golden_prop_in_attribute_substituted() {
    let html = compile_to_html(r#"
layout MainLayout { main { slot content } }
component Badge {
    props { color }
    view { span class={color} "ok" }
}
page "home" { Badge color="green" {} }
"#);
    assert!(html.contains("class=\"green\""), "prop not substituted in attribute: {}", html);
}

#[test]
fn golden_for_key_emits_data_attribute() {
    let html = compile_to_html(r#"
layout MainLayout { main { slot content } }
page "home" {
    @for item key=item in items {
        li "{item}"
    }
}
"#);
    assert!(html.contains(&format!("{}=\"item\"", attr_names::FOR_KEY)), "for-key attribute missing: {}", html);
}

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
    assert!(res.html.contains("data-webcore-attr-value"), "dynamic value binding missing:\n{}", res.html);
    assert!(
        res.html.contains("username = event.target.value") ||
        res.handlers.iter().any(|h| h.expression.contains("username = event.target.value")),
        "on:input handler missing:\n{:?}", res.handlers
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
        res.html.contains("data-webcore-attr-checked") ||
        res.handlers.iter().any(|h| h.expression.contains("event.target.checked")),
        "checked binding missing:\n{}", res.html
    );
}

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
        res.html.contains(&format!("{}=\"searchInput\"", attr_names::REF)),
        "data-webcore-ref missing:\n{}", res.html
    );
    let js = generate_runtime_js(&res.handlers, &doc);
    assert!(js.contains("const refs={}"), "refs object missing in JS:\n{}", js);
    assert!(js.contains(attr_names::REF), "refs population code missing in JS:\n{}", js);
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
        res.html.contains(&format!("{}color=\"color\"", attr_names::STYLE_PREFIX)),
        "data-webcore-style-color missing:\n{}", res.html
    );
    let js = generate_runtime_js(&res.handlers, &doc);
    assert!(js.contains(attr_names::STYLE_PREFIX), "style binding JS code missing:\n{}", js);
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
    };
    let res = generate_html(&doc, "dash", &opts_dash).expect("codegen");
    assert!(res.html.contains("Default nav"), "slot default content missing:\n{}", res.html);
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
        res.html.contains(&format!("{}=\"fade\"", attr_names::TRANSITION)),
        "data-webcore-transition missing:\n{}", res.html
    );
    let js = generate_runtime_js(&res.handlers, &doc);
    assert!(js.contains("webc-fade-enter"), "transition CSS classes missing in JS:\n{}", js);
}

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
    assert!(res.html.contains("loading=\"lazy\""), "loading=lazy missing:\n{}", res.html);
    assert!(res.html.contains("decoding=\"async\""), "decoding=async missing:\n{}", res.html);
    assert!(!res.html.contains("webc:img"), "webc:img directive leaked into output:\n{}", res.html);
}

#[test]
fn golden_webc_img_missing_alt_does_not_crash() {
    let src = r#"
layout MainLayout { main { slot content } }
page "home" {
    img webc:img=true src="/photo.png"
}
"#;
    let doc = parse_webc(src).expect("parse");
    let res = generate_html(&doc, "home", &opts()).expect("codegen");
    assert!(res.html.contains("<img"), "img tag missing from output:\n{}", res.html);
    assert!(res.html.contains("loading=\"lazy\""), "loading=lazy missing in no-alt case:\n{}", res.html);
}
