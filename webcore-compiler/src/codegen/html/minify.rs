// ── HTML minifier (prod mode) ────────────────────────────────────────────────

/// Strip HTML comments and collapse inter-tag whitespace (prod mode).
pub(crate) fn minify_html(html: &str) -> String {
    let no_comments = strip_html_comments(html);
    collapse_whitespace_between_tags(&no_comments)
}

fn strip_html_comments(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut i = 0;
    let bytes = html.as_bytes();
    while i < bytes.len() {
        if bytes[i..].starts_with(b"<!--") {
            if let Some(end) = html[i..].find("-->") {
                i += end + 3;
            } else {
                result.push_str(&html[i..]);
                break;
            }
        } else {
            // Advance by a full UTF-8 character, not a byte.
            let Some(ch) = html[i..].chars().next() else {
                break;
            };
            result.push(ch);
            i += ch.len_utf8();
        }
    }
    result
}

fn collapse_whitespace_between_tags(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let bytes = html.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'>' {
            result.push('>');
            i += 1;
            // Skip whitespace-only runs between > and <
            let start = i;
            while i < bytes.len()
                && (bytes[i] == b' ' || bytes[i] == b'\n' || bytes[i] == b'\t' || bytes[i] == b'\r')
            {
                i += 1;
            }
            if i < bytes.len() && bytes[i] == b'<' {
                // pure whitespace between > and < — discard
            } else {
                // contains non-whitespace — keep original
                result.push_str(&html[start..i]);
            }
        } else {
            // Advance by a full UTF-8 character, not a byte.
            let Some(ch) = html[i..].chars().next() else {
                break;
            };
            result.push(ch);
            i += ch.len_utf8();
        }
    }
    result
}
