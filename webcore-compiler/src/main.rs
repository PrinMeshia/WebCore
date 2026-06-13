//! `WebCore` compiler — CLI entry point.
//!
//! Commands:
//! - `webc new <name>` — scaffold a new project
//! - `webc build`      — compile `.webc` files to `dist/` (HTML + CSS + JS)
//! - `webc dev [port]` — build + serve with hot-reload via WebSocket
//! - `webc check`      — validate the project without generating output
//!
//! Build pipeline (see `build::build_project`):
//! 1. Parse `webc.toml` for project config
//! 2. Load and parse all `.webc` source files into a `WebCoreDocument`
//! 3. For each page: generate HTML (with SSG pre-rendering), collect JS handlers
//! 4. Generate a single `webcore.js` runtime (tree-shaken per document)
//! 5. Generate `theme.css` + scoped component CSS
//! 6. Write everything to `dist/`

mod ast;
mod build;
mod check;
mod cli;
pub(crate) mod codegen {
    pub(crate) mod attr_names;
    pub(crate) mod codegen_css;
    pub(crate) mod codegen_html;
    pub(crate) mod codegen_js;
}
mod css_processor;
pub(crate) mod error;
mod parser;
mod serve;
mod ssg;
#[cfg(test)]
mod tests;
mod theme;

fn main() {
    cli::run();
}
