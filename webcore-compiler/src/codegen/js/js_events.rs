//! Event-handler and expression compilation helpers.
//!
//! All functions here compile `WebCore` expression strings (e.g. `count += 1`,
//! `$store.x = y`, `emit("ping")`) into their JavaScript equivalents.

use regex::Regex;
use std::collections::HashSet;
use std::sync::OnceLock;

static RE_STORE: OnceLock<Regex> = OnceLock::new();
static RE_SENT: OnceLock<Regex> = OnceLock::new();

/// Pre-compiled, longest-first sorted variable replacement regexes for a document.
///
/// Build once per runtime-generation call instead of recompiling
/// on every expression. Sorting longest-first prevents shorter names from
/// matching inside longer ones (e.g. `count` inside `countdown`).
pub(crate) struct CompiledVars(pub(crate) Vec<(String, Regex)>);

impl CompiledVars {
    pub(crate) fn new(vars: &HashSet<String>) -> Self {
        let mut sorted: Vec<&String> = vars.iter().collect();
        sorted.sort_by_key(|v| std::cmp::Reverse(v.len()));
        let pairs = sorted
            .into_iter()
            .filter_map(|v| {
                let pat = format!(r"\b{}\b", regex::escape(v));
                Regex::new(&pat).ok().map(|re| (v.clone(), re))
            })
            .collect();
        Self(pairs)
    }

    fn replace_into(&self, expr: &str) -> String {
        let mut result: String = expr.to_owned();
        for (var, re) in &self.0 {
            // Sentinel-protect property accesses (e.g. `obj.done`) so `\bdone\b` doesn't
            // match the property name — the dot is a word-boundary in regex but `obj.done`
            // is not a state-var reference.
            let prop_pat = format!(".{var}");
            let sentinel = format!(".__WCPROP_{var}__");
            let protected = result.replace(&prop_pat, &sentinel);
            let replacement = format!("S.get('{var}')");
            let replaced = re
                .replace_all(&protected, replacement.as_str())
                .into_owned();
            result = replaced.replace(&sentinel, &prop_pat);
        }
        result
    }
}

