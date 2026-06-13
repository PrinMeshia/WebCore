//! Structured compiler error type for `WebCore`.
//!
//! All public codegen functions return `Result<T, CompileError>` instead of
//! `Result<T, String>`.  Internal helpers that still use `String` errors can
//! propagate via `?` thanks to `impl From<String> for CompileError`.

use crate::ast::Span;
use std::path::PathBuf;

#[derive(Debug)]
pub enum CompileError {
    /// Parse failure with source location (reserved for future LSP/error-recovery use).
    #[allow(dead_code)]
    Parse {
        file: PathBuf,
        message: String,
        span: Option<Span>,
    },
    /// A referenced layout does not exist.
    MissingLayout {
        name: String,
        available: Vec<String>,
    },
    /// A referenced page does not exist.
    MissingPage { name: String },
    /// A referenced component does not exist (reserved for future LSP use).
    #[allow(dead_code)]
    MissingComponent { name: String },
    /// I/O error (file read, write, etc.).
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    /// Generic catch-all for messages not yet migrated.
    Custom(String),
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompileError::Parse {
                file,
                message,
                span,
            } => {
                if let Some(sp) = span {
                    write!(
                        f,
                        "Parse error in {} ({}:{}): {}",
                        file.display(),
                        sp.line,
                        sp.col,
                        message
                    )
                } else {
                    write!(f, "Parse error in {}: {}", file.display(), message)
                }
            }
            CompileError::MissingLayout { name, available } => write!(
                f,
                "Layout '{}' not found. Available layouts: [{}]",
                name,
                available.join(", ")
            ),
            CompileError::MissingPage { name } => write!(f, "Page '{}' not found", name),
            CompileError::MissingComponent { name } => {
                write!(f, "Component '{}' not found", name)
            }
            CompileError::Io { path, source } => {
                write!(f, "I/O error for '{}': {}", path.display(), source)
            }
            CompileError::Custom(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for CompileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CompileError::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<String> for CompileError {
    fn from(s: String) -> Self {
        Self::Custom(s)
    }
}

impl From<&str> for CompileError {
    fn from(s: &str) -> Self {
        Self::Custom(s.to_owned())
    }
}

/// Holds multiple compile errors collected during a build.
///
/// Returned by `build_project()` so that **all** errors are reported together
/// (like `rustc`) rather than stopping at the first one.
#[derive(Debug)]
pub struct CompileErrors(pub Vec<CompileError>);

impl std::fmt::Display for CompileErrors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for e in &self.0 {
            writeln!(f, "{e}")?;
        }
        write!(f, "\n{} error(s) found.", self.0.len())
    }
}

impl std::error::Error for CompileErrors {}

impl From<CompileError> for CompileErrors {
    fn from(e: CompileError) -> Self {
        Self(vec![e])
    }
}

impl From<String> for CompileErrors {
    fn from(s: String) -> Self {
        Self(vec![CompileError::Custom(s)])
    }
}

/// Find the closest match using Levenshtein distance (threshold: 3 edits).
#[cfg(test)]
pub fn find_closest_match<'a>(
    needle: &str,
    haystack: impl Iterator<Item = &'a str>,
) -> Option<String> {
    let mut best_match = None;
    let mut best_distance = usize::MAX;

    for candidate in haystack {
        let distance = levenshtein_distance(needle, candidate);
        if distance < best_distance && distance <= 3 {
            best_distance = distance;
            best_match = Some(candidate.to_string());
        }
    }

    best_match
}

/// Calculate Levenshtein distance between two strings.
#[cfg(test)]
fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let a_len = a_chars.len();
    let b_len = b_chars.len();

    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }

    let mut matrix = vec![vec![0usize; b_len + 1]; a_len + 1];

    for (i, row) in matrix.iter_mut().enumerate().take(a_len + 1) {
        row[0] = i;
    }
    for (j, cell) in matrix[0].iter_mut().enumerate().take(b_len + 1) {
        *cell = j;
    }

    for i in 1..=a_len {
        for j in 1..=b_len {
            let cost = usize::from(a_chars[i - 1] != b_chars[j - 1]);
            matrix[i][j] = (matrix[i - 1][j] + 1)
                .min(matrix[i][j - 1] + 1)
                .min(matrix[i - 1][j - 1] + cost);
        }
    }

    matrix[a_len][b_len]
}

#[cfg(test)]
mod error_utils_tests {
    use super::*;

    #[test]
    fn test_levenshtein_distance() {
        assert_eq!(levenshtein_distance("Button", "Button"), 0);
        assert_eq!(levenshtein_distance("Buton", "Button"), 1);
        assert_eq!(levenshtein_distance("Btton", "Button"), 1);
        assert_eq!(levenshtein_distance("abc", "xyz"), 3);
    }

    #[test]
    fn test_find_closest_match() {
        let candidates = vec!["Button", "Card", "Input", "Modal"];
        let result = find_closest_match("Buton", candidates.iter().map(|s| *s));
        assert_eq!(result, Some("Button".to_string()));
    }

    #[test]
    fn test_find_closest_no_match() {
        let candidates = vec!["Button", "Card"];
        let result = find_closest_match("XYZ", candidates.iter().map(|s| *s));
        assert_eq!(result, None);
    }
}
