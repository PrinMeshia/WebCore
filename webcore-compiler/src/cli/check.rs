//! Project validation: parse all sources and report issues without writing output.
//!
//! `check_project(false)` prints a human summary; `check_project(true)`
//! (`webc check --json`) prints a single-line [`CheckReport`] JSON object on
//! stdout for editors and tooling (VS Code extension, future LSP).

use super::config::read_config;
use super::loader::{load_webc_document, LoadError};
use crate::core::ast;
use crate::core::diag::{CheckReport, Diagnostic, Severity};

/// Validate the current project without generating any output files.
///
/// Parses all `.webc` sources, checks component references, route targets,
/// and prop type mismatches. In JSON mode all output (including failures)
/// goes to stdout as one JSON line and the returned error string is empty —
/// the caller only uses it for the exit code.
pub(crate) fn check_project(json: bool) -> Result<(), String> {
    if !json {
        println!("🔍 Checking WebCore project...");
    }

    let config = match read_config() {
        Ok(c) => c,
        Err(e) => {
            let diag = Diagnostic {
                severity: Severity::Error,
                code: "config",
                message: e.clone(),
                file: Some("webc.toml".to_string()),
                line: None,
                col: None,
            };
            return fail_early(json, diag, e);
        }
    };

    let document = match load_webc_document(&config.locale) {
        Ok(d) => d,
        Err(load_err) => {
            let human = load_err.to_string();
            return fail_early(json, diag_from_load_error(load_err), human);
        }
    };

    let mut issues: Vec<Diagnostic> = Vec::new();

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
                issues.push(Diagnostic::project_error(
                    "route-target",
                    format!("route \"{route}\" → {target} : page/component not found"),
                ));
            }
        }
    }

    // 2. Component references in pages/layouts/components exist
    fn check_elements(
        elements: &[ast::Element],
        document: &ast::WebCoreDocument,
        _file: Option<&std::path::Path>,
        issues: &mut Vec<Diagnostic>,
    ) {
        for elem in elements {
            match elem {
                ast::Element::Component {
                    name,
                    content,
                    span: _span,
                    ..
                } => {
                    if !document.components.contains_key(name) {
                        issues.push(Diagnostic::project_error(
                            "unknown-component",
                            format!("component <{name}> used but not declared"),
                        ));
                    }
                    check_elements(content, document, _file, issues);
                }
                ast::Element::Tag { content, .. }
                | ast::Element::For { content, .. }
                | ast::Element::SlotContent { content, .. }
                | ast::Element::ErrorBlock { content, .. } => {
                    check_elements(content, document, _file, issues)
                }
                ast::Element::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    check_elements(then_branch, document, _file, issues);
                    if let Some(eb) = else_branch {
                        check_elements(eb, document, _file, issues);
                    }
                }
                _ => {}
            }
        }
    }
    let file_of = |name: &str| document.source_files.get(name).map(|p| p.as_path());
    for (name, page) in &document.pages {
        check_elements(&page.content, &document, file_of(name), &mut issues);
    }
    for (name, layout) in &document.layouts {
        check_elements(&layout.content, &document, file_of(name), &mut issues);
    }
    for (name, component) in &document.components {
        check_elements(&component.view, &document, file_of(name), &mut issues);
    }

    // 3. Prop type mismatches (static props vs declared type)
    fn check_props(
        elements: &[ast::Element],
        document: &ast::WebCoreDocument,
        _file: Option<&std::path::Path>,
        issues: &mut Vec<Diagnostic>,
    ) {
        for elem in elements {
            if let ast::Element::Component {
                name,
                attributes,
                content,
                span: _span,
            } = elem
            {
                if let Some(comp) = document.components.get(name) {
                    for attr in attributes {
                        if let ast::AttributeValue::String(val) = &attr.value {
                            if let Some(prop) = comp.props.iter().find(|p| p.name == attr.name) {
                                if let Some(t) = &prop.type_ {
                                    match t.as_str() {
                                        "Number" if val.parse::<f64>().is_err() => {
                                            issues.push(Diagnostic::project_error(
                                                "prop-type",
                                                format!(
                                                    "{}:{} — prop \"{}\" expects Number, got \"{}\"",
                                                    name, attr.name, attr.name, val
                                                ),
                                            ));
                                        }
                                        "Boolean" if val != "true" && val != "false" => {
                                            issues.push(Diagnostic::project_error(
                                                "prop-type",
                                                format!(
                                                    "{}:{} — prop \"{}\" expects Boolean, got \"{}\"",
                                                    name, attr.name, attr.name, val
                                                ),
                                            ));
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                }
                check_props(content, document, _file, issues);
            } else {
                match elem {
                    ast::Element::Tag { content, .. } | ast::Element::For { content, .. } => {
                        check_props(content, document, _file, issues)
                    }
                    ast::Element::If {
                        then_branch,
                        else_branch,
                        ..
                    } => {
                        check_props(then_branch, document, _file, issues);
                        if let Some(eb) = else_branch {
                            check_props(eb, document, _file, issues);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    for (name, page) in &document.pages {
        check_props(&page.content, &document, file_of(name), &mut issues);
    }

    // 4. Circular component references
    fn check_cycles(
        component_name: &str,
        document: &ast::WebCoreDocument,
        stack: &mut Vec<String>,
        issues: &mut Vec<Diagnostic>,
    ) {
        if stack.contains(&component_name.to_string()) {
            issues.push(Diagnostic::project_error(
                "circular-ref",
                format!(
                    "circular component reference: {} → {}",
                    stack.join(" → "),
                    component_name
                ),
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
        issues: &mut Vec<Diagnostic>,
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

    // ── Report ───────────────────────────────────────────────────────────────
    if json {
        let report = CheckReport::new(issues);
        println!("{}", report.to_json());
        return if report.ok {
            Ok(())
        } else {
            Err(String::new())
        };
    }

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
            println!("  {}", issue.message);
        }
        Err(format!(
            "\n{} issue{} — fix before building",
            issues.len(),
            if issues.len() == 1 { "" } else { "s" }
        ))
    }
}

/// Convert a loader failure into a positioned diagnostic when possible.
fn diag_from_load_error(err: LoadError) -> Diagnostic {
    match err {
        LoadError::Parse(pe) => Diagnostic {
            severity: Severity::Error,
            code: "parse",
            message: pe.concise_message(),
            file: pe.file.as_ref().map(|p| p.display().to_string()),
            line: pe.span.as_ref().map(|s| s.line),
            col: pe.span.as_ref().map(|s| s.col),
        },
        LoadError::Other(msg) => Diagnostic::project_error("load", msg),
    }
}

/// Early failure (config/load): emit the JSON report or the human message.
fn fail_early(json: bool, diag: Diagnostic, human_message: String) -> Result<(), String> {
    if json {
        println!("{}", CheckReport::new(vec![diag]).to_json());
        // Message already printed as JSON — the caller only needs the exit code.
        Err(String::new())
    } else {
        Err(human_message)
    }
}
