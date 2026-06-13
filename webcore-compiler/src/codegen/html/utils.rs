/// HTML-escape a string (& < > " ').
pub(super) fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

/// Derive a safe HTML-id-compatible prefix from a name (lowercase alphanumeric, max 12 chars).
pub(super) fn safe_id_prefix(name: &str) -> String {
    let s: String = name
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .take(12)
        .collect();
    if s.is_empty() {
        "p".to_string()
    } else {
        s
    }
}

/// Extract path from `webcore_navigate(path)` expression
pub(super) fn extract_navigate_path(expr: &str) -> Option<String> {
    // Match webcore_navigate(/path) or webcore_navigate(root) or webcore_navigate("/path")
    let expr = expr.trim();

    if let Some(start) = expr.find("webcore_navigate(") {
        let after_paren = &expr[start + 17..]; // After "webcore_navigate("
        if let Some(end) = after_paren.find(')') {
            let path = after_paren[..end].trim();

            // Handle different path formats
            let clean_path = if path == "root" {
                "/".to_string()
            } else if path.starts_with('"') && path.ends_with('"') {
                // Quoted path: "/about"
                path[1..path.len() - 1].to_string()
            } else if path.starts_with('/') {
                // Unquoted path: /about
                path.to_string()
            } else {
                // Fallback
                format!("/{path}")
            };

            return Some(clean_path);
        }
    }
    None
}
