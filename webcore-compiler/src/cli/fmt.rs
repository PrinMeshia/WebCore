//! `webc fmt` — format .webc source files from the parsed AST.

use std::path::{Path, PathBuf};

use crate::core::ast::{
    Attribute, AttributeValue, Component, Element, Layout, Page, StyleItem, StyleProperty,
    StyleRule,
};
use crate::parser::parse_webc;

pub struct FmtOptions {
    /// Spaces per indent level for DSL blocks. Default: 4.
    pub indent: usize,
}

impl Default for FmtOptions {
    fn default() -> Self {
        Self { indent: 4 }
    }
}

// ─── Public API ──────────────────────────────────────────────────────────────

/// Format a single .webc file in place. Returns (formatted_source, was_changed).
pub fn format_file(path: &Path, opts: &FmtOptions) -> Result<(String, bool), String> {
    let source =
        std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let formatted = format_webc(&source, opts)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let changed = formatted != source;
    Ok((formatted, changed))
}

/// Discover all .webc files under `root`.
pub fn collect_webc_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect_recursive(root, &mut out);
    out.sort();
    out
}

fn collect_recursive(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_recursive(&path, out);
        } else if path.extension().is_some_and(|e| e == "webc") {
            out.push(path);
        }
    }
}

/// Run the `fmt` command from CLI args.
pub fn run(paths: &[String], check: bool, indent: usize) -> Result<(), String> {
    let opts = FmtOptions { indent };
    let target_files: Vec<PathBuf> = if paths.is_empty() {
        collect_webc_files(Path::new("src"))
    } else {
        paths.iter().map(PathBuf::from).collect()
    };

    if target_files.is_empty() {
        println!("No .webc files found.");
        return Ok(());
    }

    let mut changed_count = 0usize;
    for path in &target_files {
        let (formatted, changed) = format_file(path, &opts)?;
        if changed {
            changed_count += 1;
            if check {
                eprintln!("would reformat: {}", path.display());
            } else {
                std::fs::write(path, &formatted)
                    .map_err(|e| format!("{}: {e}", path.display()))?;
                println!("reformatted: {}", path.display());
            }
        }
    }

    if check && changed_count > 0 {
        eprintln!("{changed_count} file(s) would be reformatted.");
        std::process::exit(1);
    }

    if !check {
        let total = target_files.len();
        println!("{total} file(s) checked, {changed_count} reformatted.");
    }

    Ok(())
}

// ─── Core formatter ──────────────────────────────────────────────────────────

/// Parse and format a .webc source string. Returns the formatted source.
pub fn format_webc(source: &str, opts: &FmtOptions) -> Result<String, String> {
    let doc = parse_webc(source).map_err(|e| e.to_string())?;

    let mut out = String::new();

    // Imports
    for imp in &doc.imports {
        out.push_str(&format!("import {} from \"{}\"\n", imp.name, imp.path));
    }
    if !doc.imports.is_empty() {
        out.push('\n');
    }

    // App declaration
    if let Some(app) = &doc.app {
        out.push_str(&format!("app {} {{\n", app.name));
        if let Some(theme) = &app.theme {
            out.push_str(&format!("{}theme: \"{theme}\"\n", i(opts.indent)));
        }
        if let Some(layout) = &app.layout {
            out.push_str(&format!("{}layout: {layout}\n", i(opts.indent)));
        }
        if !app.routes.is_empty() {
            out.push_str(&format!("{}routes {{\n", i(opts.indent)));
            let mut routes: Vec<_> = app.routes.iter().collect();
            routes.sort_by_key(|(k, _)| k.len());
            for (route, component) in &routes {
                let coll = app.collections.get(*route).map(|c| format!(" each {c}")).unwrap_or_default();
                out.push_str(&format!("{}\"{}\":{}{component}{coll}\n", i(opts.indent * 2), route, col_pad(route)));
            }
            out.push_str(&format!("{}}}\n", i(opts.indent)));
        }
        out.push_str("}\n");
        out.push('\n');
    }

    // Store
    if !doc.store.is_empty() {
        out.push_str("store {\n");
        for sv in &doc.store {
            let default = sv.default_value.as_deref().map(|v| format!(" = {v}")).unwrap_or_default();
            out.push_str(&format!("{}{}:{}{}{default}\n", i(opts.indent), sv.name, col_pad(&sv.name), sv.type_));
        }
        out.push_str("}\n\n");
    }

    // Layouts (alphabetical)
    let mut layout_names: Vec<_> = doc.layouts.keys().cloned().collect();
    layout_names.sort();
    for name in &layout_names {
        let layout = &doc.layouts[name];
        out.push_str(&format_layout(layout, opts));
        out.push('\n');
    }

    // Components (alphabetical)
    let mut comp_names: Vec<_> = doc.components.keys().cloned().collect();
    comp_names.sort();
    for name in &comp_names {
        let comp = &doc.components[name];
        out.push_str(&format_component(comp, opts));
        out.push('\n');
    }

    // Pages (alphabetical by route name)
    let mut page_names: Vec<_> = doc.pages.keys().cloned().collect();
    page_names.sort();
    for name in &page_names {
        let page = &doc.pages[name];
        out.push_str(&format_page(page, opts));
        out.push('\n');
    }

    // Trim trailing newlines to exactly one
    let trimmed = out.trim_end_matches('\n');
    Ok(format!("{trimmed}\n"))
}

