//! Event-handler and expression compilation helpers.
//!
//! All functions here compile `WebCore` expression strings (e.g. `count += 1`,
//! `$store.x = y`, `emit("ping")`) into their JavaScript equivalents.

use regex::Regex;
use std::collections::HashSet;

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
pub(super) fn replace_vars_short(expr: &str, state_vars: &HashSet<String>) -> String {
    let mut result = expr.to_string();
    let mut vars: Vec<_> = state_vars.iter().collect();
    vars.sort_by_key(|v| std::cmp::Reverse(v.len()));

    for var in vars {
        let pattern = format!(r"\b{}\b", regex::escape(var));
        if let Ok(re) = Regex::new(&pattern) {
            result = re
                .replace_all(&result, format!("S.get('{var}')").as_str())
                .to_string();
        }
    }
    result
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
pub(super) fn replace_store_and_local(expr: &str, state_vars: &HashSet<String>) -> String {
    let re_store = Regex::new(r"\$store\.([a-zA-Z_][a-zA-Z0-9_]*)").expect("hardcoded $store.var regex is always valid");
    // Step 1: sentinel so local-var replacement can't touch store refs
    let sentineled = re_store.replace_all(expr, "__STORE_$1__").to_string();
    // Step 2: replace local state vars  (_ is a word char → \bcount\b won't match inside __STORE_count__)
    let with_local = replace_vars_short(&sentineled, state_vars);
    // Step 3: restore sentinels → STORE.get('...')
    let re_sent = Regex::new(r"__STORE_([a-zA-Z_][a-zA-Z0-9_]*)__").expect("hardcoded store-sentinel regex is always valid");
    re_sent
        .replace_all(&with_local, "STORE.get('$1')")
        .to_string()
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
pub(super) fn compile_emit_call(expr: &str, state_vars: &HashSet<String>) -> String {
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
            let detail = replace_utils_short(&replace_store_and_local(detail_raw, state_vars));
            return format!(
                "document.dispatchEvent(new CustomEvent({event_name},{{detail:{detail}}}))"
            );
        }
    }
    let result = replace_store_and_local(expr, state_vars);
    replace_utils_short(&result)
}

/// Full expression compiler: handles both `$store.var` and local state vars.
pub(crate) fn compile_expression_full(expr: &str, state_vars: &HashSet<String>) -> String {
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
        return format!("STORE.set('{var}',STORE.get('{var}')+{rhs})");
    }
    if let Some((var, val)) = parse_store_compound_assign(compiled, "-=") {
        let rhs = replace_utils_short(&replace_store_and_local(&val, state_vars));
        return format!("STORE.set('{var}',STORE.get('{var}')-{rhs})");
    }
    if let Some((var, val)) = parse_store_compound_assign(compiled, "*=") {
        let rhs = replace_utils_short(&replace_store_and_local(&val, state_vars));
        return format!("STORE.set('{var}',STORE.get('{var}')*{rhs})");
    }
    if let Some((var, val)) = parse_store_compound_assign(compiled, "/=") {
        let rhs = replace_utils_short(&replace_store_and_local(&val, state_vars));
        return format!("STORE.set('{var}',STORE.get('{var}')/{rhs})");
    }
    // Store simple assign: $store.count = value
    if let Some((var, val)) = parse_store_simple_assign(compiled) {
        let rhs = replace_utils_short(&replace_store_and_local(&val, state_vars));
        return format!("STORE.set('{var}',{rhs})");
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
            return format!("S.set('{var}',{replaced})");
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
