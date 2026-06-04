//! CSS Code Generator with Scoped Styles

use crate::ast::{Component, StyleItem, StyleRule, WebCoreDocument};
use crate::theme::Theme;

/// Generate a unique scope ID for a component based on its name.
/// Uses FNV-1a 32-bit hash — deterministic across compilations and Rust versions.
pub fn generate_scope_id(component_name: &str) -> String {
    let mut hash: u32 = 2166136261;
    for byte in component_name.bytes() {
        hash ^= byte as u32;
        hash = hash.wrapping_mul(16777619);
    }
    format!("wc-{:06x}", hash & 0xFFFFFF)
}

fn emit_scoped_rule(rule: &StyleRule, scope_id: &str, indent: &str) -> String {
    let mut css = String::new();
    let scoped_selector = scope_selector(&rule.selector, scope_id);
    css.push_str(&format!("{}{} {{\n", indent, scoped_selector));
    for prop in &rule.properties {
        css.push_str(&format!("{}  {}: {};\n", indent, prop.name, prop.value));
    }
    css.push_str(&format!("{}}}\n", indent));
    css
}

/// Generate scoped CSS for a single component
pub fn generate_scoped_css(component: &Component) -> String {
    if component.style.is_empty() {
        return String::new();
    }

    let scope_id = generate_scope_id(&component.name);
    let mut css = String::new();

    css.push_str(&format!("/* Component: {} */\n", component.name));

    for item in &component.style {
        match item {
            StyleItem::Rule(rule) => {
                css.push_str(&emit_scoped_rule(rule, &scope_id, ""));
            }
            StyleItem::Media { query, rules, .. } => {
                css.push_str(&format!("@media {} {{\n", query));
                for rule in rules {
                    css.push_str(&emit_scoped_rule(rule, &scope_id, "  "));
                }
                css.push_str("}\n");
            }
        }
    }

    css.push('\n');
    css
}

