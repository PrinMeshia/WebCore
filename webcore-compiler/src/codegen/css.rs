//! CSS Code Generator with Scoped Styles

use crate::core::ast::{Component, KeyframeStep, StyleItem, StyleRule, WebCoreDocument};
use crate::core::theme::Theme;
use std::fmt::Write as _;

// FNV-1a 32-bit constants (https://tools.ietf.org/html/draft-eastlake-fnv)
const FNV_OFFSET_BASIS: u32 = 2_166_136_261;
const FNV_PRIME: u32 = 16_777_619;

/// Generate a unique scope ID for a component based on its name.
/// Uses FNV-1a 32-bit hash — deterministic across compilations and Rust versions.
#[must_use]
pub(crate) fn generate_scope_id(component_name: &str) -> String {
    let mut hash: u32 = FNV_OFFSET_BASIS;
    for byte in component_name.bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    format!("wc-{:06x}", hash & 0xFFFFFF)
}

/// Known standard CSS property names. Custom properties (`--var`) are always allowed.
const KNOWN_CSS_PROPS: &[&str] = &[
    "color",
    "background",
    "background-color",
    "background-image",
    "background-size",
    "background-position",
    "background-repeat",
    "background-attachment",
    "margin",
    "margin-top",
    "margin-right",
    "margin-bottom",
    "margin-left",
    "padding",
    "padding-top",
    "padding-right",
    "padding-bottom",
    "padding-left",
    "border",
    "border-top",
    "border-right",
    "border-bottom",
    "border-left",
    "border-radius",
    "border-color",
    "border-width",
    "border-style",
    "width",
    "height",
    "min-width",
    "max-width",
    "min-height",
    "max-height",
    "display",
    "flex",
    "flex-direction",
    "flex-wrap",
    "flex-grow",
    "flex-shrink",
    "justify-content",
    "align-items",
    "align-self",
    "align-content",
    "grid",
    "grid-template-columns",
    "grid-template-rows",
    "grid-column",
    "grid-row",
    "gap",
    "column-gap",
    "row-gap",
    "grid-area",
    "grid-template-areas",
    "position",
    "top",
    "right",
    "bottom",
    "left",
    "z-index",
    "float",
    "clear",
    "font-size",
    "font-weight",
    "font-family",
    "font-style",
    "font-variant",
    "line-height",
    "text-align",
    "text-decoration",
    "text-transform",
    "text-overflow",
    "letter-spacing",
    "overflow",
    "overflow-x",
    "overflow-y",
    "cursor",
    "opacity",
    "visibility",
    "pointer-events",
    "transition",
    "transform",
    "animation",
    "animation-name",
    "animation-duration",
    "box-shadow",
    "text-shadow",
    "outline",
    "list-style",
    "content",
    "white-space",
    "word-break",
    "word-wrap",
    "vertical-align",
    "object-fit",
    "object-position",
    "aspect-ratio",
    "resize",
];

/// Emit a warning to stderr if `prop_name` is not a known CSS property.
/// Custom properties (starting with `--`) are always allowed without warning.
fn warn_unknown_css_prop(prop_name: &str, context: &str) {
    if !prop_name.starts_with("--") && !KNOWN_CSS_PROPS.contains(&prop_name) {
        eprintln!("warning[css]: unknown property '{prop_name}' in {context}");
    }
}

fn emit_keyframes(name: &str, steps: &[KeyframeStep]) -> String {
    let mut css = String::new();
    writeln!(css, "@keyframes {name} {{").unwrap();
    for step in steps {
        writeln!(css, "  {} {{", step.selector).unwrap();
        for prop in &step.properties {
            writeln!(css, "    {}: {};", prop.name, prop.value).unwrap();
        }
        css.push_str("  }\n");
    }
    css.push_str("}\n");
    css
}

fn emit_scoped_rule(rule: &StyleRule, scope_id: &str, indent: &str) -> String {
    let mut css = String::new();
    let scoped_selector = scope_selector(&rule.selector, scope_id);

    // Only emit parent block when it has direct properties
    if !rule.properties.is_empty() {
        writeln!(css, "{indent}{scoped_selector} {{").unwrap();
        for prop in &rule.properties {
            warn_unknown_css_prop(&prop.name, &format!("[{}]", scope_id));
            writeln!(css, "{}  {}: {};", indent, prop.name, prop.value).unwrap();
        }
        writeln!(css, "{indent}}}").unwrap();
    }

    // Flatten nested rules: `&:hover` → `<scoped_selector>:hover`
    for nested in &rule.nested {
        let flat_selector = if nested.selector.starts_with('&') {
            // Replace leading `&` with the already-scoped parent selector
            format!("{}{}", scoped_selector, &nested.selector[1..])
        } else {
            format!("{} {}", scoped_selector, nested.selector)
        };
        writeln!(css, "{indent}{flat_selector} {{").unwrap();
        for prop in &nested.properties {
            warn_unknown_css_prop(&prop.name, &format!("[{}]", scope_id));
            writeln!(css, "{}  {}: {};", indent, prop.name, prop.value).unwrap();
        }
        writeln!(css, "{indent}}}").unwrap();
    }

    css
}

