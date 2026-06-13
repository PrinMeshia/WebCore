//! Security-focused tests: path traversal, nesting bombs, XSS, injection.

#[cfg(test)]
use super::*;

// ── Nesting depth guard ──────────────────────────────────────────────────────

#[test]
fn security_nesting_bomb_rejected() {
    // Build a deeply nested .webc string that exceeds MAX_DEPTH (128).
    let mut src = String::from("layout MainLayout { main { slot content } }\npage \"home\" {\n");
    for _ in 0..150 {
        src.push_str("div {\n");
    }
    src.push_str("\"leaf\"\n");
    for _ in 0..150 {
        src.push('}');
        src.push('\n');
    }
    src.push('}'); // close page

    let result = parse_webc(&src);
    assert!(
        result.is_err(),
        "Expected parse error for nesting depth >128, got Ok"
    );
    let msg = result.unwrap_err().message;
    assert!(
        msg.contains("nesting") || msg.contains("depth"),
        "Error should mention nesting depth: {msg}"
    );
}

#[test]
fn security_nesting_within_limit_ok() {
    // 50 levels deep: well within the 128 limit.
    let mut src = String::from("layout MainLayout { main { slot content } }\npage \"home\" {\n");
    for _ in 0..50 {
        src.push_str("div {\n");
    }
    src.push_str("\"leaf\"\n");
    for _ in 0..50 {
        src.push('}');
        src.push('\n');
    }
    src.push('}');

    let result = parse_webc(&src);
    assert!(result.is_ok(), "50-level nesting should be accepted: {:?}", result.err());
}

// ── HTML output escaping (XSS prevention) ───────────────────────────────────

#[test]
fn security_xss_in_text_node_is_escaped() {
    // A text node containing HTML special chars must be escaped in output.
    let src = r#"
layout MainLayout { main { slot content } }
page "home" {
    p "<script>alert(1)</script>"
}
"#;
    let html = compile_to_html(src);
    assert!(
        !html.contains("<script>alert(1)</script>"),
        "Raw <script> tag must not appear unescaped in output:\n{html}"
    );
    assert!(
        html.contains("&lt;script&gt;") || html.contains("&#60;script&#62;"),
        "Expected HTML-escaped <script> in output:\n{html}"
    );
}

#[test]
fn security_xss_in_attribute_value_is_escaped() {
    let src = r#"
layout MainLayout { main { slot content } }
page "home" {
    a href="\" onclick=\"alert(1)" "click"
}
"#;
    // The href value should be HTML-escaped so the injected onclick doesn't leak.
    let html = compile_to_html(src);
    assert!(
        !html.contains("onclick=\"alert(1)\""),
        "Injected onclick must not appear unescaped in attribute:\n{html}"
    );
}

#[test]
fn security_xss_interpolation_in_static_html_is_escaped() {
    // At SSG time, static string literals with HTML entities must be escaped.
    let src = r#"
layout MainLayout { main { slot content } }
page "home" {
    div "<b>bold</b>"
}
"#;
    let html = compile_to_html(src);
    assert!(
        !html.contains("<b>bold</b>"),
        "<b> in static string should be escaped:\n{html}"
    );
}

// ── Import path safety ───────────────────────────────────────────────────────
// Note: path-traversal prevention lives in cli/build.rs (canonicalize check).
// We test the parser-level import parsing is neutral (no traversal at parse time).

#[test]
fn security_import_path_traversal_parsed_but_noted() {
    // The parser accepts any string for the import path (path validation happens
    // at build time in build.rs). This test just verifies the parser doesn't panic
    // and that the path is stored verbatim for later sanitization.
    let src = r#"
import secret from "../../etc/passwd"
layout MainLayout { main { slot content } }
page "home" { p "hi" }
"#;
    let doc = parse_webc(src).expect("parser should not panic on traversal path");
    // The path is stored as-is; build.rs will reject it at build time.
    assert_eq!(doc.imports[0].path, "../../etc/passwd");
}

// ── JS output: no eval / Function constructor ────────────────────────────────

#[test]
fn security_generated_js_uses_new_function_not_eval() {
    // The runtime uses new Function() for expression evaluation (evalCond).
    // It must NOT use bare eval() which is even harder to sandbox.
    let (_, js) = compile_full(r#"
layout MainLayout { main { slot content } }
component Counter {
    state { count: Int = 0 }
    view { button on:click={count += 1} "{count}" }
}
page "home" { Counter {} }
"#);
    // "evalCond" is an internal helper — its name contains "eval" but NOT "eval(";
    // the substring "eval(" must not appear anywhere in the generated output.
    assert!(
        !js.contains("eval("),
        "Generated JS must not contain bare eval():\n{js}"
    );
}

#[test]
fn security_handler_ids_are_alphanumeric_only() {
    // Event handler IDs embedded in onclick="..." must be alphanumeric only
    // to prevent JS injection through the handler ID.
    let html = compile_to_html(r#"
layout MainLayout { main { slot content } }
component Btn {
    view { button on:click={doSomething()} "click" }
}
page "home" { Btn {} }
"#);
    // Extract the handler ID from onclick="webcore_handle_click('ID')"
    if let Some(start) = html.find("webcore_handle_click('") {
        let rest = &html[start + "webcore_handle_click('".len()..];
        if let Some(end) = rest.find('\'') {
            let handler_id = &rest[..end];
            assert!(
                handler_id.chars().all(|c| c.is_alphanumeric()),
                "Handler ID '{handler_id}' contains non-alphanumeric chars"
            );
        }
    }
}
