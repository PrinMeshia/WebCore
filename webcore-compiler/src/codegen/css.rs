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
    // Box model / logical properties
    "box-sizing",
    "inset",
    "inset-block",
    "inset-block-start",
    "inset-block-end",
    "inset-inline",
    "inset-inline-start",
    "inset-inline-end",
    "margin-block",
    "margin-block-start",
    "margin-block-end",
    "margin-inline",
    "margin-inline-start",
    "margin-inline-end",
    "padding-block",
    "padding-block-start",
    "padding-block-end",
    "padding-inline",
    "padding-inline-start",
    "padding-inline-end",
    "inline-size",
    "block-size",
    "min-inline-size",
    "max-inline-size",
    "min-block-size",
    "max-block-size",
    // Flexbox / grid extras
    "flex-basis",
    "flex-flow",
    "order",
    "place-items",
    "place-content",
    "place-self",
    "justify-items",
    "justify-self",
    "grid-template",
    "grid-auto-flow",
    "grid-auto-rows",
    "grid-auto-columns",
    "grid-column-start",
    "grid-column-end",
    "grid-row-start",
    "grid-row-end",
    // Multi-column
    "columns",
    "column-count",
    "column-width",
    "column-rule",
    "column-span",
    // Filters / compositing
    "filter",
    "backdrop-filter",
    "mix-blend-mode",
    "background-blend-mode",
    "background-clip",
    "background-origin",
    "mask",
    "mask-image",
    "clip-path",
    "isolation",
    // Text / font extras
    "tab-size",
    "text-indent",
    "text-wrap",
    "text-decoration-color",
    "text-decoration-line",
    "text-decoration-style",
    "text-decoration-thickness",
    "text-underline-offset",
    "word-spacing",
    "overflow-wrap",
    "hyphens",
    "writing-mode",
    "direction",
    "font",
    "font-stretch",
    "font-feature-settings",
    "font-kerning",
    "src",
    "unicode-range",
    // Transforms / transitions / animation longhands
    "transform-origin",
    "transform-style",
    "perspective",
    "perspective-origin",
    "backface-visibility",
    "transition-property",
    "transition-duration",
    "transition-timing-function",
    "transition-delay",
    "animation-timing-function",
    "animation-delay",
    "animation-iteration-count",
    "animation-direction",
    "animation-fill-mode",
    "animation-play-state",
    // Tables
    "border-collapse",
    "border-spacing",
    "table-layout",
    "caption-side",
    "empty-cells",
    // Lists / counters
    "list-style-type",
    "list-style-position",
    "list-style-image",
    "counter-reset",
    "counter-increment",
    "quotes",
    // Interaction / misc modern
    "user-select",
    "touch-action",
    "appearance",
    "accent-color",
    "caret-color",
    "color-scheme",
    "scroll-behavior",
    "scroll-margin",
    "scroll-padding",
    "scroll-snap-type",
    "scroll-snap-align",
    "overscroll-behavior",
    "overscroll-behavior-x",
    "overscroll-behavior-y",
    "will-change",
    "contain",
    "content-visibility",
    "image-rendering",
    "outline-offset",
    "outline-color",
    "outline-width",
    "outline-style",
];

/// Whether `prop_name` is a recognised CSS property (and so should not warn).
///
/// Two kinds of property are always accepted:
/// - custom properties (`--var`), and
/// - vendor-prefixed properties (`-webkit-…`, `-moz-…`, `-ms-…`, `-o-…`),
///
/// both of which start with `-` — a leading dash is enough to distinguish them
/// from a standard property, so we skip the allowlist check for anything that
/// begins with one. Everything else must appear in [`KNOWN_CSS_PROPS`].
fn is_known_css_prop(prop_name: &str) -> bool {
    prop_name.starts_with('-') || KNOWN_CSS_PROPS.contains(&prop_name)
}

/// Emit a warning to stderr if `prop_name` is not a known CSS property.
fn warn_unknown_css_prop(prop_name: &str, context: &str) {
    if !is_known_css_prop(prop_name) {
        eprintln!("warning[css]: unknown property '{prop_name}' in {context}");
    }
}

