//! JavaScript Code Generator for WebCore Runtime
//!
//! ## Runtime naming conventions
//!
//! The generated JavaScript uses short names intentionally — the runtime is
//! embedded verbatim in every built page and the names survive minification:
//!
//! | Name    | Meaning                              |
//! |---------|--------------------------------------|
//! | `S`     | Local component `State` instance     |
//! | `STORE` | Global shared `State` instance       |
//! | `U`     | Math utilities (`max`, `min`, `abs`) |
//! | `VARS`  | Array of local state variable names  |
//! | `STORE_VARS` | Array of global store var names |
//!
//! ## Block-scoping constraint
//!
//! The entire runtime is wrapped in a `{}` JS block so that `const` declarations
//! do not pollute `globalThis`.  As a consequence, `new Function(body)` cannot
//! see block-scoped bindings — `S`, `STORE`, `U` and `t` must therefore be
//! passed explicitly as named parameters whenever a dynamic `Function` is
//! constructed (see `evalCond`).
//!
//! ## Tree-shaking
//!
//! `generate_runtime_js_with_vars` calls `detect_features()` to inspect the
//! document AST and emits each runtime helper only when it is actually needed.
//! Simple pages (no `@if`, no `@for`, no validation) get a runtime of ~300 bytes.

use crate::ast::*;
use crate::codegen::codegen_html::HandlerMapping;
use regex::Regex;
use std::collections::HashSet;

/// Component-level event listener: emitted by `on:eventName={expr}` on a component call.
pub struct EventListenerMapping {
    pub event_name: String,
    pub expression: String,
}

/// Collect all local state variable names from a document (component-level).
/// Includes computed var names so that `evalCond('doubled')` resolves to
/// `S.get('doubled')` rather than a bare identifier that throws ReferenceError.
pub fn collect_state_variables(document: &WebCoreDocument) -> HashSet<String> {
    let mut vars = HashSet::new();
    for component in document.components.values() {
        for state_var in &component.state {
            vars.insert(state_var.name.clone());
        }
        for computed_var in &component.computed {
            vars.insert(computed_var.name.clone());
        }
    }
    vars
}

/// Collect global store variable names
pub fn collect_store_variables(document: &WebCoreDocument) -> HashSet<String> {
    document.store.iter().map(|v| v.name.clone()).collect()
}

/// Replace `$store.varname` (sentinel: `__STORE_varname__`) then local vars, then restore.
/// The sentinel approach prevents local-var replacement from touching store references.
fn replace_store_and_local(expr: &str, state_vars: &HashSet<String>) -> String {
    let re_store = Regex::new(r"\$store\.([a-zA-Z_][a-zA-Z0-9_]*)").unwrap();
    // Step 1: sentinel so local-var replacement can't touch store refs
    let sentineled = re_store.replace_all(expr, "__STORE_$1__").to_string();
    // Step 2: replace local state vars  (_ is a word char → \bcount\b won't match inside __STORE_count__)
    let with_local = replace_vars_short(&sentineled, state_vars);
    // Step 3: restore sentinels → STORE.get('...')
    let re_sent = Regex::new(r"__STORE_([a-zA-Z_][a-zA-Z0-9_]*)__").unwrap();
    re_sent
        .replace_all(&with_local, "STORE.get('$1')")
        .to_string()
}

/// Parse `$store.identifier op= value`
fn parse_store_compound_assign(expr: &str, op: &str) -> Option<(String, String)> {
    let trimmed = expr.trim_start();
    if !trimmed.starts_with("$store.") {
        return None;
    }
    let rest = trimmed["$store.".len()..].trim_start();
    let pos = rest.find(op)?;
    let var_name = rest[..pos].trim().to_string();
    let value = rest[pos + op.len()..].trim().to_string();
    if !var_name.is_empty()
        && var_name.chars().all(|c| c.is_alphanumeric() || c == '_')
        && !value.is_empty()
    {
        Some((var_name, value))
    } else {
        None
    }
}

/// Parse `$store.identifier = value`
fn parse_store_simple_assign(expr: &str) -> Option<(String, String)> {
    let trimmed = expr.trim_start();
    if !trimmed.starts_with("$store.") {
        return None;
    }
    let rest = trimmed["$store.".len()..].trim_start();
    let (var_name, value) = parse_simple_assign(rest)?;
    if !var_name.contains('.') && !var_name.contains('$') {
        Some((var_name, value))
    } else {
        None
    }
}

/// Full expression compiler: handles both `$store.var` and local state vars.
fn compile_expression_full(expr: &str, state_vars: &HashSet<String>) -> String {
    let compiled = expr.trim();

    // Multi-statement: split on ; and compile each independently
    // e.g. `items = [...items, draft]; draft = ""`
    if compiled.contains(';') {
        let parts: Vec<&str> = compiled.split(';').map(str::trim).filter(|s| !s.is_empty()).collect();
        if parts.len() > 1 {
            return parts
                .iter()
                .map(|s| compile_expression_full(s, state_vars))
                .collect::<Vec<_>>()
                .join(";");
        }
    }

    // Store compound assigns: $store.count += 1
    if let Some((var, val)) = parse_store_compound_assign(compiled, "+=") {
        let rhs = replace_utils_short(&replace_store_and_local(&val, state_vars));
        return format!("STORE.set('{}',STORE.get('{}')+{})", var, var, rhs);
    }
    if let Some((var, val)) = parse_store_compound_assign(compiled, "-=") {
        let rhs = replace_utils_short(&replace_store_and_local(&val, state_vars));
        return format!("STORE.set('{}',STORE.get('{}')-{})", var, var, rhs);
    }
    if let Some((var, val)) = parse_store_compound_assign(compiled, "*=") {
        let rhs = replace_utils_short(&replace_store_and_local(&val, state_vars));
        return format!("STORE.set('{}',STORE.get('{}')*{})", var, var, rhs);
    }
    if let Some((var, val)) = parse_store_compound_assign(compiled, "/=") {
        let rhs = replace_utils_short(&replace_store_and_local(&val, state_vars));
        return format!("STORE.set('{}',STORE.get('{}')/{})", var, var, rhs);
    }
    // Store simple assign: $store.count = value
    if let Some((var, val)) = parse_store_simple_assign(compiled) {
        let rhs = replace_utils_short(&replace_store_and_local(&val, state_vars));
        return format!("STORE.set('{}',{})", var, rhs);
    }

    // Local state compound assigns: count += 1
    if let Some((var, val)) = parse_compound_assign(compiled, "+=") {
        return format!(
            "S.set('{}',S.get('{}')+{})",
            var,
            var,
            replace_store_and_local(&val, state_vars)
        );
    }
    if let Some((var, val)) = parse_compound_assign(compiled, "-=") {
        return format!(
            "S.set('{}',S.get('{}')-{})",
            var,
            var,
            replace_store_and_local(&val, state_vars)
        );
    }
    if let Some((var, val)) = parse_compound_assign(compiled, "*=") {
        return format!(
            "S.set('{}',S.get('{}')*{})",
            var,
            var,
            replace_store_and_local(&val, state_vars)
        );
    }
    if let Some((var, val)) = parse_compound_assign(compiled, "/=") {
        return format!(
            "S.set('{}',S.get('{}')/{})",
            var,
            var,
            replace_store_and_local(&val, state_vars)
        );
    }
    // Local state simple assign: count = max(0, count - 1)
    if let Some((var, val)) = parse_simple_assign(compiled) {
        if state_vars.contains(&var) {
            let replaced = replace_utils_short(&replace_store_and_local(&val, state_vars));
            return format!("S.set('{}',{})", var, replaced);
        }
    }
    // Navigation
    if compiled.contains("webcore_navigate(") {
        return compile_navigate_call(compiled);
    }
    // emit("eventName") / emit("eventName", data) — inter-component events
    if compiled.contains("emit(") {
        return compile_emit_call(compiled, state_vars);
    }
    // Default: replace all variables and utils
    let result = replace_store_and_local(compiled, state_vars);
    replace_utils_short(&result)
}

