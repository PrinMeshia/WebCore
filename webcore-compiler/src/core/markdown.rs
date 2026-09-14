//! Build-time Markdown rendering (#73).
//!
//! Converts a Markdown source string to HTML with `pulldown-cmark`
//! (CommonMark + tables/strikethrough). A leading front-matter block delimited
//! by `---` or `+++` is stripped before rendering so it is not shown as text.

use pulldown_cmark::{html, Options, Parser};

/// Render Markdown `source` to an HTML fragment, stripping any leading
/// `---`/`+++` front-matter block.
pub(crate) fn to_html(source: &str) -> String {
    let body = strip_front_matter(source);
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_TASKLISTS);
    let parser = Parser::new_ext(body, options);
    let mut out = String::new();
    html::push_html(&mut out, parser);
    out
}

/// Drop a leading front-matter block (`---\n…\n---` or `+++\n…\n+++`). Returns
/// the input unchanged when there is none.
fn strip_front_matter(source: &str) -> &str {
    let trimmed = source.trim_start_matches(['\u{feff}', '\n', '\r']);
    for delim in ["---", "+++"] {
        if let Some(rest) = trimmed.strip_prefix(delim) {
            // The opening fence must be alone on its line.
            if rest.starts_with('\n') || rest.starts_with('\r') {
                let close = format!("\n{delim}");
                if let Some(idx) = rest.find(&close) {
                    let after = &rest[idx + close.len()..];
                    // Skip to the end of the closing fence's line.
                    return after.find('\n').map_or("", |n| &after[n + 1..]);
                }
            }
        }
    }
    source
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_basic_markdown() {
        let html = to_html("# Title\n\nSome **bold** and a [link](/x).");
        assert!(html.contains("<h1>Title</h1>"), "{html}");
        assert!(html.contains("<strong>bold</strong>"), "{html}");
        assert!(html.contains("<a href=\"/x\">link</a>"), "{html}");
    }

    #[test]
    fn strips_front_matter() {
        let src = "---\ntitle = \"x\"\n---\n\n# Body\n";
        let html = to_html(src);
        assert!(html.contains("<h1>Body</h1>"), "{html}");
        assert!(!html.contains("title = "), "front-matter leaked: {html}");
    }
}
