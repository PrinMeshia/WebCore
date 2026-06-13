//! Core compiler primitives: AST, error types, theme model, SSG evaluation,
//! shared utilities and CSS post-processing. No CLI or filesystem I/O lives here.

pub(crate) mod ast;
pub(crate) mod css_processor;
pub(crate) mod error;
pub(crate) mod ssg;
pub(crate) mod theme;
pub(crate) mod utils;