/// Scope a CSS selector by prepending [data-v="scope-id"]
fn scope_selector(selector: &str, scope_id: &str) -> String {
    // Handle comma-separated selectors
    selector
        .split(',')
        .map(|s| scope_single_selector(s.trim(), scope_id))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Scope a single selector (no commas)
fn scope_single_selector(selector: &str, scope_id: &str) -> String {
    let selector = selector.trim();

    // Handle special selectors
    if selector == ":root" || selector == "html" || selector == "body" {
        return selector.to_string(); // Don't scope global selectors
    }

    // Handle :host for web components style
    if selector.starts_with(":host") {
        return format!("[data-v=\"{}\"]", scope_id);
    }

    // Handle :global() escape hatch
    if selector.starts_with(":global(") && selector.ends_with(")") {
        return selector[8..selector.len() - 1].to_string();
    }

    // Handle pseudo-elements - they must come after the scope attribute
    // e.g., "button::before" -> "[data-v=xxx] button::before"
    if let Some(pos) = selector.find("::") {
        let (base, pseudo) = selector.split_at(pos);
        return format!("[data-v=\"{}\"] {}{}", scope_id, base, pseudo);
    }

    // Handle pseudo-classes that should stay attached
    // e.g., "button:hover" -> "[data-v=xxx] button:hover"
    // But ":first-child" at start means scope the container
    if selector.starts_with(':') && !selector.starts_with("::") {
        return format!("[data-v=\"{}\"]{}", scope_id, selector);
    }

    // Standard case: prepend scope
    format!("[data-v=\"{}\"] {}", scope_id, selector)
}

/// Generate all scoped CSS for a document
pub fn generate_all_scoped_css(document: &WebCoreDocument) -> String {
    let mut css = String::new();

    css.push_str("/* WebCore Scoped Styles */\n\n");

    for component in document.components.values() {
        css.push_str(&generate_scoped_css(component));
    }

    css
}

/// Generate combined CSS: theme variables + scoped component styles
pub fn generate_combined_css(theme: Option<&Theme>, document: &WebCoreDocument) -> String {
    let mut css = String::new();

    // Theme variables
    if let Some(theme) = theme {
        css.push_str(&generate_theme_css(theme));
        css.push('\n');
    }

    // Modern minimal base styles
    css.push_str(
        r#"/* WebCore Base */
*, *::before, *::after { box-sizing: border-box; }
body {
  margin: 0;
  font-family: var(--font-base, system-ui, -apple-system, sans-serif);
  background: var(--color-background, #fafafa);
  color: var(--color-text, #111);
  line-height: 1.5;
}
a { color: var(--color-primary, #1e88e5); text-decoration: none; }
a:hover { text-decoration: underline; }
button {
  font: inherit;
  cursor: pointer;
  padding: 0.5em 1.25em;
  border: none;
  border-radius: var(--radius-button, 6px);
  background: var(--color-primary, #1e88e5);
  color: var(--color-onPrimary, #fff);
  transition: opacity 0.15s;
}
button:hover { opacity: 0.85; }
header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 1rem 2rem;
  background: var(--color-primary, #1e88e5);
  color: var(--color-onPrimary, #fff);
}
header a { color: inherit; margin-left: 1.5rem; }
header nav { display: flex; gap: 1rem; }
main { padding: 2rem; max-width: 960px; margin: 0 auto; }
footer {
  padding: 1.5rem 2rem;
  text-align: center;
  color: var(--color-text-muted, #6b7280);
  font-size: 0.875rem;
}
h1, h2, h3 { font-family: var(--font-heading, inherit); margin: 0 0 0.5em; }
p { margin: 0 0 1em; }
"#,
    );
    css.push('\n');

    // Scoped component styles
    css.push_str(&generate_all_scoped_css(document));

    css
}

pub fn generate_css() -> String {
    "/* CSS output placeholder */".to_string()
}

pub fn generate_theme_css(theme: &Theme) -> String {
    let mut css = String::new();
    css.push_str(":root {\n");

    // Generate color variables
    for (key, value) in &theme.colors {
        css.push_str(&format!("  --color-{}: {};\n", key, value));
    }

    // Generate font variables
    for (key, value) in &theme.fonts {
        css.push_str(&format!("  --font-{}: {};\n", key, value));
    }

    // Generate spacing variables
    for (key, value) in &theme.spacing {
        css.push_str(&format!("  --space-{}: {};\n", key, value));
    }

    // Generate radius variables
    for (key, value) in &theme.radius {
        css.push_str(&format!("  --radius-{}: {};\n", key, value));
    }

    // Generate breakpoint variables
    for (key, value) in &theme.breakpoints {
        css.push_str(&format!("  --breakpoint-{}: {};\n", key, value));
    }

    css.push_str("}\n");
    css
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scope_id_generation() {
        let id1 = generate_scope_id("Button");
        let id2 = generate_scope_id("Button");
        let id3 = generate_scope_id("Card");

        assert_eq!(id1, id2); // Same component = same ID
        assert_ne!(id1, id3); // Different component = different ID
        assert!(id1.starts_with("wc-"));
    }

    #[test]
    fn test_scope_simple_selector() {
        let result = scope_selector("button", "wc-abc123");
        assert_eq!(result, "[data-v=\"wc-abc123\"] button");
    }

    #[test]
    fn test_scope_class_selector() {
        let result = scope_selector(".my-class", "wc-abc123");
        assert_eq!(result, "[data-v=\"wc-abc123\"] .my-class");
    }

    #[test]
    fn test_scope_multiple_selectors() {
        let result = scope_selector("h1, h2, h3", "wc-abc123");
        assert_eq!(
            result,
            "[data-v=\"wc-abc123\"] h1, [data-v=\"wc-abc123\"] h2, [data-v=\"wc-abc123\"] h3"
        );
    }

    #[test]
    fn test_scope_pseudo_class() {
        let result = scope_selector("button:hover", "wc-abc123");
        assert_eq!(result, "[data-v=\"wc-abc123\"] button:hover");
    }

    #[test]
    fn test_scope_pseudo_element() {
        let result = scope_selector("button::before", "wc-abc123");
        assert_eq!(result, "[data-v=\"wc-abc123\"] button::before");
    }

    #[test]
    fn test_global_escape() {
        let result = scope_selector(":global(body)", "wc-abc123");
        assert_eq!(result, "body");
    }

    #[test]
    fn test_root_not_scoped() {
        let result = scope_selector(":root", "wc-abc123");
        assert_eq!(result, ":root");
    }
}