fn emit_keyframes(name: &str, steps: &[KeyframeStep]) -> String {
    let mut css = String::new();
    writeln!(css, "@keyframes {name} {{").expect("write! to String is infallible");
    for step in steps {
        writeln!(css, "  {} {{", step.selector).expect("write! to String is infallible");
        for prop in &step.properties {
            writeln!(css, "    {}: {};", prop.name, prop.value)
                .expect("write! to String is infallible");
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
        writeln!(css, "{indent}{scoped_selector} {{").expect("write! to String is infallible");
        for prop in &rule.properties {
            warn_unknown_css_prop(&prop.name, &format!("[{}]", scope_id));
            writeln!(css, "{}  {}: {};", indent, prop.name, prop.value)
                .expect("write! to String is infallible");
        }
        writeln!(css, "{indent}}}").expect("write! to String is infallible");
    }

    // Flatten nested rules: `&:hover` → `<scoped_selector>:hover`
    for nested in &rule.nested {
        let flat_selector = if nested.selector.starts_with('&') {
            // Replace leading `&` with the already-scoped parent selector
            format!("{}{}", scoped_selector, &nested.selector[1..])
        } else {
            format!("{} {}", scoped_selector, nested.selector)
        };
        writeln!(css, "{indent}{flat_selector} {{").expect("write! to String is infallible");
        for prop in &nested.properties {
            warn_unknown_css_prop(&prop.name, &format!("[{}]", scope_id));
            writeln!(css, "{}  {}: {};", indent, prop.name, prop.value)
                .expect("write! to String is infallible");
        }
        writeln!(css, "{indent}}}").expect("write! to String is infallible");
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

    writeln!(css, "/* Component: {} */", component.name).expect("write! to String is infallible");

    for item in &component.style {
        match item {
            StyleItem::Rule(rule) => {
                css.push_str(&emit_scoped_rule(rule, &scope_id, ""));
            }
            StyleItem::Media { query, rules, .. } => {
                writeln!(css, "@media {query} {{").expect("write! to String is infallible");
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

    // Every element rendered by the component carries the scope attribute, so we
    // append it to the *subject* (rightmost) compound of the selector. This
    // matches the component's own root element as well as its descendants — a
    // descendant-combinator prefix like `[data-v] .foo` would miss the root (#43).
    append_scope(selector, scope_id)
}

/// Append `[data-v="…"]` to the subject (rightmost) compound of a single,
/// comma-free selector, inserted before any pseudo-class/element on that
/// compound. Examples:
/// - `button`            → `button[data-v="…"]`
/// - `.a .b`             → `.a .b[data-v="…"]`
/// - `.a > .b:hover`     → `.a > .b[data-v="…"]:hover`
/// - `button::before`    → `button[data-v="…"]::before`
/// - `:hover`            → `[data-v="…"]:hover`
fn append_scope(selector: &str, scope_id: &str) -> String {
    let attr = format!("[data-v=\"{scope_id}\"]");
    let chars: Vec<char> = selector.chars().collect();

    // Subject starts just after the last top-level combinator (space, >, +, ~),
    // ignoring anything inside `(…)` or `[…]`.
    let mut depth = 0i32;
    let mut subject_start = 0usize;
    for (i, &c) in chars.iter().enumerate() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth -= 1,
            ' ' | '>' | '+' | '~' if depth == 0 => subject_start = i + 1,
            _ => {}
        }
    }

    // Within the subject, insert the attribute before the first pseudo (`:`).
    let mut pdepth = 0i32;
    let mut insert_at = chars.len();
    for (i, &c) in chars.iter().enumerate().skip(subject_start) {
        match c {
            '(' | '[' => pdepth += 1,
            ')' | ']' => pdepth -= 1,
            ':' if pdepth == 0 => {
                insert_at = i;
                break;
            }
            _ => {}
        }
    }

    let prefix: String = chars[..insert_at].iter().collect();
    let suffix: String = chars[insert_at..].iter().collect();
    format!("{prefix}{attr}{suffix}")
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

/// One CSS→`.webc` line correspondence for the dev source map (#54).
pub(crate) struct CssLineMap {
    /// 0-indexed line in the generated stylesheet.
    pub out_line: u32,
    /// Owning component (resolved to a `.webc` file via `source_files`).
    pub component: String,
    /// 1-indexed line of the rule in its `.webc` source.
    pub src_line: u32,
}

/// Generate combined CSS: theme variables + scoped component styles.
/// Test-only convenience wrapper — the build uses [`generate_combined_css_mapped`]
/// (which also yields the dev source-map data).
#[cfg(test)]
#[must_use]
pub(crate) fn generate_combined_css(theme: Option<&Theme>, document: &WebCoreDocument) -> String {
    generate_combined_css_mapped(theme, document).0
}

/// Same output as [`generate_combined_css`], plus a per-rule
/// stylesheet-line → `.webc`-line map used to emit a dev CSS source map (#54).
/// The emitted bytes are identical to `generate_combined_css`, so nothing else
/// in the pipeline changes.
pub(crate) fn generate_combined_css_mapped(
    theme: Option<&Theme>,
    document: &WebCoreDocument,
) -> (String, Vec<CssLineMap>) {
    let mut css = generate_global_css(theme);
    let mut maps: Vec<CssLineMap> = Vec::new();
    // 0-indexed line where the next character will be written.
    let mut line = css.matches('\n').count() as u32;

    // Mirror of `generate_all_scoped_css`, with output-line tracking.
    css.push_str("/* WebCore Scoped Styles */\n\n");
    line += 2;

    for component in document.components.values() {
        if component.style.is_empty() {
            continue;
        }
        let scope_id = generate_scope_id(&component.name);
        let name = component.name.clone();
        css.push_str(&format!("/* Component: {name} */\n"));
        line += 1;

        for item in &component.style {
            match item {
                StyleItem::Rule(rule) => {
                    maps.push(CssLineMap {
                        out_line: line,
                        component: name.clone(),
                        src_line: rule.span.line,
                    });
                    let chunk = emit_scoped_rule(rule, &scope_id, "");
                    line += chunk.matches('\n').count() as u32;
                    css.push_str(&chunk);
                }
                StyleItem::Media { query, rules, span } => {
                    maps.push(CssLineMap {
                        out_line: line,
                        component: name.clone(),
                        src_line: span.line,
                    });
                    css.push_str(&format!("@media {query} {{\n"));
                    line += 1;
                    for rule in rules {
                        maps.push(CssLineMap {
                            out_line: line,
                            component: name.clone(),
                            src_line: rule.span.line,
                        });
                        let chunk = emit_scoped_rule(rule, &scope_id, "  ");
                        line += chunk.matches('\n').count() as u32;
                        css.push_str(&chunk);
                    }
                    css.push_str("}\n");
                    line += 1;
                }
                StyleItem::Keyframes { name: kf, steps } => {
                    // @keyframes have no source span — emit without a mapping.
                    let chunk = emit_keyframes(kf, steps);
                    line += chunk.matches('\n').count() as u32;
                    css.push_str(&chunk);
                }
            }
        }
        css.push('\n');
        line += 1;
    }

    (css, maps)
}

#[must_use]
pub(crate) fn generate_theme_css(theme: &Theme) -> String {
    let mut css = String::new();
    css.push_str(":root {\n");

    // Generate color variables
    for (key, value) in &theme.colors {
        writeln!(css, "  --color-{key}: {value};").expect("write! to String is infallible");
    }

    // Generate font variables
    for (key, value) in &theme.fonts {
        writeln!(css, "  --font-{key}: {value};").expect("write! to String is infallible");
    }

    // Generate spacing variables
    for (key, value) in &theme.spacing {
        writeln!(css, "  --space-{key}: {value};").expect("write! to String is infallible");
    }

    // Generate radius variables
    for (key, value) in &theme.radius {
        writeln!(css, "  --radius-{key}: {value};").expect("write! to String is infallible");
    }

    // Generate breakpoint variables
    for (key, value) in &theme.breakpoints {
        writeln!(css, "  --breakpoint-{key}: {value};").expect("write! to String is infallible");
    }

    css.push_str("}\n");
    css
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_css_props_cover_modern_and_prefixed() {
        // Regression (#63): standard properties that used to false-positive.
        for p in [
            "inset",
            "backdrop-filter",
            "filter",
            "user-select",
            "tab-size",
            "background-clip",
        ] {
            assert!(is_known_css_prop(p), "'{p}' should be recognised");
        }
        // Custom properties and vendor prefixes are always accepted.
        assert!(is_known_css_prop("--brand"));
        assert!(is_known_css_prop("-webkit-backdrop-filter"));
        assert!(is_known_css_prop("-moz-user-select"));
        // A genuine typo still warns.
        assert!(!is_known_css_prop("colr"));
        assert!(!is_known_css_prop("bckground"));
    }

    #[test]
    fn test_scope_id_generation() {
        let id1 = generate_scope_id("Button");
        let id2 = generate_scope_id("Button");
        let id3 = generate_scope_id("Card");

        assert_eq!(id1, id2); // Same component = same ID
        assert_ne!(id1, id3); // Different component = different ID
        assert!(id1.starts_with("wc-"));
    }

    // The scope attribute is appended to the subject compound (Vue-style) so it
    // matches the component root as well as its descendants (#43).
    #[test]
    fn test_scope_simple_selector() {
        let result = scope_selector("button", "wc-abc123");
        assert_eq!(result, "button[data-v=\"wc-abc123\"]");
    }

    #[test]
    fn test_scope_class_selector() {
        let result = scope_selector(".my-class", "wc-abc123");
        assert_eq!(result, ".my-class[data-v=\"wc-abc123\"]");
    }

    #[test]
    fn test_scope_multiple_selectors() {
        let result = scope_selector("h1, h2, h3", "wc-abc123");
        assert_eq!(
            result,
            "h1[data-v=\"wc-abc123\"], h2[data-v=\"wc-abc123\"], h3[data-v=\"wc-abc123\"]"
        );
    }

    #[test]
    fn test_scope_pseudo_class() {
        let result = scope_selector("button:hover", "wc-abc123");
        assert_eq!(result, "button[data-v=\"wc-abc123\"]:hover");
    }

    #[test]
    fn test_scope_pseudo_element() {
        let result = scope_selector("button::before", "wc-abc123");
        assert_eq!(result, "button[data-v=\"wc-abc123\"]::before");
    }

    #[test]
    fn test_scope_descendant_selector() {
        // Subject (last compound) gets the attribute; the ancestor is context.
        let result = scope_selector(".a > .b", "wc-abc123");
        assert_eq!(result, ".a > .b[data-v=\"wc-abc123\"]");
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
