//! Asset pipeline: hashing, fingerprinting, SRI, copy, and HTML patching.

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

fn rewrite_in_dir(dir: &Path, ext: &str, map: &BTreeMap<String, String>, css_mode: bool) {
    let rewrite_file = |p: &Path| -> std::io::Result<()> {
        if p.extension().and_then(|e| e.to_str()) == Some(ext) {
            if let Ok(content) = fs::read_to_string(p) {
                let mut updated = content.clone();
                for (orig, hashed) in map {
                    if css_mode {
                        // In CSS: url("/assets/orig") and url('/assets/orig')
                        let dq = format!(r#"url("/assets/{orig}")"#);
                        let dq_new = format!(r#"url("/assets/{hashed}")"#);
                        let sq = format!("url('/assets/{orig}')");
                        let sq_new = format!("url('/assets/{hashed}')");
                        updated = updated.replace(&dq, &dq_new);
                        updated = updated.replace(&sq, &sq_new);
                    } else {
                        // In HTML: /assets/orig (bare path)
                        let old_ref = format!("/assets/{orig}");
                        let new_ref = format!("/assets/{hashed}");
                        updated = updated.replace(&old_ref, &new_ref);
                    }
                }
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

pub(crate) fn copy_dir_recursive(src: &Path, dst: &Path, minify: bool) -> Result<(), String> {
    if src.is_dir() {
        let src_owned = src.to_path_buf();
        let dst_owned = dst.to_path_buf();
        fs::create_dir_all(&dst_owned)
            .map_err(|e| format!("Failed to create dir {}: {e}", dst_owned.display()))?;
        walk_files(src, |file_path| {
            let rel = file_path.strip_prefix(&src_owned).unwrap_or(file_path);
            let dst_path = dst_owned.join(rel);
            if let Some(parent) = dst_path.parent() {
                fs::create_dir_all(parent)?;
            }
            if minify && file_path.extension().and_then(|e| e.to_str()) == Some("css") {
                let raw = fs::read_to_string(file_path)?;
                let minified = css_processor::minify_css(&raw).map_err(std::io::Error::other)?;
                fs::write(&dst_path, minified)?;
            } else {
                fs::copy(file_path, &dst_path)?;
            }
            Ok(())
        })
        .map_err(|e| format!("Failed to copy {}: {e}", src.display()))?;
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
