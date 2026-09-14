//! Asset pipeline: hashing, fingerprinting, SRI, copy, and HTML patching.

use crate::core::asset_url::url_from_public_rel;
use crate::core::css_processor;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use super::loader::walk_files;

/// FNV-1a 32-bit hash — returns an 8 hex-char string.
#[must_use]
pub(crate) fn fnv1a_hash(data: &[u8]) -> String {
    let mut h: u32 = 2_166_136_261;
    for &b in data {
        h ^= u32::from(b);
        h = h.wrapping_mul(16_777_619);
    }
    format!("{h:08x}")
}

/// Compute a SHA-256 SRI hash string (`sha256-<base64>`) for the given data.
pub(super) fn sri_hash(data: &[u8]) -> String {
    use base64::{engine::general_purpose, Engine as _};
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(data);
    format!("sha256-{}", general_purpose::STANDARD.encode(hash))
}

/// Compute FNV-1a cache-busting hash and, when `compute_sri` is true, an SRI
/// SHA-256 hash — both in sequential passes over the same in-memory buffer.
/// Returns `(fnv_hex, Some(sri_string))` or `(fnv_hex, None)`.
pub(super) fn hash_asset(data: &[u8], compute_sri: bool) -> (String, Option<String>) {
    let fnv = fnv1a_hash(data);
    let sri = if compute_sri {
        Some(sri_hash(data))
    } else {
        None
    };
    (fnv, sri)
}

/// Image file extensions that are subject to content-hash fingerprinting.
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "svg", "ico", "avif"];

/// For every image file under `public_dir` (recursively), compute a content
/// hash, copy the file to `assets_dir/<relative-dir>/<stem>.<hash>.<ext>`, and
/// return a mapping keyed by the path **relative to `public_dir`**
/// (`"projects/webcore.png"` → `"projects/webcore.<hash>.png"`). Preserving the
/// sub-directory in both the key and the copy is what lets `rewrite_asset_refs`
/// match `/assets/projects/webcore.png` references — a flat name would only
/// match top-level images.
pub(crate) fn fingerprint_images(
    public_dir: &Path,
    assets_dir: &Path,
) -> Result<BTreeMap<String, String>, String> {
    let mut map: BTreeMap<String, String> = BTreeMap::new();

    fn walk(
        dir: &Path,
        root: &Path,
        assets_dir: &Path,
        map: &mut BTreeMap<String, String>,
    ) -> Result<(), String> {
        let entries =
            fs::read_dir(dir).map_err(|e| format!("Failed to read {}: {e}", dir.display()))?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, root, assets_dir, map)?;
                continue;
            }
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            if !IMAGE_EXTENSIONS.contains(&ext.as_str()) {
                continue;
            }
            let stem = match path.file_stem().and_then(|s| s.to_str()) {
                Some(s) => s.to_string(),
                None => continue,
            };
            let bytes =
                fs::read(&path).map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
            let hash = fnv1a_hash(&bytes);
            // Path relative to public/, forward-slash form (e.g. "projects/webcore.png").
            let rel = path.strip_prefix(root).unwrap_or(&path);
            let rel_key = rel.to_string_lossy().replace('\\', "/");
            let hashed_rel = match rel.parent() {
                Some(p) if !p.as_os_str().is_empty() => {
                    format!(
                        "{}/{stem}.{hash}.{ext}",
                        p.to_string_lossy().replace('\\', "/")
                    )
                }
                _ => format!("{stem}.{hash}.{ext}"),
            };
            let dst = assets_dir.join(&hashed_rel);
            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create {}: {e}", parent.display()))?;
            }
            fs::copy(&path, &dst).map_err(|e| {
                format!("Failed to copy {} → {}: {e}", path.display(), dst.display())
            })?;
            map.insert(rel_key, hashed_rel);
        }
        Ok(())
    }

    walk(public_dir, public_dir, assets_dir, &mut map)?;
    Ok(map)
}

