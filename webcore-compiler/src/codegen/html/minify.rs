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

#[cfg(test)]
mod tests {
    use super::minify_html;

    #[test]
    fn preserves_single_space_between_inline_elements() {
        // The trailing space in "Mes " must survive so the title reads
        // "Mes projets", not "Mesprojets".
        let html = "<h1><span>Mes</span> <span>projets</span></h1>";
        assert_eq!(
            minify_html(html),
            "<h1><span>Mes</span> <span>projets</span></h1>"
        );
    }

    #[test]
    fn collapses_newlines_and_indent_to_one_space() {
        let html = "<h1>\n    <span>a</span>\n    <span>b</span>\n</h1>";
        assert_eq!(
            minify_html(html),
            "<h1> <span>a</span> <span>b</span> </h1>"
        );
    }

    #[test]
    fn strips_comments_and_keeps_text_whitespace() {
        // Whitespace inside a text run (not between > and <) is untouched.
        assert_eq!(
            minify_html("<p><!-- x -->Travaillons <span>ensemble</span></p>"),
            "<p>Travaillons <span>ensemble</span></p>"
        );
    }
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
                // Pure whitespace between `>` and `<`: collapse to a SINGLE space
                // rather than removing it. The source (dev) HTML separates
                // elements with newlines/indentation, which the browser renders
                // as one space in inline flow (e.g. `<span>Mes</span> <span>…`).
                // Removing it entirely would glue inline text together
                // ("Mesprojets") and make prod render differently from dev.
                // Whitespace-only text nodes are ignored in flex/block contexts,
                // so this is safe there and faithful in inline contexts.
                if start < i {
                    result.push(' ');
                }
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
