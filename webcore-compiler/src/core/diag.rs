//! Structured diagnostics for editor and tooling consumption.
//!
//! `webc check --json` serializes a [`CheckReport`] to stdout; the VS Code
//! extension (and any future LSP server) maps each [`Diagnostic`] to an
//! in-editor squiggle. Positions are 1-based, mirroring compiler messages.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    /// Reserved for non-fatal check diagnostics (e.g. unknown props),
    /// not emitted by `webc check` yet.
    #[allow(dead_code)]
    Warning,
}

#[derive(Debug, Serialize)]
pub struct Diagnostic {
    pub severity: Severity,
    /// Stable machine-readable category: `parse`, `config`, `io`,
    /// `route-target`, `unknown-component`, `prop-type`, `circular-ref`.
    pub code: &'static str,
    pub message: String,
    /// Project-relative source path, when the diagnostic maps to a file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// 1-based line, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    /// 1-based column, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub col: Option<u32>,
}

impl Diagnostic {
    /// Project-level error without a source position.
    pub fn project_error(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            code,
            message: message.into(),
            file: None,
            line: None,
            col: None,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct CheckReport {
    pub ok: bool,
    pub diagnostics: Vec<Diagnostic>,
}

impl CheckReport {
    pub fn new(diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            ok: diagnostics.is_empty(),
            diagnostics,
        }
    }

    /// Serialize to a single JSON line (always valid JSON, even on error).
    pub fn to_json(&self) -> String {
        serde_json::to_string(self)
            .unwrap_or_else(|_| r#"{"ok":false,"diagnostics":[]}"#.to_string())
    }
}
