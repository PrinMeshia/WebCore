//! Project validation: parse all sources and report issues without writing output.

use crate::core::ast;
use super::config::read_config;
use super::loader::load_webc_document;

/// Validate the current project without generating any output files.
///
/// Parses all `.webc` sources, checks component references, route targets,
/// and prop type mismatches. Reports issues and exits cleanly.
pub(crate) fn check_project() -> Result<(), String> {
    println!("🔍 Checking WebCore project...");

    let config = read_config()?;
    let document = load_webc_document(&config.locale)?;

    let mut issues: Vec<String> = Vec::new();

    // 1. Route targets exist as pages or components
    if let Some(app) = &document.app {
        for (route, target) in &app.routes {
            let normalized = if target.ends_with("Page") {
                target[..target.len() - 4].to_lowercase()
            } else {
                target.to_lowercase()
            };
            let exists = document.pages.contains_key(target)
                || document.pages.contains_key(&normalized)
                || document.components.contains_key(target);
            if !exists {
                issues.push(format!(
                    "  route \"{route}\" → {target} : page/component not found"
                ));
            }
        }
    }

    // 2. Component references in pages/layouts/components exist
    fn check_elements(
        elements: &[ast::Element],
        document: &ast::WebCoreDocument,
        issues: &mut Vec<String>,
    ) {
        for elem in elements {
            match elem {
                ast::Element::Component { name, content, .. } => {
                    if !document.components.contains_key(name) {
                        issues.push(format!("  component <{name}> used but not declared"));
                    }
                    check_elements(content, document, issues);
                }
                ast::Element::Tag { content, .. }
                | ast::Element::For { content, .. }
                | ast::Element::SlotContent { content, .. }
                | ast::Element::ErrorBlock { content, .. } => {
                    check_elements(content, document, issues)
                }
                ast::Element::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    check_elements(then_branch, document, issues);
                    if let Some(eb) = else_branch {
                        check_elements(eb, document, issues);
                    }
                }
                _ => {}
            }
        }
    }
    for page in document.pages.values() {
        check_elements(&page.content, &document, &mut issues);
    }
    for layout in document.layouts.values() {
        check_elements(&layout.content, &document, &mut issues);
    }
    for component in document.components.values() {
        check_elements(&component.view, &document, &mut issues);
    }

    // 3. Prop type mismatches (static props vs declared type)
    fn check_props(
        elements: &[ast::Element],
        document: &ast::WebCoreDocument,
        issues: &mut Vec<String>,
    ) {
        for elem in elements {
            if let ast::Element::Component {
                name,
                attributes,
                content,
                ..
            } = elem
            {
                if let Some(comp) = document.components.get(name) {
                    for attr in attributes {
                        if let ast::AttributeValue::String(val) = &attr.value {
                            if let Some(prop) = comp.props.iter().find(|p| p.name == attr.name) {
                                if let Some(t) = &prop.type_ {
                                    match t.as_str() {
                                        "Number" if val.parse::<f64>().is_err() => {
                                            issues.push(format!(
                                                "  {}:{} — prop \"{}\" expects Number, got \"{}\"",
                                                name, attr.name, attr.name, val
                                            ));
                                        }
                                        "Boolean" if val != "true" && val != "false" => {
                                            issues.push(format!(
                                                "  {}:{} — prop \"{}\" expects Boolean, got \"{}\"",
                                                name, attr.name, attr.name, val
                                            ));
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                }
                check_props(content, document, issues);
            } else {
                match elem {
                    ast::Element::Tag { content, .. } | ast::Element::For { content, .. } => {
                        check_props(content, document, issues)
                    }
                    ast::Element::If {
                        then_branch,
                        else_branch,
                        ..
                    } => {
                        check_props(then_branch, document, issues);
                        if let Some(eb) = else_branch {
                            check_props(eb, document, issues);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    for page in document.pages.values() {
        check_props(&page.content, &document, &mut issues);
    }

    // 4. Circular component references
    fn check_cycles(
        component_name: &str,
        document: &ast::WebCoreDocument,
        stack: &mut Vec<String>,
        issues: &mut Vec<String>,
    ) {
        if stack.contains(&component_name.to_string()) {
            issues.push(format!(
                "  circular component reference: {} → {}",
                stack.join(" → "),
                component_name
            ));
            return;
        }
        if let Some(comp) = document.components.get(component_name) {
            stack.push(component_name.to_string());
            collect_component_refs(&comp.view, document, stack, issues);
            stack.pop();
        }
    }

    fn collect_component_refs(
        elements: &[ast::Element],
        document: &ast::WebCoreDocument,
        stack: &mut Vec<String>,
        issues: &mut Vec<String>,
    ) {
        for elem in elements {
            match elem {
                ast::Element::Component { name, content, .. } => {
                    check_cycles(name, document, stack, issues);
                    collect_component_refs(content, document, stack, issues);
                }
                _ => collect_component_refs(elem.children(), document, stack, issues),
            }
        }
    }

    for component_name in document.components.keys() {
        check_cycles(component_name, &document, &mut Vec::new(), &mut issues);
    }

    // Summary
    let pages = document.pages.len();
    let components = document.components.len();
    let layouts = document.layouts.len();

    if issues.is_empty() {
        println!(
            "✅  {} page{}, {} component{}, {} layout{} — no issues found",
            pages,
            if pages == 1 { "" } else { "s" },
            components,
            if components == 1 { "" } else { "s" },
            layouts,
            if layouts == 1 { "" } else { "s" },
        );
        Ok(())
    } else {
        println!(
            "❌  {} issue{} found:\n",
            issues.len(),
            if issues.len() == 1 { "" } else { "s" }
        );
        for issue in &issues {
            println!("{issue}");
        }
        Err(format!(
            "\n{} issue{} — fix before building",
            issues.len(),
            if issues.len() == 1 { "" } else { "s" }
        ))
    }
}