// ─── Declaration formatters ──────────────────────────────────────────────────

fn format_layout(layout: &Layout, opts: &FmtOptions) -> String {
    let mut out = format!("layout {} {{\n", layout.name);
    for el in &layout.content {
        out.push_str(&format_element(el, 1, opts));
    }
    out.push_str("}\n");
    out
}

fn format_page(page: &Page, opts: &FmtOptions) -> String {
    let mut out = format!("page \"{}\" {{\n", page.name);
    for el in &page.content {
        out.push_str(&format_element(el, 1, opts));
    }
    out.push_str("}\n");
    out
}

fn format_component(comp: &Component, opts: &FmtOptions) -> String {
    let mut out = format!("component {} {{\n", comp.name);

    // props
    if !comp.props.is_empty() {
        out.push_str(&format!("{}props {{\n", i(opts.indent)));
        for p in &comp.props {
            let ty = p.type_.as_deref().map(|t| format!(": {t}")).unwrap_or_default();
            let def = p.default_value.as_deref().map(|v| format!(" = {v}")).unwrap_or_default();
            out.push_str(&format!("{}{}{ty}{def}\n", i(opts.indent * 2), p.name));
        }
        out.push_str(&format!("{}}}\n", i(opts.indent)));
    }

    // state
    if !comp.state.is_empty() {
        out.push_str(&format!("{}state {{\n", i(opts.indent)));
        for sv in &comp.state {
            let default = sv.default_value.as_deref().map(|v| format!(" = {v}")).unwrap_or_default();
            out.push_str(&format!("{}{}: {}{default}\n", i(opts.indent * 2), sv.name, sv.type_));
        }
        out.push_str(&format!("{}}}\n", i(opts.indent)));
    }

    // computed
    if !comp.computed.is_empty() {
        out.push_str(&format!("{}computed {{\n", i(opts.indent)));
        for cv in &comp.computed {
            out.push_str(&format!("{}{} = {}\n", i(opts.indent * 2), cv.name, cv.expr));
        }
        out.push_str(&format!("{}}}\n", i(opts.indent)));
    }

    // http
    if let Some(http) = &comp.http {
        out.push_str(&format!("{}http {{\n", i(opts.indent)));
        out.push_str(&format!("{}get: \"{}\"\n", i(opts.indent * 2), http.url));
        out.push_str(&format!("{}into: {}\n", i(opts.indent * 2), http.into));
        out.push_str(&format!("{}}}\n", i(opts.indent)));
    }

    // on:mount
    if let Some(body) = &comp.mount_body {
        out.push_str(&format!("{}on:mount {{\n", i(opts.indent)));
        for line in body.lines() {
            if line.trim().is_empty() {
                out.push('\n');
            } else {
                out.push_str(&format!("{}{}\n", i(opts.indent * 2), line.trim_start()));
            }
        }
        out.push_str(&format!("{}}}\n", i(opts.indent)));
    }

    // on:destroy
    if let Some(body) = &comp.destroy_body {
        out.push_str(&format!("{}on:destroy {{\n", i(opts.indent)));
        for line in body.lines() {
            if line.trim().is_empty() {
                out.push('\n');
            } else {
                out.push_str(&format!("{}{}\n", i(opts.indent * 2), line.trim_start()));
            }
        }
        out.push_str(&format!("{}}}\n", i(opts.indent)));
    }

    // view
    if !comp.view.is_empty() {
        out.push_str(&format!("{}view {{\n", i(opts.indent)));
        for el in &comp.view {
            out.push_str(&format_element(el, 2, opts));
        }
        out.push_str(&format!("{}}}\n", i(opts.indent)));
    }

    // style
    if !comp.style.is_empty() {
        out.push_str(&format!("{}style {{\n", i(opts.indent)));
        for item in &comp.style {
            out.push_str(&format_style_item(item, 2, opts));
        }
        out.push_str(&format!("{}}}\n", i(opts.indent)));
    }

    out.push_str("}\n");
    out
}