/// Generate scoped CSS for a single component
#[must_use]
pub(crate) fn generate_scoped_css(component: &Component) -> String {
    if component.style.is_empty() {
        return String::new();
    }

    let scope_id = generate_scope_id(&component.name);
    let mut css = String::new();

    writeln!(css, "/* Component: {} */", component.name).unwrap();

    for item in &component.style {
        match item {
            StyleItem::Rule(rule) => {
                css.push_str(&emit_scoped_rule(rule, &scope_id, ""));
            }
            StyleItem::Media { query, rules, .. } => {
                writeln!(css, "@media {query} {{").unwrap();
                for rule in rules {
                    css.push_str(&emit_scoped_rule(rule, &scope_id, "  "));
                }
                css.push_str("}\n");
            }
            StyleItem::Keyframes { name, steps } => {
                // @keyframes are global by design — emit unscoped
                css.push_str(&emit_keyframes(name, steps));
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
        return format!("[data-v=\"{scope_id}\"]");
    }

    // Handle :global() escape hatch
    if selector.starts_with(":global(") && selector.ends_with(')') {
        return selector[8..selector.len() - 1].to_string();
    }

    // Handle pseudo-elements - they must come after the scope attribute
    // e.g., "button::before" -> "[data-v=xxx] button::before"
    if let Some(pos) = selector.find("::") {
        let (base, pseudo) = selector.split_at(pos);
        return format!("[data-v=\"{scope_id}\"] {base}{pseudo}");
    }

    // Handle pseudo-classes that should stay attached
    // e.g., "button:hover" -> "[data-v=xxx] button:hover"
    // But ":first-child" at start means scope the container
    if selector.starts_with(':') && !selector.starts_with("::") {
        return format!("[data-v=\"{scope_id}\"]{selector}");
    }

    // Standard case: prepend scope
    format!("[data-v=\"{scope_id}\"] {selector}")
}

/// Generate all scoped CSS for a document
#[must_use]
pub(crate) fn generate_all_scoped_css(document: &WebCoreDocument) -> String {
    let mut css = String::new();

    css.push_str("/* WebCore Scoped Styles */\n\n");

    for component in document.components.values() {
        css.push_str(&generate_scoped_css(component));
    }

    css
}

/// Generate the global (non-component) CSS: theme variables + base styles.
/// This is the part of the stylesheet shared by every page.
#[must_use]
pub(crate) fn generate_global_css(theme: Option<&Theme>) -> String {
    let mut css = String::new();

    // Theme variables
    if let Some(theme) = theme {
        css.push_str(&generate_theme_css(theme));
        css.push('\n');
    }

    // Modern minimal base styles
    css.push_str(
        r"/* WebCore Base */
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
",
    );
    css.push('\n');
    css
}

/// Generate combined CSS: theme variables + scoped component styles
#[must_use]
pub(crate) fn generate_combined_css(theme: Option<&Theme>, document: &WebCoreDocument) -> String {
    let mut css = generate_global_css(theme);

    // Scoped component styles
    css.push_str(&generate_all_scoped_css(document));

    css
}

#[must_use]
pub(crate) fn generate_theme_css(theme: &Theme) -> String {
    let mut css = String::new();
    css.push_str(":root {\n");

    // Generate color variables
    for (key, value) in &theme.colors {
        writeln!(css, "  --color-{key}: {value};").unwrap();
    }

    // Generate font variables
    for (key, value) in &theme.fonts {
        writeln!(css, "  --font-{key}: {value};").unwrap();
    }

    // Generate spacing variables
    for (key, value) in &theme.spacing {
        writeln!(css, "  --space-{key}: {value};").unwrap();
    }

    // Generate radius variables
    for (key, value) in &theme.radius {
        writeln!(css, "  --radius-{key}: {value};").unwrap();
    }

    // Generate breakpoint variables
    for (key, value) in &theme.breakpoints {
        writeln!(css, "  --breakpoint-{key}: {value};").unwrap();
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
