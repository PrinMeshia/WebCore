//! Asset pipeline: hashing, fingerprinting, SRI, copy, and HTML patching.

use crate::core::css_processor;
use std::collections::HashMap;
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
    let sri = if compute_sri { Some(sri_hash(data)) } else { None };
    (fnv, sri)
}

/// Image file extensions that are subject to content-hash fingerprinting.
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "svg", "ico", "avif"];

/// For every image file directly inside `public_dir`, compute a content hash,
/// copy the file to `assets_dir/<stem>.<hash>.<ext>`, and return a mapping
/// `"original.png"` → `"original.<hash>.png"`.
pub(crate) fn fingerprint_images(
    public_dir: &Path,
    assets_dir: &Path,
) -> Result<HashMap<String, String>, String> {
    let mut map: HashMap<String, String> = HashMap::new();

    fn walk(
        dir: &Path,
        assets_dir: &Path,
        map: &mut HashMap<String, String>,
    ) -> Result<(), String> {
        let entries =
            fs::read_dir(dir).map_err(|e| format!("Failed to read {}: {e}", dir.display()))?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, assets_dir, map)?;
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
            let file_name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            let bytes =
                fs::read(&path).map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
            let hash = fnv1a_hash(&bytes);
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(&file_name);
            let hashed_name = format!("{stem}.{hash}.{ext}");
            let dst = assets_dir.join(&hashed_name);
            fs::copy(&path, &dst).map_err(|e| {
                format!("Failed to copy {} → {}: {e}", path.display(), dst.display())
            })?;
            map.insert(file_name, hashed_name);
        }
        Ok(())
    }

    walk(public_dir, assets_dir, &mut map)?;
    Ok(map)
}

/// Post-process all `.html` files under `dist_dir` and all `.css` files under
/// `dist_dir/assets/`, replacing `/assets/<original>` references with
/// `/assets/<hashed>`.
pub(crate) fn rewrite_asset_refs(dist_dir: &Path, map: &HashMap<String, String>) {
    // Rewrite HTML files (any depth)
    rewrite_in_dir(dist_dir, "html", map, false);
    // Rewrite CSS files in dist/assets/
    let assets_dir = dist_dir.join("assets");
    if assets_dir.is_dir() {
        rewrite_in_dir(&assets_dir, "css", map, true);
    }
}

fn rewrite_in_dir(dir: &Path, ext: &str, map: &HashMap<String, String>, css_mode: bool) {
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
pub(super) fn patch_asset_hashes(
    dist_dir: &Path,
    js_hash: &str,
    css_hash: &str,
    js_sri: Option<&str>,
    css_sri: Option<&str>,
) {
    patch_html_files(
        dist_dir,
        &format!(r#"src="/assets/webcore.js?v={js_hash}""#),
    );
    replace_in_html_files(
        dist_dir,
        r#"href="/assets/theme.css""#,
        &format!(r#"href="/assets/theme.css?v={css_hash}""#),
    );
    replace_in_html_files(
        dist_dir,
        r#"as="script" href="/assets/webcore.js""#,
        &format!(r#"as="script" href="/assets/webcore.js?v={js_hash}""#),
    );

    if let (Some(js_sri), Some(css_sri)) = (js_sri, css_sri) {
        replace_in_html_files(
            dist_dir,
            &format!(r#"src="/assets/webcore.js?v={js_hash}""#),
            &format!(
                r#"src="/assets/webcore.js?v={js_hash}" integrity="{js_sri}" crossorigin="anonymous""#
            ),
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
            &format!(r#"as="script" href="/assets/webcore.js?v={js_hash}""#),
            &format!(
                r#"as="script" href="/assets/webcore.js?v={js_hash}" integrity="{js_sri}" crossorigin="anonymous""#
            ),
        );
    }
}
