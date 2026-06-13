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

// No panic on user input: every `.unwrap()` outside tests must be replaced by
// error propagation, a guarded alternative, or an `.expect()` whose message
// documents why it cannot fail (e.g. `write!` into a `String`).
#![cfg_attr(not(test), warn(clippy::unwrap_used))]

mod cli;
pub(crate) mod codegen {
    pub(crate) mod attr_names;
    pub(crate) mod css;
    pub(crate) mod html;
    pub(crate) mod js;
}
pub(crate) mod core;
mod parser;
#[cfg(test)]
mod tests;

fn main() {
    cli::run();
}
