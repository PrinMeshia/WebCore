//! Golden / integration tests: full parse → codegen pipeline.
//!
//! Shared helpers and re-exports for domain-specific test sub-modules.

#[cfg(test)]
pub(crate) use crate::core::ast::{self, Component, Span, WebCoreDocument};
#[cfg(test)]
pub(crate) use crate::codegen::attr_names;
#[cfg(test)]
pub(crate) use crate::codegen::css::generate_combined_css;
#[cfg(test)]
pub(crate) use crate::codegen::html::{generate_html, HtmlPageOptions};
#[cfg(test)]
pub(crate) use crate::codegen::js::{generate_runtime_js, minify_js};
#[cfg(test)]
pub(crate) use crate::parser::parse_webc;
#[cfg(test)]
pub(crate) use std::collections::HashMap;

#[cfg(test)]
mod css;
#[cfg(test)]
mod errors;
#[cfg(test)]
mod features;
#[cfg(test)]
mod html;
#[cfg(test)]
mod i18n;
#[cfg(test)]
mod js;
#[cfg(test)]
mod security;
#[cfg(test)]
mod ssg;

#[cfg(test)]
pub(crate) fn opts() -> HtmlPageOptions {
    HtmlPageOptions {
        lang: "en".into(),
        title: "Test".into(),
        extra_css_files: vec![],
    }
}

/// Parse + generate HTML for page "home".  Panics on parse or codegen error.
#[cfg(test)]
pub(crate) fn compile_to_html(src: &str) -> String {
    let doc = parse_webc(src).expect("parse");
    generate_html(&doc, "home", &opts()).expect("codegen").html
}

/// Parse + generate the runtime JS (no HTML handlers).  Panics on parse error.
#[cfg(test)]
pub(crate) fn compile_to_js(src: &str) -> String {
    let doc = parse_webc(src).expect("parse");
    generate_runtime_js(&[], &doc)
}

/// Parse, generate HTML for page "home", then generate the full runtime JS
/// (including HTML-collected event handlers).  Returns (html, js).
#[cfg(test)]
pub(crate) fn compile_full(src: &str) -> (String, String) {
    let doc = parse_webc(src).expect("parse");
    let res = generate_html(&doc, "home", &opts()).expect("codegen");
    let js = generate_runtime_js(&res.handlers, &doc);
    (res.html, js)
}