// ─── Element formatter ───────────────────────────────────────────────────────

pub fn format_element(el: &Element, depth: usize, opts: &FmtOptions) -> String {
    let ind = i(depth * opts.indent);
    match el {
        Element::Text(s, _) => {
            format!("{ind}\"{s}\"\n")
        }
        Element::Interpolation(expr, _) => {
            format!("{ind}\"{{{expr}}}\"\n")
        }
        Element::Slot(name, _) => {
            if name.is_empty() {
                format!("{ind}slot\n")
            } else {
                format!("{ind}slot {name}\n")
            }
        }
        Element::SlotContent { name, content, .. } => {
            let mut out = format!("{ind}slot {name} {{\n");
            for child in content {
                out.push_str(&format_element(child, depth + 1, opts));
            }
            out.push_str(&format!("{ind}}}\n"));
            out
        }
        Element::Tag { name, attributes, content, .. } => {
            format_tag(name, attributes, content, depth, opts)
        }
        Element::Component { name, attributes, content, .. } => {
            format_tag(name, attributes, content, depth, opts)
        }
        Element::For { item, index, iterable, key, content, .. } => {
            let index_part = index.as_deref().map(|i| format!(", {i}")).unwrap_or_default();
            let key_part = key.as_deref().map(|k| format!(" key={k}")).unwrap_or_default();
            let mut out = format!("{ind}@for {item}{index_part}{key_part} in {iterable} {{\n");
            for child in content {
                out.push_str(&format_element(child, depth + 1, opts));
            }
            out.push_str(&format!("{ind}}}\n"));
            out
        }
        Element::If { condition, then_branch, else_branch, .. } => {
            let mut out = format!("{ind}@if {condition} {{\n");
            for child in then_branch {
                out.push_str(&format_element(child, depth + 1, opts));
            }
            out.push_str(&format!("{ind}}}"));
            if let Some(else_els) = else_branch {
                out.push_str(" @else {\n");
                for child in else_els {
                    out.push_str(&format_element(child, depth + 1, opts));
                }
                out.push_str(&format!("{ind}}}"));
            }
            out.push('\n');
            out
        }
        Element::ErrorBlock { field, content, .. } => {
            let mut out = format!("{ind}@error \"{field}\" {{\n");
            for child in content {
                out.push_str(&format_element(child, depth + 1, opts));
            }
            out.push_str(&format!("{ind}}}\n"));
            out
        }
        Element::Fragment { content, .. } => {
            let mut out = format!("{ind}<>\n");
            for child in content {
                out.push_str(&format_element(child, depth + 1, opts));
            }
            out.push_str(&format!("{ind}</>\n"));
            out
        }
    }
}

fn format_tag(
    name: &str,
    attrs: &[Attribute],
    content: &[Element],
    depth: usize,
    opts: &FmtOptions,
) -> String {
    let ind = i(depth * opts.indent);

    // Format attribute strings
    let attr_strs: Vec<String> = attrs.iter().map(format_attribute).collect();

    // Decide inline vs block layout for attributes
    let attrs_inline = if attrs.len() < 3 {
        Some(attr_strs.join(" "))
    } else {
        None // use multi-line
    };

    // Decide content rendering
    let content_str = inline_content(content, opts);

    if attrs.is_empty() {
        if content.is_empty() {
            return format!("{ind}{name} {{}}\n");
        }
        if let Some(inline) = &content_str {
            let candidate = format!("{ind}{name} {{ {inline} }}\n");
            if candidate.len() - ind.len() <= 80 {
                return candidate;
            }
        }
        // Block content
        let mut out = format!("{ind}{name} {{\n");
        for child in content {
            out.push_str(&format_element(child, depth + 1, opts));
        }
        out.push_str(&format!("{ind}}}\n"));
        return out;
    }

    // With attributes
    if let Some(a) = &attrs_inline {
        if content.is_empty() {
            return format!("{ind}{name} {a} {{}}\n");
        }
        if let Some(inline) = &content_str {
            let candidate = format!("{ind}{name} {a} {{ {inline} }}\n");
            if candidate.len() - ind.len() <= 80 {
                return candidate;
            }
        }
        // Block content, inline attrs
        let mut out = format!("{ind}{name} {a} {{\n");
        for child in content {
            out.push_str(&format_element(child, depth + 1, opts));
        }
        out.push_str(&format!("{ind}}}\n"));
        return out;
    }

    // 3+ attributes: each on its own line, closing { on its own line
    let inner_ind = i((depth + 1) * opts.indent);
    let mut out = format!("{ind}{name}\n");
    for a in &attr_strs {
        out.push_str(&format!("{inner_ind}{a}\n"));
    }
    if content.is_empty() {
        out.push_str(&format!("{ind}{{}}\n"));
        return out;
    }
    out.push_str(&format!("{ind}{{\n"));
    for child in content {
        out.push_str(&format_element(child, depth + 1, opts));
    }
    out.push_str(&format!("{ind}}}\n"));
    out
}

