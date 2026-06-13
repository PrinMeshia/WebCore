//! Feature-specific tests (head block, switch, etc.).

#[cfg(test)]
use super::*;

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
    let page = doc.pages.get("article").expect("page exists");
    let head = page.head.as_ref().expect("head block present");
    assert_eq!(head.title.as_deref(), Some("Mon Article"), "title mismatch");
    assert_eq!(head.metas.len(), 2, "expected 2 meta tags");
    assert!(
        head.metas.iter().any(|(k, v)| k == "description" && v == "Article de blog WebCore"),
        "description meta missing: {:?}", head.metas
    );
    assert!(
        head.metas.iter().any(|(k, v)| k == "og:title" && v == "Mon Article"),
        "og:title meta missing: {:?}", head.metas
    );
}

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
    assert!(js.contains("bindIf"), "bindIf missing — @switch should expand to @if chain:\n{js}");
    assert!(
        html.contains("status === &quot;active&quot;") || html.contains("status === \"active\""),
        "case condition missing in HTML:\n{}", html
    );
    assert!(html.contains("Active"), "case content missing");
    assert!(html.contains("Unknown"), "default content missing");
}

#[test]
fn golden_switch_without_default() {
    let html = compile_to_html(r#"
layout MainLayout { main { slot content } }
page "home" {
    @switch role {
        @case "admin" { span "Admin panel" }
        @case "user"  { span "User view" }
    }
}
"#);
    assert!(html.contains("Admin panel"), "admin case missing");
    assert!(html.contains("User view"), "user case missing");
}

#[test]
fn golden_class_binding_emits_data_attr() {
    let (html, js) = compile_full(r#"
layout MainLayout { main { slot content } }
component Toggle {
    state { isActive: Boolean = false }
    view {
        div class:active={isActive} { "content" }
    }
}
page "home" { Toggle {} }
"#);
    assert!(html.contains(&format!("{}active=\"isActive\"", attr_names::CLASS_PREFIX)), "data-webcore-class-active attribute missing:\n{}", html);
    assert!(!html.contains("class:active"), "raw class:active should not appear in output:\n{}", html);
    assert!(js.contains("bindClassBindings"), "bindClassBindings missing in JS:\n{}", js);
}
