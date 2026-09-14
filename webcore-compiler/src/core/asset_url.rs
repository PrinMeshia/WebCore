//! The single definition of how a document URL maps to a file under `public/`.
//!
//! `webc build` copies `public/` into `dist/assets/`, so a file living at
//! `public/<rel>` is served at `/assets/<rel>` and nowhere else. That rule used
//! to be re-derived independently by each pass that needed it — the reference
//! rewriter read `/assets/X` as `public/X`, while the `webc:img` dimension
//! lookup read `/X` as `public/X` — and the two disagreed on every real
//! project: one convention got the dimensions, the other got a URL that
//! resolved, never both. Both now go through this module.
//!
//! Going from a URL to a path also means these functions feed `Path::join`, so
//! they reject a `..` segment: `/assets/../../etc/passwd` must not become a
//! file read outside `public/`.

/// URL prefix under which `public/` is served in the built site.
pub(crate) const PUBLIC_URL_PREFIX: &str = "/assets/";

/// `/assets/photos/hero.png` → `photos/hero.png`.
///
/// Returns `None` when the URL does not address a file of `public/`: an
/// absolute URL, a `data:` payload, a site route, or a path that would escape
/// the directory. Query string and fragment are dropped — `theme.css?v=abc`
/// names the same file as `theme.css`.
pub(crate) fn public_rel_from_url(url: &str) -> Option<&str> {
    let path = url
        .split(['?', '#'])
        .next()
        .unwrap_or(url)
        .strip_prefix(PUBLIC_URL_PREFIX)?;
    if path.is_empty() || path.split('/').any(|seg| seg == ".." || seg == ".") {
        return None;
    }
    Some(path)
}

/// `photos/hero.png` → `/assets/photos/hero.png`, the inverse of
/// [`public_rel_from_url`] for a path relative to `public/`.
pub(crate) fn url_from_public_rel(rel: &str) -> String {
    format!("{PUBLIC_URL_PREFIX}{}", rel.trim_start_matches('/'))
}

/// True when `url` looks like a file of `public/` addressed without the
/// `/assets/` prefix (`/hero.png` for `public/hero.png`).
///
/// Only used to turn a silent miss into a message that names the working form:
/// such a URL resolves to nothing in `dist/`, so an author who writes it gets a
/// 404 and no explanation.
pub(crate) fn looks_like_unprefixed_public_url(url: &str) -> Option<&str> {
    if url.starts_with(PUBLIC_URL_PREFIX) {
        return None;
    }
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let rel = path.strip_prefix('/')?;
    if rel.is_empty() || rel.split('/').any(|seg| seg == ".." || seg == ".") {
        return None;
    }
    Some(rel)
}