/// Parse `$store.identifier op= value`
pub(super) fn parse_store_compound_assign(expr: &str, op: &str) -> Option<(String, String)> {
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
pub(super) fn parse_store_simple_assign(expr: &str) -> Option<(String, String)> {
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

/// Parse compound assignment like "count += 1"
pub(super) fn parse_compound_assign(expr: &str, operator: &str) -> Option<(String, String)> {
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
pub(super) fn parse_simple_assign(expr: &str) -> Option<(String, String)> {
    // Walk char_indices so we never allocate a Vec<char>.
    // '=' is ASCII so byte-indexing the next char is safe.
    let mut prev = '\0';
    for (i, c) in expr.char_indices() {
        if c == '=' {
            let prev_is_op = matches!(prev, '=' | '!' | '<' | '>' | '+' | '-' | '*' | '/');
            let next_is_eq = expr.as_bytes().get(i + 1) == Some(&b'=');
            if !prev_is_op && !next_is_eq {
                let var_name = expr[..i].trim();
                let value = expr[i + 1..].trim();
                if !var_name.is_empty() && var_name.chars().all(|c| c.is_alphanumeric() || c == '_')
                {
                    return Some((var_name.to_string(), value.to_string()));
                }
            }
        }
        prev = c;
    }
    None
}

/// Compile reactive list mutations for local state vars and `$store` vars.
///
/// Recognized patterns (where `var` is a known state/store variable):
/// - `var.push(value)`  → spread-append into reactive set
/// - `var.remove(i)`    → filter by index
/// - `var.clear()`      → reset to empty array
/// - `$store.var.push(…)` / `.remove(…)` / `.clear()` — same for the global store
pub(crate) fn compile_list_method(expr: &str, vars: &CompiledVars) -> Option<String> {
    let trimmed = expr.trim();

    // $store.var.push/remove/clear
    if let Some(rest) = trimmed.strip_prefix("$store.") {
        for method in ["push", "remove", "clear"] {
            let marker = format!(".{}(", method);
            if let Some(dot_pos) = rest.find(&marker) {
                let var = &rest[..dot_pos];
                if var.is_empty() || !var.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    continue;
                }
                let after_open = &rest[dot_pos + marker.len()..];
                let arg = after_open
                    .rfind(')')
                    .map(|i| after_open[..i].trim())
                    .unwrap_or("");
                let compiled = match method {
                    "push" => {
                        let a = replace_utils_short(&replace_store_and_local(arg, vars));
                        format!("STORE.set('{var}',[...STORE.get('{var}'),{a}])")
                    }
                    "remove" => {
                        let a = replace_utils_short(&replace_store_and_local(arg, vars));
                        format!("STORE.set('{var}',STORE.get('{var}').filter((_,_i)=>_i!==({a})))")
                    }
                    "clear" => format!("STORE.set('{var}',[])"),
                    _ => continue,
                };
                return Some(compiled);
            }
        }
    }

    // local state var.push/remove/clear
    for method in ["push", "remove", "clear"] {
        let marker = format!(".{}(", method);
        if let Some(dot_pos) = trimmed.find(&marker) {
            let var = &trimmed[..dot_pos];
            if var.is_empty() || !var.chars().all(|c| c.is_alphanumeric() || c == '_') {
                continue;
            }
            if !vars.0.iter().any(|(v, _)| v == var) {
                continue;
            }
            let after_open = &trimmed[dot_pos + marker.len()..];
            let arg = after_open
                .rfind(')')
                .map(|i| after_open[..i].trim())
                .unwrap_or("");
            let compiled = match method {
                "push" => {
                    let a = replace_utils_short(&replace_store_and_local(arg, vars));
                    format!("S.set('{var}',[...S.get('{var}'),{a}])")
                }
                "remove" => {
                    let a = replace_utils_short(&replace_store_and_local(arg, vars));
                    format!("S.set('{var}',S.get('{var}').filter((_,_i)=>_i!==({a})))")
                }
                "clear" => format!("S.set('{var}',[])"),
                _ => continue,
            };
            return Some(compiled);
        }
    }

    None
}

/// Replace utility functions with short aliases (U.max, etc.)
pub(super) fn replace_utils_short(expr: &str) -> String {
    let mut result = expr.to_string();
    for (old, new) in [("max(", "U.max("), ("min(", "U.min("), ("abs(", "U.abs(")] {
        if !result.contains(&format!("U.{old}")) {
            result = result.replace(old, new);
        }
    }
    result
}

/// Replace `$store.varname` (sentinel: `__STORE_varname__`) then local vars, then restore.
/// The sentinel approach prevents local-var replacement from touching store references.
/// The two hardcoded regexes are compiled once (OnceLock) for the process lifetime.
pub(super) fn replace_store_and_local(expr: &str, vars: &CompiledVars) -> String {
    let re_store = RE_STORE
        .get_or_init(|| Regex::new(r"\$store\.([a-zA-Z_][a-zA-Z0-9_]*)").expect("hardcoded regex"));
    let re_sent = RE_SENT.get_or_init(|| {
        Regex::new(r"__STORE_([a-zA-Z_][a-zA-Z0-9_]*)__").expect("hardcoded regex")
    });
    // Step 1: sentinel so local-var replacement can't touch store refs
    let sentineled = re_store.replace_all(expr, "__STORE_$1__").into_owned();
    // Step 2: replace local state vars (_ is a word char → \bcount\b won't match inside __STORE_count__)
    let with_local = vars.replace_into(&sentineled);
    // Step 3: restore sentinels → STORE.get('...')
    re_sent
        .replace_all(&with_local, "STORE.get('$1')")
        .into_owned()
}

/// Compile a `store { computed { name = expr } }` expression.
///
/// Both `$store.var` (explicit) and bare `var` (implicit, when var is a store variable)
/// are rewritten to `STORE.get('var')`.
///
/// Single-pass strategy: builds a combined regex that matches either `$store.var`
/// (explicit, group 1) or a bare store-var name at word boundary (implicit, group 2),
/// longest-first. Because the regex engine tries alternatives left-to-right, `$store.var`
/// consumes the text before the bare name can be matched inside the same span.
pub(crate) fn compile_store_computed_expr(expr: &str, store_vars: &HashSet<String>) -> String {
    use std::cmp::Reverse;

    let mut sorted: Vec<&String> = store_vars.iter().collect();
    sorted.sort_by_key(|v| Reverse(v.len()));

    let var_alts: String = sorted
        .iter()
        .map(|v| regex::escape(v))
        .collect::<Vec<_>>()
        .join("|");

    let pattern = if var_alts.is_empty() {
        r"\$store\.([a-zA-Z_][a-zA-Z0-9_]*)".to_string()
    } else {
        format!(r"\$store\.([a-zA-Z_][a-zA-Z0-9_]*)|\b({})\b", var_alts)
    };

    let re = match Regex::new(&pattern) {
        Ok(r) => r,
        Err(_) => return expr.to_string(),
    };

    re.replace_all(expr, |caps: &regex::Captures| {
        if let Some(var) = caps.get(1) {
            return format!("STORE.get('{}')", var.as_str());
        }
        if let Some(var) = caps.get(2) {
            return format!("STORE.get('{}')", var.as_str());
        }
        caps[0].to_string()
    })
    .into_owned()
}

/// Convert a route pattern `/post/:slug` to a JS regex string `^\/post\/([^\/]+)$`
/// and return the list of param names in capture order.
pub(super) fn route_to_js_regex(pattern: &str) -> (String, Vec<String>) {
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

/// Compile a read expression into a JS closure string (v3 compiled expressions).
/// e.g. `"count > 0"` → `"()=>S.get('count')>0"`
/// Used for: interpolations, @if conditions, class:/style: bindings, dynamic attrs.
pub(crate) fn compile_read_expr(
    expr: &str,
    vars: &CompiledVars,
    has_route_params: bool,
    has_query_params: bool,
) -> String {
    static RE_ROUTE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    static RE_QUERY: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();

    let mut compiled = replace_store_and_local(expr.trim(), vars);
    compiled = replace_utils_short(&compiled);
    if has_route_params {
        let re = RE_ROUTE
            .get_or_init(|| Regex::new(r"\$route\.([a-zA-Z_]\w*)").expect("hardcoded regex"));
        compiled = re.replace_all(&compiled, "ROUTE_PARAMS['$1']").into_owned();
    }
    if has_query_params {
        let re = RE_QUERY
            .get_or_init(|| Regex::new(r"\$query\.([a-zA-Z_]\w*)").expect("hardcoded regex"));
        compiled = re.replace_all(&compiled, "QUERY_PARAMS['$1']").into_owned();
    }
    format!("()=>{compiled}")
}

/// Convert a component/page name to the expected HTML filename (mirrors main.rs logic).
pub(super) fn page_name_to_file(name: &str) -> String {
    let route = name
        .to_lowercase()
        .replace("page", "")
        .replace("home", "index");
    if route.is_empty() || route == "index" {
        "index.html".to_string()
    } else {
        format!("{route}/index.html")
    }
}

/// Compile `webcore_navigate` calls to short `nav()` function
pub(super) fn compile_navigate_call(expr: &str) -> String {
    if let Some(start) = expr.find("webcore_navigate(") {
        if let Some(end) = expr[start..].find(')') {
            let path = expr[start + 17..start + end].trim();
            if path == "root" {
                return "nav('/')".to_string();
            }
            if path.starts_with('"') {
                return format!("nav({path})");
            }
            if path.starts_with('/') {
                return format!("nav('{path}')");
            }
            return format!("nav('/{path}')");
        }
    }
    expr.to_string()
}

/// Compile emit("eventName") or emit("eventName", data) to `CustomEvent` dispatch.
pub(super) fn compile_emit_call(expr: &str, vars: &CompiledVars) -> String {
    if let Some(start) = expr.find("emit(") {
        let inner_start = start + 5;
        if let Some(paren_end) = expr[inner_start..].rfind(')') {
            let args_str = &expr[inner_start..inner_start + paren_end];
            let parts: Vec<&str> = args_str.splitn(2, ',').collect();
            let event_name = parts[0].trim();
            if parts.len() == 1 || parts[1].trim().is_empty() {
                return format!("document.dispatchEvent(new CustomEvent({event_name}))");
            }
            let detail_raw = parts[1].trim();
            let detail = replace_utils_short(&replace_store_and_local(detail_raw, vars));
            return format!(
                "document.dispatchEvent(new CustomEvent({event_name},{{detail:{detail}}}))"
            );
        }
    }
    let result = replace_store_and_local(expr, vars);
    replace_utils_short(&result)
}

/// Full expression compiler: handles both `$store.var` and local state vars.
pub(crate) fn compile_expression_full(expr: &str, vars: &CompiledVars) -> String {
    let compiled = expr.trim();

    // Multi-statement: split on ; and compile each independently
    // e.g. `items = [...items, draft]; draft = ""`
    if compiled.contains(';') {
        let parts: Vec<&str> = compiled
            .split(';')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        if parts.len() > 1 {
            return parts
                .iter()
                .map(|s| compile_expression_full(s, vars))
                .collect::<Vec<_>>()
                .join(";");
        }
    }

    // Store compound assigns: $store.count += 1
    if let Some((var, val)) = parse_store_compound_assign(compiled, "+=") {
        let rhs = replace_utils_short(&replace_store_and_local(&val, vars));
        return format!("STORE.set('{var}',STORE.get('{var}')+{rhs})");
    }
    if let Some((var, val)) = parse_store_compound_assign(compiled, "-=") {
        let rhs = replace_utils_short(&replace_store_and_local(&val, vars));
        return format!("STORE.set('{var}',STORE.get('{var}')-{rhs})");
    }
    if let Some((var, val)) = parse_store_compound_assign(compiled, "*=") {
        let rhs = replace_utils_short(&replace_store_and_local(&val, vars));
        return format!("STORE.set('{var}',STORE.get('{var}')*{rhs})");
    }
    if let Some((var, val)) = parse_store_compound_assign(compiled, "/=") {
        let rhs = replace_utils_short(&replace_store_and_local(&val, vars));
        return format!("STORE.set('{var}',STORE.get('{var}')/{rhs})");
    }
    // Store simple assign: $store.count = value
    if let Some((var, val)) = parse_store_simple_assign(compiled) {
        let rhs = replace_utils_short(&replace_store_and_local(&val, vars));
        return format!("STORE.set('{var}',{rhs})");
    }

    // Local state compound assigns: count += 1
    if let Some((var, val)) = parse_compound_assign(compiled, "+=") {
        return format!(
            "S.set('{}',S.get('{}')+{})",
            var,
            var,
            replace_store_and_local(&val, vars)
        );
    }
    if let Some((var, val)) = parse_compound_assign(compiled, "-=") {
        return format!(
            "S.set('{}',S.get('{}')-{})",
            var,
            var,
            replace_store_and_local(&val, vars)
        );
    }
    if let Some((var, val)) = parse_compound_assign(compiled, "*=") {
        return format!(
            "S.set('{}',S.get('{}')*{})",
            var,
            var,
            replace_store_and_local(&val, vars)
        );
    }
    if let Some((var, val)) = parse_compound_assign(compiled, "/=") {
        return format!(
            "S.set('{}',S.get('{}')/{})",
            var,
            var,
            replace_store_and_local(&val, vars)
        );
    }
    // Local state simple assign: count = max(0, count - 1)
    if let Some((var, val)) = parse_simple_assign(compiled) {
        // Check membership against the raw pairs to avoid a HashSet lookup
        if vars.0.iter().any(|(v, _)| v == &var) {
            let replaced = replace_utils_short(&replace_store_and_local(&val, vars));
            return format!("S.set('{var}',{replaced})");
        }
    }
    // Reactive list mutations: items.push(x), items.remove(i), items.clear()
    if let Some(result) = compile_list_method(compiled, vars) {
        return result;
    }
    // Navigation
    if compiled.contains("webcore_navigate(") {
        return compile_navigate_call(compiled);
    }
    // emit("eventName") / emit("eventName", data) — inter-component events
    if compiled.contains("emit(") {
        return compile_emit_call(compiled, vars);
    }
    // Default: replace all variables and utils
    let result = replace_store_and_local(compiled, vars);
    replace_utils_short(&result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cv(names: &[&str]) -> CompiledVars {
        let set: HashSet<String> = names.iter().map(|s| s.to_string()).collect();
        CompiledVars::new(&set)
    }

    #[test]
    fn test_compile_increment() {
        let result = compile_expression_full("count += 1", &cv(&["count"]));
        assert_eq!(result, "S.set('count',S.get('count')+1)");
    }

    #[test]
    fn test_compile_decrement() {
        let result = compile_expression_full("count -= 1", &cv(&["count"]));
        assert_eq!(result, "S.set('count',S.get('count')-1)");
    }

    #[test]
    fn test_compile_assignment_with_max() {
        let result = compile_expression_full("count = max(0, count - 1)", &cv(&["count"]));
        assert!(result.starts_with("S.set('count',"));
        assert!(result.contains("U.max"));
        assert!(result.contains("S.get('count')"));
    }

    #[test]
    fn test_compile_navigate() {
        let result = compile_expression_full("webcore_navigate(/about)", &cv(&[]));
        assert_eq!(result, "nav('/about')");
    }

    #[test]
    fn test_multiple_variables() {
        let result = compile_expression_full("total = x + y", &cv(&["x", "y", "total"]));
        assert!(result.starts_with("S.set('total',"));
        assert!(result.contains("S.get('x')"));
        assert!(result.contains("S.get('y')"));
    }

    #[test]
    fn test_property_access_not_replaced() {
        // `i.done` must not become `i.S.get('done')` — dot-prefixed uses are object properties
        let vars = cv(&["items", "done"]);
        let expr = "(items ?? []).filter(i => i.done).length";
        let result = vars.replace_into(expr);
        assert!(
            result.contains("i.done"),
            "property access i.done should be preserved, got: {result}"
        );
        assert!(
            !result.contains("i.S.get"),
            "i.S.get must not appear, got: {result}"
        );
        assert!(
            result.contains("S.get('items')"),
            "standalone items should be replaced"
        );
    }

    #[test]
    fn test_standalone_var_still_replaced() {
        let vars = cv(&["done", "total"]);
        let result = vars.replace_into("done > 0 && total > 1");
        assert_eq!(result, "S.get('done') > 0 && S.get('total') > 1");
    }
}