/// Post-process all `.html` files under `dist_dir` and all `.css` files under
/// `dist_dir/assets/`, replacing `/assets/<original>` references with
/// `/assets/<hashed>`.
pub(crate) fn rewrite_asset_refs(dist_dir: &Path, map: &BTreeMap<String, String>) {
    // Rewrite HTML files (any depth)
    rewrite_in_dir(dist_dir, "html", map, false);
    // Rewrite CSS files in dist/assets/
    let assets_dir = dist_dir.join("assets");
    if assets_dir.is_dir() {
        rewrite_in_dir(&assets_dir, "css", map, true);
    }
}

/// Replace every `/assets/<original>` found in `value` with its hashed name.
fn swap_refs(value: &str, map: &BTreeMap<String, String>) -> String {
    let mut out = value.to_string();
    for (orig, hashed) in map {
        if out.contains(orig.as_str()) {
            out = out.replace(&url_from_public_rel(orig), &url_from_public_rel(hashed));
        }
    }
    out
}

/// Rewrite asset references inside an HTML document — **only where a reference
/// can live**: quoted attribute values (`src`, `href`, `srcset`, `content`, the
/// per-item `data-webcore-fattr-*`) and `url(…)` in inlined CSS.
///
/// The previous version ran `String::replace` over the whole file, which has no
/// idea what it is replacing: a page documenting the asset pipeline saw the
/// `/assets/hero.png` of its own code sample silently rewritten to the
/// fingerprinted name. Text is content, not a reference. Escaping does the rest
/// of the work for us — an attribute shown inside a `<code>` block is emitted as
/// `src=&quot;…&quot;`, which no longer looks like an attribute to these
/// patterns.
fn rewrite_html_refs(html: &str, map: &BTreeMap<String, String>) -> String {
    let attrs = regex::Regex::new(r#"([-\w:.@]+)="([^"<>]*)""#);
    let urls = regex::Regex::new(r#"url\((\s*['"]?)([^)'"]*)(['"]?\s*)\)"#);
    let (Ok(attrs), Ok(urls)) = (attrs, urls) else {
        return html.to_string();
    };
    let step = attrs.replace_all(html, |c: &regex::Captures| {
        format!("{}=\"{}\"", &c[1], swap_refs(&c[2], map))
    });
    urls.replace_all(&step, |c: &regex::Captures| {
        format!("url({}{}{})", &c[1], swap_refs(&c[2], map), &c[3])
    })
    .into_owned()
}

fn rewrite_in_dir(dir: &Path, ext: &str, map: &BTreeMap<String, String>, css_mode: bool) {
    let rewrite_file = |p: &Path| -> std::io::Result<()> {
        if p.extension().and_then(|e| e.to_str()) == Some(ext) {
            if let Ok(content) = fs::read_to_string(p) {
                let updated = if css_mode {
                    // In CSS: url("/assets/orig") and url('/assets/orig')
                    let mut out = content.clone();
                    for (orig, hashed) in map {
                        let dq = format!(r#"url("/assets/{orig}")"#);
                        let dq_new = format!(r#"url("/assets/{hashed}")"#);
                        let sq = format!("url('/assets/{orig}')");
                        let sq_new = format!("url('/assets/{hashed}')");
                        out = out.replace(&dq, &dq_new);
                        out = out.replace(&sq, &sq_new);
                    }
                    out
                } else {
                    rewrite_html_refs(&content, map)
                };
                if updated != content {
                    let _ = fs::write(p, updated);
                }
            }
        }
        Ok(())
    };
    if css_mode {
        // CSS mode: flat scan of a single directory (assets/), no recursion
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() {
                    let _ = rewrite_file(&p);
                }
            }
        }
    } else {
        let _ = walk_files(dir, rewrite_file);
    }
}

/// Lowercased file extension, or `""` if none.
fn ext_lower(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
}

/// Why a `public/` file is skipped by the plain copy pass.
enum SkipReason {
    /// Internal documentation (`.md`) — never deployed (#64).
    Doc,
    /// Image — copied (and content-hashed) by [`fingerprint_images`] instead,
    /// so copying it here too would just duplicate it in `dist/` (#65).
    Image,
}

/// The public-asset copy policy: which files the plain copy pass must skip.
fn public_copy_skip(path: &Path) -> Option<SkipReason> {
    match ext_lower(path).as_str() {
        "md" => Some(SkipReason::Doc),
        e if IMAGE_EXTENSIONS.contains(&e) => Some(SkipReason::Image),
        _ => None,
    }
}

pub(crate) fn copy_dir_recursive(src: &Path, dst: &Path, minify: bool) -> Result<(), String> {
    if src.is_dir() {
        let src_owned = src.to_path_buf();
        let dst_owned = dst.to_path_buf();
        fs::create_dir_all(&dst_owned)
            .map_err(|e| format!("Failed to create dir {}: {e}", dst_owned.display()))?;
        let mut skipped_docs: Vec<String> = Vec::new();
        walk_files(src, |file_path| {
            // Skip internal docs (never deployed) and images (fingerprinted
            // separately) — see `public_copy_skip`.
            match public_copy_skip(file_path) {
                Some(SkipReason::Doc) => {
                    let rel = file_path.strip_prefix(&src_owned).unwrap_or(file_path);
                    skipped_docs.push(rel.to_string_lossy().replace('\\', "/"));
                    return Ok(());
                }
                Some(SkipReason::Image) => return Ok(()),
                None => {}
            }
            let rel = file_path.strip_prefix(&src_owned).unwrap_or(file_path);
            let dst_path = dst_owned.join(rel);
            if let Some(parent) = dst_path.parent() {
                fs::create_dir_all(parent)?;
            }
            if minify && ext_lower(file_path) == "css" {
                let raw = fs::read_to_string(file_path)?;
                let minified = css_processor::minify_css(&raw).map_err(std::io::Error::other)?;
                fs::write(&dst_path, minified)?;
            } else {
                fs::copy(file_path, &dst_path)?;
            }
            Ok(())
        })
        .map_err(|e| format!("Failed to copy {}: {e}", src.display()))?;
        if !skipped_docs.is_empty() {
            eprintln!(
                "  Skipped {} internal doc file(s) in public/: {}",
                skipped_docs.len(),
                skipped_docs.join(", ")
            );
        }
    } else if minify && src.extension().and_then(|e| e.to_str()) == Some("css") {
        let raw = fs::read_to_string(src)
            .map_err(|e| format!("Failed to read {}: {e}", src.display()))?;
        let minified = css_processor::minify_css(&raw)?;
        fs::write(dst, minified).map_err(|e| format!("Failed to write {}: {e}", dst.display()))?;
    } else {
        fs::copy(src, dst)
            .map_err(|e| format!("Failed to copy {} to {}: {e}", src.display(), dst.display()))?;
    }
    Ok(())
}

/// Raster formats we can decode/resize for responsive variants.
const RASTER_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg"];

/// Generate resized width variants (`<stem>-<w>w.<ext>`, same format) for every
/// raster image under `public_dir`, written next to the original under
/// `assets_dir`. Only widths strictly smaller than the source width are made.
/// Returns the number of variant files written. (#74)
pub(crate) fn generate_image_variants(
    public_dir: &Path,
    assets_dir: &Path,
    widths: &[u32],
) -> usize {
    if widths.is_empty() {
        return 0;
    }
    let mut count = 0usize;
    let _ = walk_files(public_dir, |p| {
        if !RASTER_EXTENSIONS.contains(&ext_lower(p).as_str()) {
            return Ok(());
        }
        let img = match image::open(p) {
            Ok(i) => i,
            Err(_) => return Ok(()), // unreadable/corrupt → skip, not fatal
        };
        let (ow, oh) = (img.width(), img.height());
        let rel = p.strip_prefix(public_dir).unwrap_or(p);
        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let ext = ext_lower(p);
        let dir = rel.parent();
        for &w in widths {
            if w >= ow || w == 0 {
                continue;
            }
            let nh = ((oh as f64) * (w as f64) / (ow as f64)).round().max(1.0) as u32;
            let resized = img.resize(w, nh, image::imageops::FilterType::Lanczos3);
            let name = format!("{stem}-{w}w.{ext}");
            let out = match dir {
                Some(d) if !d.as_os_str().is_empty() => assets_dir.join(d).join(&name),
                _ => assets_dir.join(&name),
            };
            if let Some(parent) = out.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if resized.save(&out).is_ok() {
                count += 1;
            }
        }
        Ok(())
    });
    count
}

/// Post-build pass for responsive images (#74): replace each
/// `data-webcore-img="<rel>|<width>"` marker with a `srcset`/`sizes` pair
/// referencing the generated variants. When `widths` is empty the markers are
/// simply stripped (feature disabled).
pub(crate) fn apply_responsive_srcset(dist_dir: &Path, widths: &[u32], sizes: &str) {
    let re = match regex::Regex::new(r#" data-webcore-img="([^"|]+)\|(\d+)""#) {
        Ok(r) => r,
        Err(_) => return,
    };
    let _ = walk_files(dist_dir, |p| {
        if ext_lower(p) != "html" {
            return Ok(());
        }
        let html = match fs::read_to_string(p) {
            Ok(h) => h,
            Err(_) => return Ok(()),
        };
        let replaced = re.replace_all(&html, |caps: &regex::Captures| {
            let rel = &caps[1];
            let ow: u32 = caps[2].parse().unwrap_or(0);
            if widths.is_empty() {
                return String::new(); // strip the marker
            }
            let (dir, stem, ext) = split_rel(rel);
            let mut entries: Vec<String> = Vec::new();
            for &w in widths {
                if w < ow && w > 0 {
                    let path = if dir.is_empty() {
                        format!("{stem}-{w}w.{ext}")
                    } else {
                        format!("{dir}/{stem}-{w}w.{ext}")
                    };
                    entries.push(format!("{} {w}w", url_from_public_rel(&path)));
                }
            }
            // The full-resolution entry names the source image, which only
            // exists in `dist/` under its fingerprinted name. This pass runs
            // before `rewrite_asset_refs`, so that rewrite is what turns this
            // URL into the file that is actually there — emitting it after the
            // rewrite is what used to leave a 404 in every `srcset`.
            entries.push(format!("{} {ow}w", url_from_public_rel(rel)));
            format!(" srcset=\"{}\" sizes=\"{}\"", entries.join(", "), sizes)
        });
        if replaced != html {
            let _ = fs::write(p, replaced.as_ref());
        }
        Ok(())
    });
}

/// Split a public-relative path into `(dir, stem, ext)` (forward-slash dir).
fn split_rel(rel: &str) -> (String, String, String) {
    let (dir, file) = match rel.rsplit_once('/') {
        Some((d, f)) => (d.to_string(), f),
        None => (String::new(), rel),
    };
    let (stem, ext) = match file.rsplit_once('.') {
        Some((s, e)) => (s.to_string(), e.to_string()),
        None => (file.to_string(), String::new()),
    };
    (dir, stem, ext)
}

/// Insert `snippet` immediately before the first `</head>` in every HTML file
/// under `dir` (used to add feed auto-discovery links post-build). Idempotent.
pub(crate) fn inject_head_snippet(dir: &Path, snippet: &str) {
    let needle = snippet.trim();
    let _ = walk_files(dir, |p| {
        if p.extension().and_then(|e| e.to_str()) == Some("html") {
            if let Ok(html) = fs::read_to_string(p) {
                if html.contains("</head>") && !html.contains(needle) {
                    let patched = html.replacen("</head>", &format!("{snippet}</head>"), 1);
                    let _ = fs::write(p, patched);
                }
            }
        }
        Ok(())
    });
}

pub(super) fn patch_html_files(dir: &Path, js_src: &str) {
    let _ = walk_files(dir, |p| {
        if p.extension().and_then(|e| e.to_str()) == Some("html") {
            if let Ok(html) = fs::read_to_string(p) {
                let patched = html.replace(r#"src="/assets/webcore.js""#, js_src);
                if patched != html {
                    let _ = fs::write(p, patched);
                }
            }
        }
        Ok(())
    });
}

/// Replace all occurrences of `from` with `to` in every HTML file under `dir`.
pub(crate) fn replace_in_html_files(dir: &Path, from: &str, to: &str) {
    let _ = walk_files(dir, |p| {
        if p.extension().and_then(|e| e.to_str()) == Some("html") {
            if let Ok(html) = fs::read_to_string(p) {
                let patched = html.replace(from, to);
                if patched != html {
                    let _ = fs::write(p, patched);
                }
            }
        }
        Ok(())
    });
}

/// Apply content-hash versioning and SRI to all HTML files under `dist_dir`.
/// `js_filename` is the hashed filename (e.g. `webcore.abc12345.js`); HTML
/// already contains the plain `webcore.js` placeholder which is replaced here.
pub(super) fn patch_asset_hashes(
    dist_dir: &Path,
    js_filename: &str,
    css_hash: &str,
    js_sri: Option<&str>,
    css_sri: Option<&str>,
) {
    // Replace script src placeholder with content-hash filename
    patch_html_files(dist_dir, &format!(r#"src="/assets/{js_filename}""#));
    // CSS keeps query-param versioning (file not renamed)
    replace_in_html_files(
        dist_dir,
        r#"href="/assets/theme.css""#,
        &format!(r#"href="/assets/theme.css?v={css_hash}""#),
    );
    // JS preload hint
    replace_in_html_files(
        dist_dir,
        r#"as="script" href="/assets/webcore.js""#,
        &format!(r#"as="script" href="/assets/{js_filename}""#),
    );

    if let (Some(js_sri), Some(css_sri)) = (js_sri, css_sri) {
        replace_in_html_files(
            dist_dir,
            &format!(r#"src="/assets/{js_filename}""#),
            &format!(r#"src="/assets/{js_filename}" integrity="{js_sri}" crossorigin="anonymous""#),
        );
        replace_in_html_files(
            dist_dir,
            &format!(r#"href="/assets/theme.css?v={css_hash}""#),
            &format!(
                r#"href="/assets/theme.css?v={css_hash}" integrity="{css_sri}" crossorigin="anonymous""#
            ),
        );
        replace_in_html_files(
            dist_dir,
            &format!(r#"as="script" href="/assets/{js_filename}""#),
            &format!(
                r#"as="script" href="/assets/{js_filename}" integrity="{js_sri}" crossorigin="anonymous""#
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn fingerprints_images_preserving_subdirectories() {
        let base = std::env::temp_dir().join("wc_fp_subdir_test");
        let _ = fs::remove_dir_all(&base);
        let public = base.join("public");
        let assets = base.join("assets");
        fs::create_dir_all(public.join("projects")).unwrap();
        fs::create_dir_all(&assets).unwrap();
        fs::write(public.join("og.png"), b"root").unwrap();
        fs::write(public.join("projects").join("thumb.png"), b"sub").unwrap();

        let map = fingerprint_images(&public, &assets).unwrap();

        // Sub-directory image: keyed by its relative path and copied into the
        // same sub-directory (so `/assets/projects/thumb.png` refs get rewritten).
        let sub = map
            .get("projects/thumb.png")
            .expect("subdir image not mapped");
        assert!(
            sub.starts_with("projects/thumb.") && sub.ends_with(".png"),
            "hashed name should keep the subdir: {sub}"
        );
        assert!(assets.join(sub).exists(), "hashed file missing in subdir");

        // Top-level image stays flat.
        let root = map.get("og.png").expect("root image not mapped");
        assert!(!root.contains('/'), "root image should stay flat: {root}");
        assert!(assets.join(root).exists());

        let _ = fs::remove_dir_all(&base);
    }
}