/// Compile emit("eventName") or emit("eventName", data) to CustomEvent dispatch.
fn compile_emit_call(expr: &str, state_vars: &HashSet<String>) -> String {
    if let Some(start) = expr.find("emit(") {
        let inner_start = start + 5;
        if let Some(paren_end) = expr[inner_start..].rfind(')') {
            let args_str = &expr[inner_start..inner_start + paren_end];
            let parts: Vec<&str> = args_str.splitn(2, ',').collect();
            let event_name = parts[0].trim();
            if parts.len() == 1 || parts[1].trim().is_empty() {
                return format!("document.dispatchEvent(new CustomEvent({}))", event_name);
            } else {
                let detail_raw = parts[1].trim();
                let detail = replace_utils_short(&replace_store_and_local(detail_raw, state_vars));
                return format!(
                    "document.dispatchEvent(new CustomEvent({},{{detail:{}}}))",
                    event_name, detail
                );
            }
        }
    }
    let result = replace_store_and_local(expr, state_vars);
    replace_utils_short(&result)
}

/// Parse compound assignment like "count += 1"
fn parse_compound_assign(expr: &str, operator: &str) -> Option<(String, String)> {
    if let Some(pos) = expr.find(operator) {
        let var_name = expr[..pos].trim().to_string();
        let value = expr[pos + operator.len()..].trim().to_string();
        // Validate variable name (simple identifier)
        if var_name.chars().all(|c| c.is_alphanumeric() || c == '_') && !var_name.is_empty() {
            return Some((var_name, value));
        }
    }
    None
}

/// Parse simple assignment like "count = value" (but not ==, !=, <=, >=)
fn parse_simple_assign(expr: &str) -> Option<(String, String)> {
    // Find = that is not part of ==, !=, <=, >=, +=, -=, *=, /=
    let chars: Vec<char> = expr.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c == '=' {
            // Check previous char
            if i > 0 {
                let prev = chars[i - 1];
                if prev == '='
                    || prev == '!'
                    || prev == '<'
                    || prev == '>'
                    || prev == '+'
                    || prev == '-'
                    || prev == '*'
                    || prev == '/'
                {
                    continue;
                }
            }
            // Check next char
            if i + 1 < chars.len() && chars[i + 1] == '=' {
                continue;
            }

            let var_name = expr[..i].trim().to_string();
            let value = expr[i + 1..].trim().to_string();

            // Validate variable name
            if var_name.chars().all(|c| c.is_alphanumeric() || c == '_') && !var_name.is_empty() {
                return Some((var_name, value));
            }
        }
    }
    None
}

/// Replace variable references with short getter (S.get)
fn replace_vars_short(expr: &str, state_vars: &HashSet<String>) -> String {
    let mut result = expr.to_string();
    let mut vars: Vec<_> = state_vars.iter().collect();
    vars.sort_by_key(|v| std::cmp::Reverse(v.len()));

    for var in vars {
        let pattern = format!(r"\b{}\b", regex::escape(var));
        if let Ok(re) = Regex::new(&pattern) {
            result = re
                .replace_all(&result, format!("S.get('{}')", var).as_str())
                .to_string();
        }
    }
    result
}

/// Replace utility functions with short aliases (U.max, etc.)
fn replace_utils_short(expr: &str) -> String {
    let mut result = expr.to_string();
    for (old, new) in [("max(", "U.max("), ("min(", "U.min("), ("abs(", "U.abs(")] {
        if !result.contains(&format!("U.{}", old)) {
            result = result.replace(old, new);
        }
    }
    result
}

/// Convert a route pattern `/post/:slug` to a JS regex string `^\/post\/([^\/]+)$`
/// and return the list of param names in capture order.
fn route_to_js_regex(pattern: &str) -> (String, Vec<String>) {
    if pattern == "/" {
        return ("^\\/$".to_string(), vec![]);
    }
    let mut params = Vec::new();
    let mut regex = String::from("^");
    for segment in pattern.split('/') {
        if segment.is_empty() {
            continue;
        }
        if let Some(name) = segment.strip_prefix(':') {
            params.push(name.to_string());
            regex.push_str("\\/([^\\/]+)");
        } else {
            regex.push_str("\\/");
            for c in segment.chars() {
                match c {
                    '.' | '+' | '*' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '\\' | '^'
                    | '$' | '|' => {
                        regex.push('\\');
                        regex.push(c);
                    }
                    _ => regex.push(c),
                }
            }
        }
    }
    regex.push('$');
    (regex, params)
}

/// Convert a component/page name to the expected HTML filename (mirrors main.rs logic).
fn page_name_to_file(name: &str) -> String {
    let route = name
        .to_lowercase()
        .replace("page", "")
        .replace("home", "index");
    if route.is_empty() || route == "index" {
        "index.html".to_string()
    } else {
        format!("{}/index.html", route)
    }
}

/// Compile webcore_navigate calls to short nav() function
fn compile_navigate_call(expr: &str) -> String {
    if let Some(start) = expr.find("webcore_navigate(") {
        if let Some(end) = expr[start..].find(')') {
            let path = expr[start + 17..start + end].trim();
            if path == "root" {
                return "nav('/')".to_string();
            }
            if path.starts_with('"') {
                return format!("nav({})", path);
            }
            if path.starts_with('/') {
                return format!("nav('{}')", path);
            }
            return format!("nav('/{}')", path);
        }
    }
    expr.to_string()
}

/// Collect on:mount bodies from all components (raw JS to run at DOMContentLoaded).
fn collect_on_mount_bodies(document: &WebCoreDocument) -> Vec<String> {
    document
        .components
        .values()
        .filter_map(|c| c.mount_body.as_ref())
        .filter(|b| !b.trim().is_empty())
        .cloned()
        .collect()
}