/// Returns a single-line content string if the content is simple enough, else None.
fn inline_content(content: &[Element], _opts: &FmtOptions) -> Option<String> {
    if content.len() == 1 {
        match &content[0] {
            Element::Text(s, _) => {
                if s.len() < 60 {
                    return Some(format!("\"{s}\""));
                }
            }
            Element::Interpolation(expr, _) => {
                return Some(format!("\"{{{expr}}}\""));
            }
            _ => {}
        }
    }
    None
}

fn format_attribute(attr: &Attribute) -> String {
    match &attr.value {
        AttributeValue::String(s) => format!("{}=\"{}\"", attr.name, s),
        AttributeValue::Expression(e) => format!("{}={{{e}}}", attr.name),
        AttributeValue::Boolean(true) => attr.name.clone(),
        AttributeValue::Boolean(false) => format!("{}=false", attr.name),
    }
}

// ─── Style formatter ─────────────────────────────────────────────────────────

fn format_style_item(item: &StyleItem, depth: usize, opts: &FmtOptions) -> String {
    match item {
        StyleItem::Rule(rule) => format_style_rule(rule, depth, opts),
        StyleItem::Media { query, rules, .. } => {
            let ind = i(depth * opts.indent);
            let mut out = format!("{ind}@media {query} {{\n");
            for rule in rules {
                out.push_str(&format_style_rule(rule, depth + 1, opts));
            }
            out.push_str(&format!("{ind}}}\n"));
            out
        }
        StyleItem::Keyframes { name, steps } => {
            let ind = i(depth * opts.indent);
            let mut out = format!("{ind}@keyframes {name} {{\n");
            for step in steps {
                let step_ind = i((depth + 1) * opts.indent);
                out.push_str(&format!("{step_ind}{} {{\n", step.selector));
                for prop in &step.properties {
                    out.push_str(&format_style_prop(prop, depth + 2, opts));
                }
                out.push_str(&format!("{step_ind}}}\n"));
            }
            out.push_str(&format!("{ind}}}\n"));
            out
        }
    }
}

fn format_style_rule(rule: &StyleRule, depth: usize, opts: &FmtOptions) -> String {
    let ind = i(depth * opts.indent);
    // Try single-line for rules with 1-2 short properties and no nested rules
    if rule.nested.is_empty() && !rule.properties.is_empty() {
        let props_line: String = rule
            .properties
            .iter()
            .map(|p| format!("{}: {};", p.name, p.value))
            .collect::<Vec<_>>()
            .join(" ");
        let candidate = format!("{ind}{} {{ {props_line} }}\n", rule.selector);
        if candidate.len() - ind.len() <= 80 {
            return candidate;
        }
    }
    let mut out = format!("{ind}{} {{\n", rule.selector);
    for prop in &rule.properties {
        out.push_str(&format_style_prop(prop, depth + 1, opts));
    }
    for nested in &rule.nested {
        out.push_str(&format_style_rule(nested, depth + 1, opts));
    }
    out.push_str(&format!("{ind}}}\n"));
    out
}

fn format_style_prop(prop: &StyleProperty, depth: usize, opts: &FmtOptions) -> String {
    format!("{}{}: {};\n", i(depth * opts.indent), prop.name, prop.value)
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Returns a string of `n` spaces.
fn i(n: usize) -> String {
    " ".repeat(n)
}

/// Padding to align route values in the routes table.
fn col_pad(route: &str) -> &'static str {
    match route.len() {
        0..=3 => "          ",
        4..=7 => "       ",
        8..=11 => "    ",
        12..=15 => " ",
        _ => " ",
    }
}
