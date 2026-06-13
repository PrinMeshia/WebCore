//! i18n (internationalisation) tests.

#[cfg(test)]
use super::*;

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
    assert!(js.contains("const setLocale="), "setLocale missing:\n{}", js);
    assert!(js.contains("setLocale"), "setLocale not exported:\n{}", js);
}

#[test]
fn golden_i18n_ssg_prerender() {
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
    assert!(res.html.contains(attr_names::INTERPOLATION), "no interpolation span:\n{}", res.html);
    let state = crate::core::ssg::build_initial_state(&doc);
    let ssg = crate::core::ssg::apply_ssg_with_locales(&res.html, &state, &doc.locales, "fr");
    assert!(ssg.contains("Bienvenue"), "translation not pre-rendered:\n{}", ssg);
}

#[test]
fn golden_i18n_no_locales_runtime_omits_t() {
    let js = compile_to_js(r#"
layout MainLayout { main { slot content } }
page "home" { h1 "hi" }
"#);
    assert!(!js.contains("const LOCALES="), "LOCALES should be absent when no locales:\n{}", js);
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
    assert!(js.contains("typeof a==='number'"), "plural t() not emitted: {js}");
    assert!(js.contains("_one"), "plural key suffix _one missing: {js}");
}