/// Collect on:destroy bodies from all components.
fn collect_on_destroy_bodies(document: &WebCoreDocument) -> Vec<String> {
    document
        .components
        .values()
        .filter_map(|c| c.destroy_body.as_ref())
        .filter(|b| !b.trim().is_empty())
        .cloned()
        .collect()
}

/// Features detected by walking the document AST — drives tree-shaking.
#[derive(Default)]
struct RuntimeFeatures {
    has_interpolation: bool,
    has_if: bool,
    has_for: bool,
    has_dynamic_attrs: bool,
    has_validation: bool,
    has_navigation: bool,
    has_param_routes: bool,
    /// Any component has an `http { }` block
    has_http: bool,
    /// Any expression contains `$query.`
    has_query_params: bool,
    /// Any element attribute starts with `class:`
    has_class_binding: bool,
    /// Any event attribute name contains `|debounce`
    has_debounce: bool,
    /// Any element attribute starts with `ref:`
    has_refs: bool,
    /// Any element attribute starts with `style:`
    has_style_binding: bool,
    /// Any element has a `webc:transition` attribute
    has_transition: bool,
}

fn detect_features_in_elements(elements: &[Element], f: &mut RuntimeFeatures) {
    for elem in elements {
        match elem {
            Element::Interpolation(expr, _) => {
                f.has_interpolation = true;
                if expr.contains("$query.") {
                    f.has_query_params = true;
                }
            }
            Element::For { content, iterable, .. } => {
                f.has_for = true;
                if iterable.contains("$query.") {
                    f.has_query_params = true;
                }
                detect_features_in_elements(content, f);
            }
            Element::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                f.has_if = true;
                if condition.contains("$query.") {
                    f.has_query_params = true;
                }
                detect_features_in_elements(then_branch, f);
                if let Some(eb) = else_branch {
                    detect_features_in_elements(eb, f);
                }
            }
            Element::Tag {
                name,
                attributes,
                content,
                ..
            } => {
                if name == "link" && attributes.iter().any(|a| a.name == "to") {
                    f.has_navigation = true;
                }
                for attr in attributes {
                    if attr.name.starts_with("validate:") {
                        f.has_validation = true;
                    }
                    if attr.name.starts_with("class:") {
                        f.has_class_binding = true;
                    }
                    if attr.name.contains("|debounce") {
                        f.has_debounce = true;
                    }
                    if attr.name.starts_with("ref:") {
                        f.has_refs = true;
                    }
                    if attr.name.starts_with("style:") {
                        f.has_style_binding = true;
                    }
                    if attr.name == "webc:transition" {
                        f.has_transition = true;
                    }
                    match &attr.value {
                        AttributeValue::Expression(expr) => {
                            if !attr.name.starts_with("on:")
                                && !attr.name.starts_with("class:")
                                && !attr.name.starts_with("ref:")
                                && !attr.name.starts_with("style:")
                                && attr.name != "webc:transition"
                            {
                                f.has_dynamic_attrs = true;
                            }
                            if expr.contains("webcore_navigate(") {
                                f.has_navigation = true;
                            }
                            if expr.contains("$query.") {
                                f.has_query_params = true;
                            }
                        }
                        AttributeValue::String(s) => {
                            if s.contains("$query.") {
                                f.has_query_params = true;
                            }
                        }
                        _ => {}
                    }
                }
                detect_features_in_elements(content, f);
            }
            Element::Component { content, .. } => {
                detect_features_in_elements(content, f);
            }
            Element::ErrorBlock { content, .. } => {
                f.has_validation = true;
                detect_features_in_elements(content, f);
            }
            Element::SlotContent { content, .. } => {
                detect_features_in_elements(content, f);
            }
            Element::Text(t, _) => {
                if t.contains("$query.") {
                    f.has_query_params = true;
                }
            }
            Element::Slot(..) => {}
        }
    }
}

fn detect_features(document: &WebCoreDocument) -> RuntimeFeatures {
    let mut f = RuntimeFeatures::default();
    if let Some(app) = &document.app {
        if !app.routes.is_empty() {
            f.has_navigation = true;
        }
        if app.routes.keys().any(|path| path.contains(':')) {
            f.has_param_routes = true;
        }
    }
    for page in document.pages.values() {
        detect_features_in_elements(&page.content, &mut f);
    }
    for component in document.components.values() {
        if component.http.is_some() {
            f.has_http = true;
        }
        detect_features_in_elements(&component.view, &mut f);
    }
    for layout in document.layouts.values() {
        detect_features_in_elements(&layout.content, &mut f);
    }
    f
}

/// Build the semicolon-separated sequence of rebind calls for a given feature set.
fn rebind_seq(f: &RuntimeFeatures, needs_bind: bool) -> String {
    let mut parts: Vec<&str> = Vec::new();
    if needs_bind {
        parts.push("bind()");
    }
    if f.has_if {
        parts.push("bindIf()");
    }
    if f.has_for {
        parts.push("bindFor()");
    }
    if f.has_dynamic_attrs || f.has_style_binding {
        parts.push("bindAttrs()");
    }
    if f.has_class_binding {
        parts.push("bindClassBindings()");
    }
    if f.has_validation {
        parts.push("bindValidation()");
    }
    parts.join(";")
}

/// Walk elements collecting on:eventName={expr} attrs on component calls.
fn collect_event_listeners_from_elements(
    elements: &[Element],
    out: &mut Vec<EventListenerMapping>,
) {
    for elem in elements {
        match elem {
            Element::Component {
                attributes,
                content,
                ..
            } => {
                for attr in attributes {
                    if let Some(event_name) = attr.name.strip_prefix("on:") {
                        if let AttributeValue::Expression(expr) = &attr.value {
                            out.push(EventListenerMapping {
                                event_name: event_name.to_string(),
                                expression: expr.clone(),
                            });
                        }
                    }
                }
                collect_event_listeners_from_elements(content, out);
            }
            Element::Tag { content, .. } => collect_event_listeners_from_elements(content, out),
            Element::For { content, .. } => collect_event_listeners_from_elements(content, out),
            Element::If {
                then_branch,
                else_branch,
                ..
            } => {
                collect_event_listeners_from_elements(then_branch, out);
                if let Some(eb) = else_branch {
                    collect_event_listeners_from_elements(eb, out);
                }
            }
            Element::ErrorBlock { content, .. } => {
                collect_event_listeners_from_elements(content, out)
            }
            Element::SlotContent { content, .. } => {
                collect_event_listeners_from_elements(content, out)
            }
            _ => {}
        }
    }
}

/// Collect component-level event listeners from the full document.
fn collect_component_event_listeners(document: &WebCoreDocument) -> Vec<EventListenerMapping> {
    let mut out = Vec::new();
    for page in document.pages.values() {
        collect_event_listeners_from_elements(&page.content, &mut out);
    }
    for component in document.components.values() {
        collect_event_listeners_from_elements(&component.view, &mut out);
    }
    for layout in document.layouts.values() {
        collect_event_listeners_from_elements(&layout.content, &mut out);
    }
    out
}

