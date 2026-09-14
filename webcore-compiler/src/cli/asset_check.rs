//! Orphan-asset detection for `webc check` (#71).
//!
//! Warns about files in `public/` that are referenced nowhere in the project
//! sources (`.webc`, `webc.toml`, `theme.toml`, `data/*`). **Best-effort and
//! warning-only** (`orphan-asset`): references built dynamically at runtime
//! cannot be seen statically, so matching is on the file's *basename* to stay
//! conservative — better to miss an orphan than to wrongly flag a used asset.
//!
//! Internal docs (`.md`) are ignored: they are never deployed (see #64).

use crate::core::ast::WebCoreDocument;
use crate::core::diag::Diagnostic;
use std::fs;
use std::path::Path;

use super::loader::walk_files;

/// Report public assets that no source file references.
pub(crate) fn lint(document: &WebCoreDocument) -> Vec<Diagnostic> {
    let public = Path::new("public");
    if !public.is_dir() {
        return Vec::new();
    }

    // Haystack: every source that could name an asset (src/*.webc + config).
    let mut haystack = String::new();
    for path in document.source_files.values() {
        if let Ok(s) = fs::read_to_string(path) {
            haystack.push_str(&s);
            haystack.push('\n');
        }
    }
    // `src/app.webc` (the app/head declaration) is loaded but not tracked in
    // `source_files`, yet it names assets — `favicon "/assets/favicon.svg"`,
    // head `link`/`meta`. Read it explicitly so those aren't flagged orphan.
    for extra in ["webc.toml", "theme.toml", "src/app.webc"] {
        if let Ok(s) = fs::read_to_string(extra) {
            haystack.push_str(&s);
            haystack.push('\n');
        }
    }
    // Data collections (`data/*.json` / `.toml`) can reference assets by path in
    // their fields (e.g. a `image` / `url` column), so scan them too — otherwise
    // an asset used only from data is wrongly flagged orphan.
    let data_dir = Path::new("data");
    if data_dir.is_dir() {
        let _ = walk_files(data_dir, |p| {
            if let Ok(s) = fs::read_to_string(p) {
                haystack.push_str(&s);
                haystack.push('\n');
            }
            Ok(())
        });
    }

    let mut orphans: Vec<String> = Vec::new();
    let _ = walk_files(public, |p| {
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
        let ext = p
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        // `.md` files are never deployed; don't flag them as orphans.
        if name.is_empty() || ext == "md" {
            return Ok(());
        }
        let rel = p
            .strip_prefix(public)
            .unwrap_or(p)
            .to_string_lossy()
            .replace('\\', "/");
        // Top-level `public/*.css` is auto-linked into every `<head>` by the
        // build (see `extra_css_files`), so it is never named in the sources.
        if ext == "css" && !rel.contains('/') {
            return Ok(());
        }
        if !haystack.contains(name) {
            orphans.push(rel);
        }
        Ok(())
    });

    orphans.sort();
    orphans
        .into_iter()
        .map(|rel| {
            Diagnostic::project_warning(
                "orphan-asset",
                format!("public/{rel} is referenced nowhere in the sources"),
            )
        })
        .collect()
}
