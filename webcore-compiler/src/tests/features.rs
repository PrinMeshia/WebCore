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

#[test]
fn golden_import_decl_parsed() {
    let src = r#"
import posts from "data/posts.json"
layout MainLayout { main { slot content } }
page "home" { p "hello" }
"#;
    let doc = parse_webc(src).expect("parse");
    assert_eq!(doc.imports.len(), 1, "expected 1 import");
    let imp = &doc.imports[0];
    assert_eq!(imp.name, "posts");
    assert_eq!(imp.path, "data/posts.json");
}

#[test]
fn golden_import_data_emits_setq_in_js() {
    let src = r#"
import posts from "data/posts.json"
layout MainLayout { main { slot content } }
component PostList {
    view { @for post in posts { li "{post}" } }
}
page "home" { PostList {} }
"#;
    let mut doc = parse_webc(src).expect("parse");
    // Simulate resolved data import (normally done by build.rs)
    doc.data_imports.insert("posts".to_string(), r#"["hello","world"]"#.to_string());
    let res = crate::codegen::html::generate_html(&doc, "home", &opts()).expect("codegen");
    let js = generate_runtime_js(&res.handlers, &doc);
    assert!(
        js.contains("S.setQ('posts',"),
        "expected S.setQ('posts',...) in JS:\n{}", js
    );
    assert!(
        js.contains(r#"["hello","world"]"#),
        "expected JSON array in JS:\n{}", js
    );
}

#[test]
fn golden_for_list_transition_emits_data_attr() {
    let (html, js) = compile_full(r#"
layout MainLayout { main { slot content } }
component ItemList {
    state { items: Array }
    view {
        @for item key={item} webc:transition="fade" in items {
            li "{item}"
        }
    }
}
page "home" { ItemList {} }
"#);
    assert!(
        html.contains("data-webcore-for-transition=\"fade\""),
        "expected data-webcore-for-transition in HTML:\n{}", html
    );
    assert!(
        js.contains("webc-list-'+tr+'-enter"),
        "expected list enter transition template in JS:\n{}", js
    );
    assert!(
        js.contains("webc-list-'+tr+'-leave"),
        "expected list leave transition template in JS:\n{}", js
    );
}

#[test]
fn golden_watch_store_var_emits_store_on() {
    let js = compile_to_js(r#"
store { theme: String }
component ThemeWatcher {
    $watch $store.theme => {
        document.body.dataset.theme = theme;
    }
    view { p "Watching theme" }
}
"#);
    assert!(
        js.contains("STORE.on('theme'"),
        "expected STORE.on('theme',...) in JS:\n{}", js
    );
    assert!(
        !js.contains("S.on('theme'"),
        "should use STORE.on not S.on for store watch:\n{}", js
    );
}

#[test]
fn golden_for_range_expands_to_array() {
    let (html, _) = compile_full(r#"
layout MainLayout { main { slot content } }
page "home" {
    @for i in 0..5 {
        li "{i}"
    }
}
"#);
    // Range 0..5 should be expanded to [0,1,2,3,4] inline in the template attribute
    assert!(
        html.contains("[0,1,2,3,4]"),
        "expected expanded range array in HTML:\n{}", html
    );
    assert!(
        !html.contains("0..5"),
        "raw range syntax should not appear in output:\n{}", html
    );
}