pub fn generate_runtime_js(handlers: &[HandlerMapping], document: &WebCoreDocument) -> String {
    let state_vars = collect_state_variables(document);
    let store_vars = collect_store_variables(document);
    generate_runtime_js_with_vars(handlers, &state_vars, &store_vars, document)
}

pub fn generate_runtime_js_with_vars(
    handlers: &[HandlerMapping],
    state_vars: &HashSet<String>,
    store_vars: &HashSet<String>,
    document: &WebCoreDocument,
) -> String {
    let mut unique_handlers: std::collections::HashMap<String, &HandlerMapping> =
        std::collections::HashMap::new();
    for handler in handlers {
        unique_handlers.insert(handler.id.clone(), handler);
    }

    // ── Feature detection (tree-shaking) ────────────────────────────────────
    let features = detect_features(document);

    // Computed derived vars
    let mut computed_entries: Vec<String> = Vec::new();
    for component in document.components.values() {
        for cv in &component.computed {
            let compiled = replace_utils_short(&replace_store_and_local(&cv.expr, state_vars));
            computed_entries.push(format!("{{name:'{}',fn:()=>{}}}", cv.name, compiled));
        }
    }
    let has_computed = !computed_entries.is_empty();

    // Destroy hooks
    let destroy_bodies = collect_on_destroy_bodies(document);
    let has_destroy = !destroy_bodies.is_empty();

    // bind() is needed when there are interpolations or computed vars
    let needs_bind = features.has_interpolation || has_computed;
    // evalCond is needed when bind (with interpolations) or any conditional directive exists
    let needs_eval_cond = features.has_interpolation
        || features.has_if
        || features.has_for
        || features.has_dynamic_attrs
        || features.has_style_binding;
    // VARS/STORE_VARS needed whenever any reactive listener subscribes to them
    let needs_vars = needs_eval_cond || needs_bind;

    // Build the "rebind all" sequence used in nav(), setLocale(), WASM loader, DOMContentLoaded
    let all_rebinds = rebind_seq(&features, needs_bind);

    let mut js = String::new();

    js.push_str("// WebCore Runtime (ES2024+)\n");
    js.push_str("{\n");

    // ── State class ─────────────────────────────────────────────────────────
    // setQ: silent setter — updates value without notifying listeners (used by computed)
    js.push_str("class State{#d=new Map();#l=new Map();\n");
    js.push_str("set(k,v){this.#d.set(k,v);this.#l.get(k)?.forEach(f=>f(v))}\n");
    js.push_str("setQ(k,v){this.#d.set(k,v)}\n");
    js.push_str("get(k){return this.#d.get(k)}\n");
    js.push_str("on(k,f){(this.#l.get(k)??this.#l.set(k,[]).get(k)).push(f)}}\n");
    js.push_str("const S=new State();\n");
    js.push_str("const STORE=new State();\n");
    if features.has_refs {
        js.push_str("const refs={};\n");
    }
    js.push('\n');

    // ── State initialisation ─────────────────────────────────────────────────
    for component in document.components.values() {
        for state_var in &component.state {
            let default_value = state_var.default_value.as_deref().unwrap_or("null");
            let value = if state_var.type_ == "String"
                && !default_value.starts_with('"')
                && default_value != "null"
            {
                format!("\"{}\"", default_value)
            } else {
                default_value.to_string()
            };
            js.push_str(&format!("S.set('{}',{});\n", state_var.name, value));
        }
    }
    for store_var in &document.store {
        let default_value = store_var.default_value.as_deref().unwrap_or("null");
        let value = if store_var.type_ == "String"
            && !default_value.starts_with('"')
            && default_value != "null"
        {
            format!("\"{}\"", default_value)
        } else {
            default_value.to_string()
        };
        js.push_str(&format!("STORE.set('{}',{});\n", store_var.name, value));
    }

    // ── VARS / STORE_VARS (only when needed by reactive binding) ─────────────
    if needs_vars {
        let mut sorted_vars: Vec<_> = state_vars.iter().collect();
        sorted_vars.sort();
        let vars_list = sorted_vars
            .iter()
            .map(|v| format!("'{}'", v))
            .collect::<Vec<_>>()
            .join(",");
        js.push_str(&format!("const VARS=[{}];\n", vars_list));

        let mut sorted_store: Vec<_> = store_vars.iter().collect();
        sorted_store.sort();
        let store_list = sorted_store
            .iter()
            .map(|v| format!("'{}'", v))
            .collect::<Vec<_>>()
            .join(",");
        js.push_str(&format!("const STORE_VARS=[{}];\n", store_list));
    }

    js.push_str("const{max,min,abs}=Math,U={max,min,abs};\n\n");

    // ── Computed derived state ────────────────────────────────────────────────
    if has_computed {
        js.push_str(&format!(
            "const COMPUTED=[{}];\n",
            computed_entries.join(",")
        ));
        js.push_str("const rebindComputed=()=>COMPUTED.forEach(c=>S.setQ(c.name,c.fn()));\n\n");
    }

    // ── i18n runtime ─────────────────────────────────────────────────────────
    if !document.locales.is_empty() {
        let mut locale_entries: Vec<String> = document
            .locales
            .iter()
            .map(|(code, messages)| {
                let mut msg_entries: Vec<String> = messages
                    .iter()
                    .map(|(k, v)| format!("\"{}\":\"{}\"", escape_js_str(k), escape_js_str(v)))
                    .collect();
                msg_entries.sort();
                format!("\"{}\":{{{}}}", escape_js_str(code), msg_entries.join(","))
            })
            .collect();
        locale_entries.sort();
        js.push_str(&format!(
            "const LOCALES={{{}}};\n",
            locale_entries.join(",")
        ));
        js.push_str(&format!(
            "let LOCALE=\"{}\";\n",
            escape_js_str(&document.default_locale)
        ));
        // t(key) — simple lookup
        // t(key, n: number) — plural: looks for key_one / key_other, replaces {{count}}
        // t(key, arg) — positional: replaces {{0}} in the translation string
        js.push_str("const t=(k,a)=>{if(a===undefined)return LOCALES[LOCALE]?.[k]??k;if(typeof a==='number'){const pk=a===1?k+'_one':k+'_other';return(LOCALES[LOCALE]?.[pk]??LOCALES[LOCALE]?.[k]??k).replace(/\\{\\{count\\}\\}/g,String(a));}return(LOCALES[LOCALE]?.[k]??k).replace(/\\{\\{0\\}\\}/g,String(a));};\n");
        js.push_str(&format!(
            "const setLocale=l=>{{if(LOCALES[l]){{LOCALE=l;{}}}}};\n\n",
            all_rebinds
        ));
    }

    // ── QUERY_PARAMS Proxy (tree-shaken: only when $query. used) ─────────────
    if features.has_query_params {
        js.push_str("const QUERY_PARAMS=new Proxy({},{get:(_,k)=>new URLSearchParams(location.search).get(String(k))??\"\"});\n");
    }

    // ── evalCond (tree-shaken away when unused) ───────────────────────────────
    if needs_eval_cond {
        // Fast-path lookups for simple identifiers avoid new Function entirely, which
        // is important because (a) new Function runs in global scope so block-scoped
        // S/STORE/U would not be visible if the fast path were skipped, and (b) it
        // avoids the catch fallback returning false for numeric-zero values.
        let route_fast_path = if features.has_param_routes {
            "const rp=_c.match(/^\\$route\\.([a-zA-Z_]\\w*)$/);if(rp)return ROUTE_PARAMS[rp[1]];"
        } else {
            ""
        };
        let query_fast_path = if features.has_query_params {
            "const qp=_c.match(/^\\$query\\.([a-zA-Z_]\\w*)$/);if(qp)return QUERY_PARAMS[qp[1]];"
        } else {
            ""
        };
        let route_replace = if features.has_param_routes {
            "e=e.replace(/\\$route\\.([a-zA-Z_]\\w*)/g,\"ROUTE_PARAMS['$1']\");"
        } else {
            ""
        };
        let query_replace = if features.has_query_params {
            "e=e.replace(/\\$query\\.([a-zA-Z_]\\w*)/g,\"QUERY_PARAMS['$1']\");"
        } else {
            ""
        };
        // S, STORE, U (and t/setLocale when i18n is active) are block-scoped — pass
        // them explicitly as Function parameters so that the dynamically-created
        // function body can resolve them even though Function() runs in global scope.
        let has_locales = !document.locales.is_empty();
        let has_query = features.has_query_params;
        let fn_call = match (features.has_param_routes, has_locales, has_query) {
            (true,  true,  true)  => "new Function('S','STORE','U','ROUTE_PARAMS','QUERY_PARAMS','t','\"use strict\";return('+e+')')(S,STORE,U,ROUTE_PARAMS,QUERY_PARAMS,t)",
            (true,  true,  false) => "new Function('S','STORE','U','ROUTE_PARAMS','t','\"use strict\";return('+e+')')(S,STORE,U,ROUTE_PARAMS,t)",
            (true,  false, true)  => "new Function('S','STORE','U','ROUTE_PARAMS','QUERY_PARAMS','\"use strict\";return('+e+')')(S,STORE,U,ROUTE_PARAMS,QUERY_PARAMS)",
            (true,  false, false) => "new Function('S','STORE','U','ROUTE_PARAMS','\"use strict\";return('+e+')')(S,STORE,U,ROUTE_PARAMS)",
            (false, true,  true)  => "new Function('S','STORE','U','QUERY_PARAMS','t','\"use strict\";return('+e+')')(S,STORE,U,QUERY_PARAMS,t)",
            (false, true,  false) => "new Function('S','STORE','U','t','\"use strict\";return('+e+')')(S,STORE,U,t)",
            (false, false, true)  => "new Function('S','STORE','U','QUERY_PARAMS','\"use strict\";return('+e+')')(S,STORE,U,QUERY_PARAMS)",
            (false, false, false) => "new Function('S','STORE','U','\"use strict\";return('+e+')')(S,STORE,U)",
        };
        // Fast path: simple state-var → S.get(name); $store.x → STORE.get(x);
        // optional $route.x → ROUTE_PARAMS[x]; optional $query.x → QUERY_PARAMS[x].
        // Complex expressions fall through to new Function.  On error return undefined
        // (not false) so interpolation spans show '' rather than the string "false".
        // Note: local var named _c (not t) to avoid shadowing the i18n t() function.
        js.push_str(&format!(
            "const evalCond=c=>{{const _c=c.trim();if(VARS.indexOf(_c)>=0)return S.get(_c);const sm=_c.match(/^\\$store\\.([a-zA-Z_]\\w*)$/);if(sm)return STORE.get(sm[1]);{}{}let e=_c;e=e.replace(/\\$store\\.([a-zA-Z_]\\w*)/g,\"STORE.get('$1')\");{}{}VARS.forEach(v=>{{e=e.replace(new RegExp('\\\\b'+v+'\\\\b','g'),\"S.get('\"+v+\"')\")}});try{{return {}}}catch(_){{return undefined}}}};\n",
            route_fast_path,
            query_fast_path,
            route_replace,
            query_replace,
            fn_call
        ));
    }

    // ── Reactive binding functions (tree-shaken) ──────────────────────────────
    if features.has_if {
        // bindIf with optional webc:transition support
        if features.has_transition {
            js.push_str(
                "const bindIf=()=>{\n\
                 document.querySelectorAll('[data-webcore-if]').forEach(el=>{\n\
                   const cond=el.dataset.webcoreIf,\n\
                         next=el.nextElementSibling,\n\
                         hasElse=next?.dataset.webcoreElse===cond,\n\
                         upd=()=>{\n\
                           const v=evalCond(cond),show=!!v;\n\
                           const _tr=el.dataset.webcoreTransition;\n\
                           if(_tr){\n\
                             if(show){\n\
                               el.style.display='';\n\
                               el.classList.add('webc-'+_tr+'-enter');\n\
                               requestAnimationFrame(()=>el.classList.replace('webc-'+_tr+'-enter','webc-'+_tr+'-enter-to'));\n\
                             } else {\n\
                               el.classList.add('webc-'+_tr+'-leave');\n\
                               requestAnimationFrame(()=>{\n\
                                 el.classList.replace('webc-'+_tr+'-leave','webc-'+_tr+'-leave-to');\n\
                                 el.addEventListener('transitionend',()=>{el.style.display='none';el.classList.remove('webc-'+_tr+'-leave-to');},{once:true});\n\
                               });\n\
                             }\n\
                           } else {\n\
                             el.style.display=show?'':'none';\n\
                           }\n\
                           if(hasElse)next.style.display=show?'none':''\n\
                         };\n\
                   upd();\n\
                   VARS.forEach(v=>S.on(v,upd));\n\
                   STORE_VARS.forEach(v=>STORE.on(v,upd))\n\
                 })\n\
                 };\n"
            );
        } else {
            js.push_str(
                "const bindIf=()=>{\n\
                 document.querySelectorAll('[data-webcore-if]').forEach(el=>{\n\
                   const cond=el.dataset.webcoreIf,\n\
                         next=el.nextElementSibling,\n\
                         hasElse=next?.dataset.webcoreElse===cond,\n\
                         upd=()=>{\n\
                           const v=evalCond(cond);\n\
                           el.style.display=v?'':'none';\n\
                           if(hasElse)next.style.display=v?'none':''\n\
                         };\n\
                   upd();\n\
                   VARS.forEach(v=>S.on(v,upd));\n\
                   STORE_VARS.forEach(v=>STORE.on(v,upd))\n\
                 })\n\
                 };\n"
            );
        }
    }
    if features.has_for {
        // bindFor — renders @for loops; supports optional key-based DOM diffing when
        // data-webcore-for-key is present on the template (avoids full re-render).
        // fillItem(el, val, i): sets text for interpolation spans (supports "item.prop" paths),
        // writes data-webcore-idx, and mirrors object properties as data-* attributes for CSS.
        // Keyed diffing: webcoreKey stored on firstElementChild (no extra wrapper div).
        js.push_str("const bindFor=()=>{document.querySelectorAll('template[data-webcore-for]').forEach(tmpl=>{const iN=tmpl.dataset.webcoreFor,rawItN=tmpl.dataset.webcoreIn,keyExpr=tmpl.dataset.webcoreForKey,idxN=tmpl.dataset.webcoreForIndex,isStore=rawItN.startsWith('$store.'),itN=isStore?rawItN.slice(7):rawItN,state=isStore?STORE:S,cont=tmpl.nextElementSibling,evalKey=keyExpr?(val=>keyExpr.split('.').reduce((o,k)=>o?.[k],{[iN]:val})):null,fillItem=(el,val,i)=>{el.querySelectorAll('[data-webcore-interpolation]').forEach(s=>{const ie=s.dataset.webcoreInterpolation;if(ie===iN)s.textContent=String(val??'');else if(idxN&&ie===idxN)s.textContent=String(i);else if(ie.startsWith(iN+'.'))s.textContent=String(ie.slice(iN.length+1).split('.').reduce((o,k)=>o?.[k],val)??'')});el.dataset.webcoreIdx=String(i);if(val&&typeof val==='object')Object.entries(val).forEach(([k,v])=>{if(typeof v!=='object')el.dataset[k]=String(v)})},render=()=>{const items=state.get(itN)??[];if(evalKey){const newKeys=items.map(evalKey);const existing=new Map([...cont.children].map(c=>[c.dataset.webcoreKey,c]));const keep=new Set(newKeys);[...existing.keys()].filter(k=>!keep.has(k)).forEach(k=>existing.get(k).remove());const frag=document.createDocumentFragment();newKeys.forEach((key,i)=>{if(existing.has(key)){const el=existing.get(key);fillItem(el,items[i],i);frag.appendChild(el);}else{const cl=tmpl.content.cloneNode(true);const fe=cl.firstElementChild;if(fe){fe.dataset.webcoreKey=key;fillItem(fe,items[i],i);}frag.append(...Array.from(cl.children));}});cont.replaceChildren(frag);}else{cont.innerHTML='';items.forEach((val,i)=>{const cl=tmpl.content.cloneNode(true);const firstEl=cl.firstElementChild;if(firstEl)fillItem(firstEl,val,i);cont.appendChild(cl)});}};render();state.on(itN,render);STORE_VARS.forEach(v=>STORE.on(v,render))});};\n");
    }
    if features.has_dynamic_attrs {
        if features.has_style_binding {
            js.push_str(
                "const bindAttrs=()=>{\n\
                 document.querySelectorAll('[data-webcore-bound]').forEach(el=>{\n\
                   [...el.attributes]\n\
                     .filter(a=>a.name.startsWith('data-webcore-attr-'))\n\
                     .forEach(a=>{\n\
                       const name=a.name.slice(18),expr=a.value,\n\
                             upd=()=>{\n\
                               const val=String(evalCond(expr)??'');\n\
                               name in el?el[name]=val:el.setAttribute(name,val)\n\
                             };\n\
                       upd();\n\
                       VARS.forEach(v=>S.on(v,upd));\n\
                       STORE_VARS.forEach(v=>STORE.on(v,upd))\n\
                     });\n\
                   for(const a of el.attributes){if(a.name.startsWith('data-webcore-style-')){const p=a.name.slice('data-webcore-style-'.length);const styleUpd=()=>el.style.setProperty(p,String(evalCond(a.value)??''));styleUpd();VARS.forEach(v=>S.on(v,styleUpd));STORE_VARS.forEach(v=>STORE.on(v,styleUpd));}}\n\
                 })\n\
                 };\n"
            );
        } else {
            js.push_str(
                "const bindAttrs=()=>{\n\
                 document.querySelectorAll('[data-webcore-bound]').forEach(el=>{\n\
                   [...el.attributes]\n\
                     .filter(a=>a.name.startsWith('data-webcore-attr-'))\n\
                     .forEach(a=>{\n\
                       const name=a.name.slice(18),expr=a.value,\n\
                             upd=()=>{\n\
                               const val=String(evalCond(expr)??'');\n\
                               name in el?el[name]=val:el.setAttribute(name,val)\n\
                             };\n\
                       upd();\n\
                       VARS.forEach(v=>S.on(v,upd));\n\
                       STORE_VARS.forEach(v=>STORE.on(v,upd))\n\
                     })\n\
                 })\n\
                 };\n"
            );
        }
    } else if features.has_style_binding {
        // Only style bindings, no regular dynamic attrs — emit a simpler bindAttrs
        js.push_str(
            "const bindAttrs=()=>{\
document.querySelectorAll('[data-webcore-bound]').forEach(el=>{\
for(const a of Array.from(el.attributes)){\
if(a.name.startsWith('data-webcore-style-')){\
const p=a.name.slice('data-webcore-style-'.length);\
const styleUpd=()=>el.style.setProperty(p,String(evalCond(a.value)??''));\
styleUpd();\
VARS.forEach(v=>S.on(v,styleUpd));\
STORE_VARS.forEach(v=>STORE.on(v,styleUpd));\
}\
}\
})\
};\n"
        );
    }
    if features.has_class_binding {
        js.push_str(
            "const bindClassBindings=()=>{\n\
             document.querySelectorAll('*').forEach(el=>{\n\
               for(const attr of Array.from(el.attributes)){\n\
                 if(attr.name.startsWith('data-webcore-class-')){\n\
                   const cls=attr.name.slice(19),expr=attr.value,\n\
                         upd=()=>el.classList.toggle(cls,!!evalCond(expr));\n\
                   upd();\n\
                   VARS.forEach(v=>S.on(v,upd));\n\
                   STORE_VARS.forEach(v=>STORE.on(v,upd))\n\
                 }\n\
               }\n\
             })\n\
             };\n"
        );
    }
    if features.has_validation {
        js.push_str(
            "const validateField=input=>{\n\
               const val=input.value??'';\n\
               if('webcoreValidateRequired'in input.dataset&&!val.trim())\n\
                 return input.dataset.webcoreValidateRequired||'Champ requis';\n\
               const ml=input.dataset.webcoreValidateMinlength;\n\
               if(ml&&val.length<+ml)\n\
                 return input.dataset.webcoreValidateMinlengthMsg||`Minimum ${ml} caractères`;\n\
               const xl=input.dataset.webcoreValidateMaxlength;\n\
               if(xl&&val.length>+xl)\n\
                 return input.dataset.webcoreValidateMaxlengthMsg||`Maximum ${xl} caractères`;\n\
               if('webcoreValidateEmail'in input.dataset&&\n\
                  !/^[^\\s@]+@[^\\s@]+\\.[^\\s@]+$/.test(val))\n\
                 return input.dataset.webcoreValidateEmail||'Email invalide';\n\
               const pat=input.dataset.webcoreValidatePattern;\n\
               if(pat){try{if(!new RegExp(pat).test(val))\n\
                 return input.dataset.webcoreValidatePatternMsg||'Format invalide'}catch(_){}}\n\
               return''\n\
             };\n"
        );
        js.push_str(
            "const bindValidation=()=>{\n\
             document.querySelectorAll('form').forEach(form=>{\n\
               const check=input=>{\n\
                 const field=input.dataset.webcoreField,\n\
                       err=validateField(input),\n\
                       el=field&&form.querySelector(`[data-webcore-error=\"${field}\"]`);\n\
                 if(el){(el.firstElementChild||el).textContent=err;el.style.display=err?'':'none'}\n\
                 return!err\n\
               };\n\
               form.querySelectorAll('[data-webcore-field]').forEach(input=>{\n\
                 input.addEventListener('blur',()=>{input.dataset.webcoreTouched='1';check(input)});\n\
                 input.addEventListener('input',()=>{if(input.dataset.webcoreTouched)check(input)})\n\
               });\n\
               form.addEventListener('submit',e=>{\n\
                 let ok=true;\n\
                 form.querySelectorAll('[data-webcore-field]').forEach(input=>{\n\
                   if(!check(input))ok=false\n\
                 });\n\
                 if(!ok){e.preventDefault();e.stopImmediatePropagation()}\n\
               },true)\n\
             })\n\
             };\n"
        );
    }
    js.push('\n');

    // ── Handlers ─────────────────────────────────────────────────────────────
    // Debounce handlers are wired up in DOMContentLoaded; regular handlers go in H.
    js.push_str("const H={\n");
    let mut sorted_handlers: Vec<_> = unique_handlers.values().collect();
    sorted_handlers.sort_by(|a, b| a.id.cmp(&b.id));
    for handler in &sorted_handlers {
        // Skip debounce handlers — they get wired up in DOMContentLoaded instead
        if handler.event_type.contains("|debounce") {
            continue;
        }
        let compiled = compile_expression_full(&handler.expression, state_vars);
        js.push_str(&format!("{}(){{{}}},\n", handler.id, compiled));
    }
    js.push_str("};\n\n");

    // ── bind() — re-evaluate computed, then wire interpolation spans ──────────
    if needs_bind {
        if features.has_interpolation {
            js.push_str("const bind=()=>{");
            if has_computed {
                js.push_str("rebindComputed();");
            }
            // u re-runs rebindComputed so computed vars (e.g. doubled) are fresh
            // before the span's textContent is updated. setQ is side-effect-free
            // (no listener cascade), so this cannot loop.
            let recompute_in_u = if has_computed { "rebindComputed();" } else { "" };
            js.push_str(&format!(
                "document.querySelectorAll('[data-webcore-interpolation]').forEach(el=>{{const e=el.dataset.webcoreInterpolation,u=()=>{{{}{}}};u();VARS.forEach(v=>S.on(v,u));STORE_VARS.forEach(v=>STORE.on(v,u))}})}};\n\n",
                recompute_in_u,
                "el.textContent=String(evalCond(e)??'')"
            ));
        } else {
            // only computed, no interpolations
            js.push_str("const bind=()=>rebindComputed();\n\n");
        }
    }

    // ── on:destroy hooks ─────────────────────────────────────────────────────
    if has_destroy {
        let bodies: Vec<String> = destroy_bodies
            .iter()
            .map(|b| format!("()=>{{{}}}", b.trim()))
            .collect();
        js.push_str(&format!("const DESTROY_HOOKS=[{}];\n", bodies.join(",")));
        js.push_str("const runDestroyHooks=()=>DESTROY_HOOKS.forEach(f=>f());\n\n");
    }

    // ── SPA navigation (tree-shaken when unused) ──────────────────────────────
    if features.has_navigation {
        if features.has_param_routes {
            // Build ROUTES array with regex patterns for parameterized routes
            let routes = document
                .app
                .as_ref()
                .map(|a| &a.routes)
                .cloned()
                .unwrap_or_default();
            let mut route_entries: Vec<(String, String)> = routes.into_iter().collect();
            route_entries.sort_by_key(|(path, _)| path.clone());

            let mut routes_js = String::from("const ROUTES=[");
            for (path, page_name) in &route_entries {
                let (regex, params) = route_to_js_regex(path);
                let file = page_name_to_file(page_name);
                let params_js: Vec<String> = params.iter().map(|p| format!("\"{}\"", p)).collect();
                routes_js.push_str(&format!(
                    "{{re:/{}/,file:\"{}\",params:[{}]}},",
                    regex,
                    escape_js_str(&file),
                    params_js.join(",")
                ));
            }
            routes_js.push_str("];\n");
            js.push_str(&routes_js);
            js.push_str("let ROUTE_PARAMS={};\n");
            js.push_str("const matchRoute=p=>{for(const r of ROUTES){const m=p.match(r.re);if(m){r.params.forEach((k,i)=>ROUTE_PARAMS[k]=m[i+1]);return r.file;}}return p==='/'?'index.html':`${p.slice(1)}/index.html`;};\n");
        } else {
            js.push_str("const toFile=p=>p==='/'?'index.html':`${p.slice(1)}/index.html`;\n");
        }
        // init=true → replaceState (no duplicate history entry on first load)
        js.push_str("const nav=async(p,init=false)=>{\n");
        if has_destroy {
            js.push_str("runDestroyHooks();\n");
        }
        let file_expr = if features.has_param_routes {
            "matchRoute(p)"
        } else {
            "toFile(p)"
        };
        js.push_str(&format!("const file={};\n", file_expr));
        js.push_str("try{const html=await(await fetch('/'+file)).text();\n");
        js.push_str("const doc=new DOMParser().parseFromString(html,'text/html');\n");
        js.push_str("const main=doc.querySelector('main');\n");
        js.push_str("if(main)document.querySelector('main').replaceWith(main);\n");
        js.push_str(&format!(
            "if(init)history.replaceState({{}},'',p);else history.pushState({{}},'',p);{}}}catch(e){{location.href='/'+file}}}};\n\n",
            all_rebinds
        ));
        js.push_str("addEventListener('popstate',()=>nav(location.pathname));\n\n");
    }

    // ── globalThis exports ────────────────────────────────────────────────────
    let i18n_export = if !document.locales.is_empty() {
        ",setLocale"
    } else {
        ""
    };
    let nav_export = if features.has_navigation {
        "webcore_navigate:nav,"
    } else {
        ""
    };
    js.push_str(&format!(
        "Object.assign(globalThis,{{{}webcore_handle_event:(t,id)=>H[id]?.(){},\n",
        nav_export, i18n_export
    ));
    js.push_str(
        "...Object.fromEntries(['click','submit','change','input'].map(e=>[`webcore_handle_${e}`,id=>H[id]?.()]))});\n\n",
    );

    // ── DOMContentLoaded ─────────────────────────────────────────────────────
    let mount_bodies = collect_on_mount_bodies(document);
    let comp_listeners = collect_component_event_listeners(document);

    // For param routes: populate ROUTE_PARAMS then, if the current URL is a
    // parameterised path (slug, id, …), asynchronously load the correct page.
    // The dev server may have served index.html as SPA fallback for /post/hello,
    // so we need to fetch post.html and swap <main> with the right content.
    let init_route_params = if features.has_param_routes {
        "matchRoute(location.pathname);if(Object.keys(ROUTE_PARAMS).length)nav(location.pathname,true);"
    } else {
        ""
    };
    let transition_css_inject = if features.has_transition {
        ";document.head.insertAdjacentHTML('beforeend','<style>.webc-fade-enter{opacity:0;transition:opacity 250ms ease}.webc-fade-enter-to{opacity:1}.webc-fade-leave{opacity:1;transition:opacity 250ms ease}.webc-fade-leave-to{opacity:0}.webc-slide-enter{transform:translateY(-6px);opacity:0;transition:transform 200ms ease,opacity 200ms ease}.webc-slide-enter-to{transform:none;opacity:1}.webc-slide-leave{transition:transform 200ms ease,opacity 200ms ease}.webc-slide-leave-to{transform:translateY(-6px);opacity:0}</style>')"
    } else {
        ""
    };
    let refs_populate = if features.has_refs {
        ";document.querySelectorAll('[data-webcore-ref]').forEach(el=>{refs[el.dataset.webcoreRef]=el;})"
    } else {
        ""
    };
    js.push_str(&format!(
        "document.addEventListener('DOMContentLoaded',()=>{{{init}{transition_css}{rebinds}{refs_populate}",
        init = init_route_params,
        transition_css = transition_css_inject,
        rebinds = all_rebinds,
        refs_populate = refs_populate,
    ));
    for body in &mount_bodies {
        js.push_str(&format!(";(()=>{{{}}})()", body.trim()));
    }
    for listener in &comp_listeners {
        let compiled = compile_expression_full(&listener.expression, state_vars);
        js.push_str(&format!(
            ";document.addEventListener('{}',e=>{{{}}})",
            listener.event_name, compiled
        ));
    }
    // ── Debounce event listeners ──────────────────────────────────────────────
    // Wire up debounce handlers: find the element by id and attach a debounced listener.
    {
        let mut dbt_idx = 0usize;
        let debounce_handlers: Vec<_> = sorted_handlers
            .iter()
            .filter(|h| h.event_type.contains("|debounce"))
            .collect();
        for handler in debounce_handlers {
            // event_type is like "input|debounce=300"
            let (event_name, delay_ms) = if let Some(pipe_pos) = handler.event_type.find("|debounce") {
                let base = &handler.event_type[..pipe_pos];
                let after = &handler.event_type[pipe_pos + "|debounce".len()..];
                let ms: u32 = if let Some(stripped) = after.strip_prefix('=') {
                    stripped.parse().unwrap_or(300)
                } else {
                    300
                };
                (base, ms)
            } else {
                (handler.event_type.as_str(), 300u32)
            };
            let compiled = compile_expression_full(&handler.expression, state_vars);
            dbt_idx += 1;
            js.push_str(&format!(
                ";(()=>{{const __el=document.getElementById('{}');if(__el){{let __dbt{};__el.addEventListener('{}',e=>{{clearTimeout(__dbt{});__dbt{}=setTimeout(()=>{{{};}}{},{})}})}}}})()",
                handler.id,
                dbt_idx,
                event_name,
                dbt_idx,
                dbt_idx,
                compiled,
                if all_rebinds.is_empty() { "".to_string() } else { format!(";{}", all_rebinds) },
                delay_ms
            ));
        }
    }
    // ── HTTP fetch blocks ─────────────────────────────────────────────────────
    if features.has_http {
        for component in document.components.values() {
            if let Some(http) = &component.http {
                let into_var = &http.into;
                let url = &http.url;
                let rb = if all_rebinds.is_empty() { "".to_string() } else { format!("{};", all_rebinds) };
                js.push_str(&format!(
                    ";(async()=>{{try{{const __r=await fetch(\"{}\");if(!__r.ok)throw new Error(__r.statusText);const __d=await __r.json();S.set('{}',__d);S.set('loading',false);{}}}catch(__e){{S.set('error',__e.message);S.set('loading',false);{}}}}})()",
                    escape_js_str(url),
                    escape_js_str(into_var),
                    rb,
                    rb,
                ));
            }
        }
    }
    if has_destroy {
        js.push_str(";window.addEventListener('beforeunload',runDestroyHooks)");
    }
    js.push_str("});\n");

    // ── WASM async loader (only when module detected) ─────────────────────────
    if let Some(module) = &document.wasm_module {
        let wasm_loader = format!(
            "const WASM={{}};globalThis.wasm=WASM;\
(async()=>{{try{{const m=await import('./wasm/{m}.js');\
await m.default();Object.assign(WASM,m);\
{rb};\
}}catch(e){{console.warn('[WebCore WASM]',e);}}}})();\n",
            m = escape_js_str(module),
            rb = all_rebinds,
        );
        js.push_str(&wasm_loader);
    }

    js.push_str("}\n");

    js
}

/// Escape a string for safe embedding in a JS double-quoted string literal.
fn escape_js_str(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

/// Strip line comments and collapse whitespace — safe for generated JS (no multiline strings).
pub fn minify_js(js: &str) -> String {
    js.lines()
        .filter(|l| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with("//")
        })
        .map(str::trim)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compile_increment() {
        let mut vars = HashSet::new();
        vars.insert("count".to_string());

        let result = compile_expression_full("count += 1", &vars);
        assert_eq!(result, "S.set('count',S.get('count')+1)");
    }

    #[test]
    fn test_compile_decrement() {
        let mut vars = HashSet::new();
        vars.insert("count".to_string());

        let result = compile_expression_full("count -= 1", &vars);
        assert_eq!(result, "S.set('count',S.get('count')-1)");
    }

    #[test]
    fn test_compile_assignment_with_max() {
        let mut vars = HashSet::new();
        vars.insert("count".to_string());

        let result = compile_expression_full("count = max(0, count - 1)", &vars);
        assert!(result.starts_with("S.set('count',"));
        assert!(result.contains("U.max"));
        assert!(result.contains("S.get('count')"));
    }

    #[test]
    fn test_compile_navigate() {
        let vars = HashSet::new();

        let result = compile_expression_full("webcore_navigate(/about)", &vars);
        assert_eq!(result, "nav('/about')");
    }

    #[test]
    fn test_multiple_variables() {
        let mut vars = HashSet::new();
        vars.insert("x".to_string());
        vars.insert("y".to_string());
        vars.insert("total".to_string());

        let result = compile_expression_full("total = x + y", &vars);
        assert!(result.starts_with("S.set('total',"));
        assert!(result.contains("S.get('x')"));
        assert!(result.contains("S.get('y')"));
    }
}
