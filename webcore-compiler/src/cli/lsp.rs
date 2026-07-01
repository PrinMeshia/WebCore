//! `webc lsp` — Language Server Protocol server over stdin/stdout.
//!
//! Implements a subset of LSP 3.17 using the JSON-RPC framing protocol.
//! No additional dependencies are required beyond `serde_json` (already in Cargo.toml).
//!
//! ## Supported capabilities
//! - `textDocument/hover`      — type, default value, or expression for the symbol under the cursor
//! - `textDocument/completion` — state vars, computed vars, props, component names
//! - `textDocument/definition` — jump to a component or state variable declaration

use crate::core::ast::{Component, WebCoreDocument};
use crate::parser::parse_webc;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{self, BufRead, Write};

// ── JSON-RPC framing ──────────────────────────────────────────────────────────

fn read_message(stdin: &mut impl BufRead) -> Option<Value> {
    let mut content_length: usize = 0;
    loop {
        let mut line = String::new();
        if stdin.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            break;
        }
        if let Some(val) = line.strip_prefix("Content-Length: ") {
            content_length = val.trim().parse().unwrap_or(0);
        }
    }
    if content_length == 0 {
        return None;
    }
    let mut buf = vec![0u8; content_length];
    stdin.read_exact(&mut buf).ok()?;
    serde_json::from_slice(&buf).ok()
}

fn send_message(stdout: &mut impl Write, msg: &Value) {
    let bytes = serde_json::to_vec(msg).expect("json serialization is infallible");
    let _ = write!(stdout, "Content-Length: {}\r\n\r\n", bytes.len());
    let _ = stdout.write_all(&bytes);
    let _ = stdout.flush();
}

fn send_response(stdout: &mut impl Write, id: &Value, result: Value) {
    send_message(stdout, &json!({"jsonrpc":"2.0","id":id,"result":result}));
}

fn send_error(stdout: &mut impl Write, id: &Value, code: i32, message: &str) {
    send_message(
        stdout,
        &json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}}),
    );
}

fn send_notification(stdout: &mut impl Write, method: &str, params: Value) {
    send_message(
        stdout,
        &json!({"jsonrpc":"2.0","method":method,"params":params}),
    );
}

// ── Diagnostics ───────────────────────────────────────────────────────────────

/// Convert a byte offset in `source` to a (line, character) LSP position (0-indexed).
fn byte_offset_to_pos(source: &str, offset: usize) -> (u64, u64) {
    let capped = offset.min(source.len());
    let before = &source[..capped];
    let line = before.bytes().filter(|&b| b == b'\n').count();
    let col = before.rfind('\n').map(|i| capped - i - 1).unwrap_or(capped);
    (line as u64, col as u64)
}

/// Parse `source` and push a `textDocument/publishDiagnostics` notification.
/// On success the diagnostics array is empty (clears any previous errors).
/// On error one diagnostic is emitted covering the reported span.
fn publish_diagnostics(stdout: &mut impl Write, uri: &str, source: &str) {
    let diagnostics = match parse_webc(source) {
        Ok(_) => vec![],
        Err(err) => {
            let (start_line, start_char, end_line, end_char) = match &err.span {
                Some(span) => {
                    // Use byte offsets for both positions — consistent 0-indexed LSP coords.
                    let (sl, sc) = byte_offset_to_pos(source, span.start);
                    let (el, ec) = byte_offset_to_pos(source, span.end);
                    // If end collapsed onto start, extend one character so the range is visible.
                    if (el, ec) == (sl, sc) {
                        (sl, sc, sl, sc + 1)
                    } else {
                        (sl, sc, el, ec)
                    }
                }
                // No span: mark the first character of the file.
                None => (0, 0, 0, 1),
            };
            vec![json!({
                "range": {
                    "start": {"line": start_line, "character": start_char},
                    "end":   {"line": end_line,   "character": end_char}
                },
                "severity": 1,   // Error
                "source":   "webc",
                "message":  err.concise_message()
            })]
        }
    };
    send_notification(
        stdout,
        "textDocument/publishDiagnostics",
        json!({"uri": uri, "diagnostics": diagnostics}),
    );
}

// ── Document store ────────────────────────────────────────────────────────────

struct LspState {
    /// uri → raw source text
    docs: HashMap<String, String>,
}

impl LspState {
    fn new() -> Self {
        Self {
            docs: HashMap::new(),
        }
    }

    fn open(&mut self, uri: String, text: String) {
        self.docs.insert(uri, text);
    }

    fn change(&mut self, uri: &str, text: String) {
        if let Some(entry) = self.docs.get_mut(uri) {
            *entry = text;
        }
    }

    fn close(&mut self, uri: &str) {
        self.docs.remove(uri);
    }

    fn parse(&self, uri: &str) -> Option<WebCoreDocument> {
        let src = self.docs.get(uri)?;
        parse_webc(src).ok()
    }

    fn source(&self, uri: &str) -> Option<&str> {
        self.docs.get(uri).map(String::as_str)
    }
}

// ── Symbol lookup helpers ─────────────────────────────────────────────────────

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Extract the identifier word at `(line, col)` from raw source (0-indexed, LSP convention).
fn word_at(source: &str, line: usize, col: usize) -> Option<String> {
    let src_line = source.lines().nth(line)?;
    let chars: Vec<char> = src_line.chars().collect();
    let col = col.min(chars.len().saturating_sub(1));

    if col >= chars.len() || !is_word_char(chars[col]) {
        return None;
    }
    let start = (0..=col)
        .rev()
        .take_while(|&i| is_word_char(chars[i]))
        .last()?;
    let end = (col..chars.len())
        .take_while(|&i| is_word_char(chars[i]))
        .last()?
        + 1;
    Some(chars[start..end].iter().collect())
}

/// Markdown hover content for a symbol found in the document.
fn hover_for_symbol(doc: &WebCoreDocument, word: &str) -> Option<String> {
    // State variables
    for comp in doc.components.values() {
        for sv in &comp.state {
            if sv.name == word {
                let def = sv
                    .default_value
                    .as_deref()
                    .map(|v| format!(", default: `{v}`"))
                    .unwrap_or_default();
                return Some(format!(
                    "**state** `{}`  \nType: `{}`{}",
                    sv.name, sv.type_, def
                ));
            }
        }
        // Computed vars
        for cv in &comp.computed {
            if cv.name == word {
                return Some(format!(
                    "**computed** `{}`  \nExpression: `{}`",
                    cv.name, cv.expr
                ));
            }
        }
        // Props
        for p in &comp.props {
            if p.name == word {
                let ty = p
                    .type_
                    .as_deref()
                    .map(|t| format!(": `{t}`"))
                    .unwrap_or_default();
                let def = p
                    .default_value
                    .as_deref()
                    .map(|v| format!(", default: `{v}`"))
                    .unwrap_or_default();
                return Some(format!("**prop** `{}`{}{}", p.name, ty, def));
            }
        }
    }
    // Store variables
    for sv in &doc.store {
        if sv.name == word {
            let def = sv
                .default_value
                .as_deref()
                .map(|v| format!(", default: `{v}`"))
                .unwrap_or_default();
            return Some(format!(
                "**store** `{}`  \nType: `{}`{}",
                sv.name, sv.type_, def
            ));
        }
    }
    // Component declarations
    if let Some(comp) = doc.components.get(word) {
        return Some(format_component_hover(comp));
    }
    None
}

fn format_component_hover(comp: &Component) -> String {
    let mut lines = vec![format!("**component** `{}`", comp.name)];
    if !comp.props.is_empty() {
        lines.push(String::new());
        lines.push("**Props:**".to_string());
        for p in &comp.props {
            let ty = p.type_.as_deref().unwrap_or("Any");
            let def = p
                .default_value
                .as_deref()
                .map(|v| format!(" = {v}"))
                .unwrap_or_default();
            lines.push(format!("- `{}`: {}{}", p.name, ty, def));
        }
    }
    if !comp.state.is_empty() {
        lines.push(String::new());
        lines.push("**State:**".to_string());
        for sv in &comp.state {
            let def = sv
                .default_value
                .as_deref()
                .map(|v| format!(" = {v}"))
                .unwrap_or_default();
            lines.push(format!("- `{}`: {}{}", sv.name, sv.type_, def));
        }
    }
    lines.join("  \n")
}

/// Build LSP completion items from the document.
fn completion_items(doc: &WebCoreDocument) -> Vec<Value> {
    let mut items: Vec<Value> = Vec::new();

    for comp in doc.components.values() {
        // State vars (kind 6 = Variable)
        for sv in &comp.state {
            let detail = format!("{}: {}", sv.name, sv.type_);
            items.push(json!({
                "label": sv.name,
                "kind": 6,
                "detail": detail,
                "documentation": {
                    "kind": "markdown",
                    "value": hover_for_symbol(doc, &sv.name).unwrap_or_default()
                }
            }));
        }
        // Computed vars (kind 6 = Variable)
        for cv in &comp.computed {
            items.push(json!({
                "label": cv.name,
                "kind": 6,
                "detail": format!("computed = {}", cv.expr),
                "documentation": {
                    "kind": "markdown",
                    "value": format!("**computed** `{}`  \nExpression: `{}`", cv.name, cv.expr)
                }
            }));
        }
        // Props (kind 5 = Field)
        for p in &comp.props {
            let ty = p.type_.as_deref().unwrap_or("Any");
            items.push(json!({
                "label": p.name,
                "kind": 5,
                "detail": format!("prop: {ty}"),
                "documentation": {
                    "kind": "markdown",
                    "value": hover_for_symbol(doc, &p.name).unwrap_or_default()
                }
            }));
        }
        // Component name (kind 7 = Class)
        items.push(json!({
            "label": comp.name,
            "kind": 7,
            "detail": "component",
            "documentation": {
                "kind": "markdown",
                "value": format_component_hover(comp)
            }
        }));
    }

    // Store vars (kind 6 = Variable)
    for sv in &doc.store {
        items.push(json!({
            "label": sv.name,
            "kind": 6,
            "detail": format!("store: {}", sv.type_),
            "documentation": {
                "kind": "markdown",
                "value": hover_for_symbol(doc, &sv.name).unwrap_or_default()
            }
        }));
    }

    // Built-in identifiers
    for builtin in ["loading", "error"] {
        items.push(json!({
            "label": builtin,
            "kind": 6,
            "detail": "http state (auto-injected)",
            "documentation": {
                "kind": "markdown",
                "value": format!("`{builtin}` — injected automatically when an `http {{}}` block is present")
            }
        }));
    }

    items
}

/// Collect all source ranges where `word` appears as a whole-word identifier.
/// Returns a list of LSP `TextEdit` objects (range + newText) for a rename.
fn rename_locations(source: &str, uri: &str, word: &str, new_name: &str) -> Value {
    let word_char_len = word.chars().count();
    let mut edits: Vec<Value> = Vec::new();
    for (line_idx, line_text) in source.lines().enumerate() {
        let mut col = 0usize;
        while col < line_text.len() {
            if let Some(pos) = line_text[col..].find(word) {
                let abs = col + pos;
                let before_ok =
                    abs == 0 || !line_text[..abs].chars().last().is_some_and(is_word_char);
                let after_abs = abs + word.len();
                let after_ok = after_abs >= line_text.len()
                    || !line_text[after_abs..]
                        .chars()
                        .next()
                        .is_some_and(is_word_char);
                if before_ok && after_ok {
                    let char_start = line_text[..abs].chars().count();
                    edits.push(json!({
                        "range": {
                            "start": {"line": line_idx, "character": char_start},
                            "end":   {"line": line_idx, "character": char_start + word_char_len}
                        },
                        "newText": new_name
                    }));
                }
                col = abs + word.len().max(1);
            } else {
                break;
            }
        }
    }
    json!({ "changes": { uri: edits } })
}

/// Find the definition location (0-indexed line/col) of a symbol in the document.
fn definition_location(uri: &str, doc: &WebCoreDocument, word: &str) -> Option<Value> {
    // Component declaration: use span.line (1-indexed → 0-indexed)
    if let Some(comp) = doc.components.get(word) {
        let line = comp.span.line.saturating_sub(1);
        let col = comp.span.col.saturating_sub(1);
        return Some(json!({
            "uri": uri,
            "range": {
                "start": {"line": line, "character": col},
                "end":   {"line": line, "character": col + comp.name.len() as u32}
            }
        }));
    }
    // State var: use span from the first component that declares it
    for comp in doc.components.values() {
        for sv in &comp.state {
            if sv.name == word {
                let line = sv.span.line.saturating_sub(1);
                let col = sv.span.col.saturating_sub(1);
                return Some(json!({
                    "uri": uri,
                    "range": {
                        "start": {"line": line, "character": col},
                        "end":   {"line": line, "character": col + sv.name.len() as u32}
                    }
                }));
            }
        }
        for p in &comp.props {
            if p.name == word {
                let line = p.span.line.saturating_sub(1);
                let col = p.span.col.saturating_sub(1);
                return Some(json!({
                    "uri": uri,
                    "range": {
                        "start": {"line": line, "character": col},
                        "end":   {"line": line, "character": col + p.name.len() as u32}
                    }
                }));
            }
        }
    }
    None
}

// ── Semantic tokens ───────────────────────────────────────────────────────────

const SEM_KEYWORD: u32 = 0; // "keyword"
const SEM_TYPE: u32 = 1; // "type"
const SEM_STRING: u32 = 2; // "string"
const SEM_COMMENT: u32 = 3; // "comment"
const SEM_VARIABLE: u32 = 4; // "variable"
const SEM_CLASS: u32 = 5; // "class"
const SEM_NUMBER: u32 = 6; // "number"

const MOD_READONLY: u32 = 1; // computed vars

const BLOCK_KEYWORDS: &[&str] = &[
    "component",
    "page",
    "layout",
    "app",
    "store",
    "state",
    "computed",
    "props",
    "view",
    "style",
    "http",
    "head",
    "routes",
    "import",
    "from",
    "slot",
    "link",
    "each",
];
const TYPE_NAMES: &[&str] = &["Number", "String", "Boolean", "List"];
#[allow(dead_code)]
const AT_DIRECTIVES: &[&str] = &[
    "if", "else", "for", "switch", "case", "default", "error", "loading", "catch", "defer",
];
const ATTR_PREFIXES: &[&str] = &["on", "bind", "class", "style", "ref", "validate", "webc"];

/// Scan `source` and produce the LSP delta-encoded semantic token array.
fn scan_semantic_tokens(source: &str, doc: Option<&WebCoreDocument>) -> Vec<u32> {
    use std::collections::HashSet;

    // Build name sets from the AST (if available).
    let mut state_var_names: HashSet<String> = HashSet::new();
    let mut computed_names: HashSet<String> = HashSet::new();
    let mut comp_names: HashSet<String> = HashSet::new();

    if let Some(d) = doc {
        for comp in d.components.values() {
            comp_names.insert(comp.name.clone());
            for sv in &comp.state {
                state_var_names.insert(sv.name.clone());
            }
            for p in &comp.props {
                state_var_names.insert(p.name.clone());
            }
            for cv in &comp.computed {
                computed_names.insert(cv.name.clone());
            }
        }
        for sv in &d.store {
            state_var_names.insert(sv.name.clone());
        }
    }

    struct Token {
        line: u32,
        ch: u32,
        len: u32,
        ty: u32,
        mods: u32,
    }
    let mut tokens: Vec<Token> = Vec::new();

    for (line_idx, line_text) in source.lines().enumerate() {
        let line = line_idx as u32;
        let chars: Vec<char> = line_text.chars().collect();
        let len = chars.len();
        let mut i = 0usize;

        while i < len {
            // --- line comment ---
            if i + 1 < len && chars[i] == '/' && chars[i + 1] == '/' {
                tokens.push(Token {
                    line,
                    ch: i as u32,
                    len: (len - i) as u32,
                    ty: SEM_COMMENT,
                    mods: 0,
                });
                break; // rest of line is comment
            }

            // --- string literal ---
            if chars[i] == '"' {
                let str_start = i;
                i += 1;
                // Track segments: the string may contain $variable references inside it.
                // We emit variable tokens for $word or $word.word found inside the string.
                let mut seg_start = str_start; // start of current string segment
                while i < len {
                    if chars[i] == '\\' && i + 1 < len {
                        i += 2; // skip escaped char
                    } else if chars[i] == '$' && i + 1 < len && is_word_char(chars[i + 1]) {
                        // Emit the string segment before the $
                        let seg_len = i - seg_start;
                        if seg_len > 0 {
                            tokens.push(Token {
                                line,
                                ch: seg_start as u32,
                                len: seg_len as u32,
                                ty: SEM_STRING,
                                mods: 0,
                            });
                        }
                        // Scan the $variable
                        let var_start = i;
                        i += 1; // skip '$'
                        while i < len && is_word_char(chars[i]) {
                            i += 1;
                        }
                        // optional .word suffix
                        if i < len && chars[i] == '.' {
                            let dot_pos = i;
                            i += 1;
                            if i < len && is_word_char(chars[i]) {
                                while i < len && is_word_char(chars[i]) {
                                    i += 1;
                                }
                            } else {
                                i = dot_pos;
                            }
                        }
                        tokens.push(Token {
                            line,
                            ch: var_start as u32,
                            len: (i - var_start) as u32,
                            ty: SEM_VARIABLE,
                            mods: 0,
                        });
                        seg_start = i;
                    } else if chars[i] == '"' {
                        i += 1;
                        break;
                    } else {
                        i += 1;
                    }
                }
                // Emit the remaining string segment (including closing quote)
                let seg_len = i - seg_start;
                if seg_len > 0 {
                    tokens.push(Token {
                        line,
                        ch: seg_start as u32,
                        len: seg_len as u32,
                        ty: SEM_STRING,
                        mods: 0,
                    });
                }
                continue;
            }

            // --- $variable (possibly $word.word) ---
            if chars[i] == '$' {
                let start = i;
                i += 1;
                while i < len && is_word_char(chars[i]) {
                    i += 1;
                }
                // optional .word suffix
                if i < len && chars[i] == '.' {
                    let dot_pos = i;
                    i += 1;
                    if i < len && is_word_char(chars[i]) {
                        while i < len && is_word_char(chars[i]) {
                            i += 1;
                        }
                    } else {
                        // No word chars after dot — backtrack to dot.
                        i = dot_pos;
                    }
                }
                tokens.push(Token {
                    line,
                    ch: start as u32,
                    len: (i - start) as u32,
                    ty: SEM_VARIABLE,
                    mods: 0,
                });
                continue;
            }

            // --- @directive ---
            if chars[i] == '@' {
                let start = i;
                i += 1;
                while i < len && is_word_char(chars[i]) {
                    i += 1;
                }
                // Any @word is a keyword.
                let word_len = i - start;
                if word_len > 1 {
                    tokens.push(Token {
                        line,
                        ch: start as u32,
                        len: word_len as u32,
                        ty: SEM_KEYWORD,
                        mods: 0,
                    });
                }
                continue;
            }

            // --- digit (number literal) ---
            if chars[i].is_ascii_digit() {
                let start = i;
                while i < len && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                tokens.push(Token {
                    line,
                    ch: start as u32,
                    len: (i - start) as u32,
                    ty: SEM_NUMBER,
                    mods: 0,
                });
                continue;
            }

            // --- word (keyword, type, attr-prefix, identifier) ---
            if is_word_char(chars[i]) {
                let start = i;
                while i < len && is_word_char(chars[i]) {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();

                // Check for attr-prefix colon: e.g. on:click, bind:value
                if i < len && chars[i] == ':' {
                    let prefix: &str = &word;
                    if ATTR_PREFIXES.contains(&prefix) {
                        let tok_start = start;
                        i += 1; // skip ':'
                        while i < len && is_word_char(chars[i]) {
                            i += 1;
                        }
                        tokens.push(Token {
                            line,
                            ch: tok_start as u32,
                            len: (i - tok_start) as u32,
                            ty: SEM_KEYWORD,
                            mods: 0,
                        });
                        continue;
                    }
                }

                // Block keywords
                if BLOCK_KEYWORDS.contains(&word.as_str()) {
                    tokens.push(Token {
                        line,
                        ch: start as u32,
                        len: word.chars().count() as u32,
                        ty: SEM_KEYWORD,
                        mods: 0,
                    });
                    continue;
                }

                // Type names
                if TYPE_NAMES.contains(&word.as_str()) {
                    tokens.push(Token {
                        line,
                        ch: start as u32,
                        len: word.chars().count() as u32,
                        ty: SEM_TYPE,
                        mods: 0,
                    });
                    continue;
                }

                // Component names → class
                if comp_names.contains(&word) {
                    tokens.push(Token {
                        line,
                        ch: start as u32,
                        len: word.chars().count() as u32,
                        ty: SEM_CLASS,
                        mods: 0,
                    });
                    continue;
                }

                // Computed vars → variable with readonly modifier
                if computed_names.contains(&word) {
                    tokens.push(Token {
                        line,
                        ch: start as u32,
                        len: word.chars().count() as u32,
                        ty: SEM_VARIABLE,
                        mods: MOD_READONLY,
                    });
                    continue;
                }

                // State vars / props → variable
                if state_var_names.contains(&word) {
                    tokens.push(Token {
                        line,
                        ch: start as u32,
                        len: word.chars().count() as u32,
                        ty: SEM_VARIABLE,
                        mods: 0,
                    });
                    continue;
                }

                // Everything else: no token
                continue;
            }

            // Skip any other character.
            i += 1;
        }
    }

    // Delta-encode tokens (already in source order).
    let mut data: Vec<u32> = Vec::with_capacity(tokens.len() * 5);
    let mut prev_line = 0u32;
    let mut prev_ch = 0u32;
    for tok in &tokens {
        let delta_line = tok.line - prev_line;
        let delta_ch = if delta_line == 0 {
            tok.ch - prev_ch
        } else {
            tok.ch
        };
        data.push(delta_line);
        data.push(delta_ch);
        data.push(tok.len);
        data.push(tok.ty);
        data.push(tok.mods);
        prev_line = tok.line;
        prev_ch = tok.ch;
    }
    data
}

// ── Code actions ──────────────────────────────────────────────────────────────

/// Return true if source already contains `import {name} from`.
fn already_imported(source: &str, name: &str) -> bool {
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("import") && trimmed.contains(name) && trimmed.contains("from") {
            return true;
        }
    }
    false
}

/// Return true if word is declared in any component's state/computed/props or the global store.
fn is_known_var(doc: &WebCoreDocument, word: &str) -> bool {
    for comp in doc.components.values() {
        for sv in &comp.state {
            if sv.name == word {
                return true;
            }
        }
        for cv in &comp.computed {
            if cv.name == word {
                return true;
            }
        }
        for p in &comp.props {
            if p.name == word {
                return true;
            }
        }
    }
    for sv in &doc.store {
        if sv.name == word {
            return true;
        }
    }
    false
}

/// Return true if word is a DSL keyword or built-in identifier.
fn is_dsl_keyword_or_builtin(word: &str) -> bool {
    if BLOCK_KEYWORDS.contains(&word) {
        return true;
    }
    if TYPE_NAMES.contains(&word) {
        return true;
    }
    matches!(word, "loading" | "error" | "true" | "false" | "null")
}

/// Find where to insert inside the `state {}` block nearest to `cursor_line`.
/// Returns (insert_before_line, indentation_str).
fn find_state_insert_line(source: &str, cursor_line: usize) -> Option<(usize, &'static str)> {
    let lines: Vec<&str> = source.lines().collect();

    // Walk backward to find the enclosing component/page/layout.
    let mut comp_line: Option<usize> = None;
    for i in (0..=cursor_line.min(lines.len().saturating_sub(1))).rev() {
        let trimmed = lines[i].trim();
        if trimmed.starts_with("component ")
            || trimmed.starts_with("page ")
            || trimmed.starts_with("layout ")
        {
            comp_line = Some(i);
            break;
        }
    }

    let start = comp_line?;

    // Scan forward for `state {`.
    let mut state_line: Option<usize> = None;
    for (i, line_str) in lines.iter().enumerate().skip(start) {
        let trimmed = line_str.trim();
        if trimmed == "state {"
            || trimmed.starts_with("state {")
            || trimmed == "state{"
            || trimmed.starts_with("state{")
        {
            state_line = Some(i);
            break;
        }
        if i > start
            && (trimmed.starts_with("component ")
                || trimmed.starts_with("page ")
                || trimmed.starts_with("layout "))
        {
            break;
        }
    }

    let state_start = state_line?;

    // Find the closing `}` of the state block.
    for (i, line_str) in lines.iter().enumerate().skip(state_start + 1) {
        if line_str.trim() == "}" {
            return Some((i, "    "));
        }
    }

    // Single-line state block.
    Some((state_start, "        "))
}

/// Build the list of code actions for the given cursor position.
fn code_actions(
    source: &str,
    uri: &str,
    doc: Option<&WebCoreDocument>,
    line: usize,
    col: usize,
) -> Vec<Value> {
    let mut actions: Vec<Value> = Vec::new();

    let word = match word_at(source, line, col) {
        Some(w) => w,
        None => return actions,
    };

    let first_char = word.chars().next().unwrap_or('\0');

    if first_char.is_uppercase() {
        // "Import component 'X'" — uppercase identifier not already known or imported
        let is_known_component = doc.is_some_and(|d| d.components.contains_key(word.as_str()));
        if !is_known_component && !already_imported(source, &word) {
            let new_text = format!("import {} from \"./{}.webc\"\n", word, word);
            actions.push(json!({
                "title": format!("Import component '{}'", word),
                "kind": "quickfix",
                "isPreferred": true,
                "edit": {
                    "changes": {
                        uri: [{
                            "range": {
                                "start": {"line": 0, "character": 0},
                                "end":   {"line": 0, "character": 0}
                            },
                            "newText": new_text
                        }]
                    }
                }
            }));
        }
    } else if first_char.is_lowercase() || first_char == '_' {
        // "Add 'x' to state" — unknown lowercase identifier
        let is_known = doc.is_some_and(|d| is_known_var(d, &word));
        if !is_known && !is_dsl_keyword_or_builtin(&word) {
            if let Some((insert_line, indent)) = find_state_insert_line(source, line) {
                let new_text = format!("{}{}: String = \"\"\n", indent, word);
                actions.push(json!({
                    "title": format!("Add '{}' to state", word),
                    "kind": "quickfix",
                    "edit": {
                        "changes": {
                            uri: [{
                                "range": {
                                    "start": {"line": insert_line, "character": 0},
                                    "end":   {"line": insert_line, "character": 0}
                                },
                                "newText": new_text
                            }]
                        }
                    }
                }));
            } else {
                // No existing state block — insert a new one after the component's opening brace.
                let lines: Vec<&str> = source.lines().collect();
                let mut comp_line: Option<usize> = None;
                for i in (0..=line.min(lines.len().saturating_sub(1))).rev() {
                    let trimmed = lines[i].trim();
                    if trimmed.starts_with("component ")
                        || trimmed.starts_with("page ")
                        || trimmed.starts_with("layout ")
                    {
                        comp_line = Some(i);
                        break;
                    }
                }
                if let Some(cl) = comp_line {
                    let mut open_brace_line = cl;
                    for (i, line_str) in lines.iter().enumerate().skip(cl) {
                        if line_str.contains('{') {
                            open_brace_line = i;
                            break;
                        }
                    }
                    let insert_line = open_brace_line + 1;
                    let new_text =
                        format!("\n    state {{\n        {}: String = \"\"\n    }}\n", word);
                    actions.push(json!({
                        "title": format!("Add '{}' to state", word),
                        "kind": "quickfix",
                        "edit": {
                            "changes": {
                                uri: [{
                                    "range": {
                                        "start": {"line": insert_line, "character": 0},
                                        "end":   {"line": insert_line, "character": 0}
                                    },
                                    "newText": new_text
                                }]
                            }
                        }
                    }));
                }
            }
        }
    }

    actions
}

// ── Test-accessible re-exports ────────────────────────────────────────────────

/// Exposed for unit tests in `tests/mod.rs`.
#[cfg(test)]
pub(crate) fn word_at_test(source: &str, line: usize, col: usize) -> Option<String> {
    word_at(source, line, col)
}

/// Exposed for unit tests in `tests/mod.rs`.
#[cfg(test)]
pub(crate) fn hover_for_symbol_test(doc: &WebCoreDocument, word: &str) -> Option<String> {
    hover_for_symbol(doc, word)
}

/// Exposed for tests: parse `source`, return the first LSP diagnostic (if any).
#[cfg(test)]
pub(crate) fn first_diagnostic(source: &str) -> Option<Value> {
    let mut buf = Vec::new();
    publish_diagnostics(&mut buf, "file:///test.webc", source);
    let msg: Value = serde_json::from_slice(
        buf.splitn(2, |&b| b == b'\n')
            .nth(1)
            .and_then(|s| s.strip_prefix(b"\r\n"))
            .unwrap_or(&buf),
    )
    .ok()?;
    msg["params"]["diagnostics"].as_array()?.first().cloned()
}

/// Decode the flat delta-encoded u32 array into absolute (line, char, len, type, mods) tuples.
#[cfg(test)]
pub(crate) fn decode_semantic_tokens(data: &[u32]) -> Vec<(u32, u32, u32, u32, u32)> {
    let mut out = Vec::new();
    let mut line = 0u32;
    let mut ch = 0u32;
    for chunk in data.chunks_exact(5) {
        line += chunk[0];
        ch = if chunk[0] == 0 {
            ch + chunk[1]
        } else {
            chunk[1]
        };
        out.push((line, ch, chunk[2], chunk[3], chunk[4]));
    }
    out
}

#[cfg(test)]
pub(crate) fn semantic_tokens_for_source(source: &str) -> Vec<u32> {
    let doc = parse_webc(source).ok();
    scan_semantic_tokens(source, doc.as_ref())
}

/// Exposed for tests: build code actions for the given source position.
#[cfg(test)]
pub(crate) fn code_actions_for_source(source: &str, line: usize, col: usize) -> Vec<Value> {
    let doc = parse_webc(source).ok();
    code_actions(source, "file:///test.webc", doc.as_ref(), line, col)
}

// ── Main server loop ──────────────────────────────────────────────────────────

pub(crate) fn run_lsp() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdin = io::BufReader::new(stdin.lock());
    let mut stdout = stdout.lock();

    let mut state = LspState::new();
    let mut initialized = false;
    let mut shutdown_requested = false;

    while let Some(msg) = read_message(&mut stdin) {
        let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
        let id = msg.get("id").cloned().unwrap_or(Value::Null);
        let params = msg.get("params").cloned().unwrap_or(Value::Null);

        match method {
            "initialize" => {
                initialized = true;
                send_response(
                    &mut stdout,
                    &id,
                    json!({
                        "capabilities": {
                            "textDocumentSync": {
                                "openClose": true,
                                "change": 1  // full sync
                            },
                            "hoverProvider": true,
                            "completionProvider": {
                                "triggerCharacters": ["{", " ", "."]
                            },
                            "definitionProvider": true,
                            "renameProvider": true,
                            "codeActionProvider": true,
                            "semanticTokensProvider": {
                                "legend": {
                                    "tokenTypes": ["keyword", "type", "string", "comment", "variable", "class", "number"],
                                    "tokenModifiers": ["readonly"]
                                },
                                "full": true
                            }
                        },
                        "serverInfo": {
                            "name": "webc-lsp",
                            "version": env!("CARGO_PKG_VERSION")
                        }
                    }),
                );
            }
            "initialized" => {}
            "shutdown" => {
                shutdown_requested = true;
                send_response(&mut stdout, &id, Value::Null);
            }
            "exit" => {
                std::process::exit(if shutdown_requested { 0 } else { 1 });
            }
            "$/cancelRequest" => {}

            "textDocument/didOpen" => {
                if let (Some(uri), Some(text)) = (
                    params["textDocument"]["uri"].as_str(),
                    params["textDocument"]["text"].as_str(),
                ) {
                    state.open(uri.to_string(), text.to_string());
                    publish_diagnostics(&mut stdout, uri, text);
                }
            }
            "textDocument/didChange" => {
                if let Some(uri) = params["textDocument"]["uri"].as_str() {
                    if let Some(changes) = params["contentChanges"].as_array() {
                        if let Some(last) = changes.last() {
                            if let Some(text) = last["text"].as_str() {
                                state.change(uri, text.to_string());
                                publish_diagnostics(&mut stdout, uri, text);
                            }
                        }
                    }
                }
            }
            "textDocument/didClose" => {
                if let Some(uri) = params["textDocument"]["uri"].as_str() {
                    // Clear diagnostics when the file is closed.
                    send_notification(
                        &mut stdout,
                        "textDocument/publishDiagnostics",
                        json!({"uri": uri, "diagnostics": []}),
                    );
                    state.close(uri);
                }
            }

            "textDocument/hover" => {
                if !initialized {
                    send_error(&mut stdout, &id, -32002, "server not initialized");
                    continue;
                }
                let uri = params["textDocument"]["uri"].as_str().unwrap_or("");
                let line = params["position"]["line"].as_u64().unwrap_or(0) as usize;
                let col = params["position"]["character"].as_u64().unwrap_or(0) as usize;

                let result = state
                    .source(uri)
                    .and_then(|src| word_at(src, line, col))
                    .and_then(|word| {
                        state
                            .parse(uri)
                            .as_ref()
                            .and_then(|doc| hover_for_symbol(doc, &word))
                    })
                    .map(|md| {
                        json!({
                            "contents": {
                                "kind": "markdown",
                                "value": md
                            }
                        })
                    })
                    .unwrap_or(Value::Null);

                send_response(&mut stdout, &id, result);
            }

            "textDocument/completion" => {
                if !initialized {
                    send_error(&mut stdout, &id, -32002, "server not initialized");
                    continue;
                }
                let uri = params["textDocument"]["uri"].as_str().unwrap_or("");
                let items = state
                    .parse(uri)
                    .as_ref()
                    .map(completion_items)
                    .unwrap_or_default();
                send_response(
                    &mut stdout,
                    &id,
                    json!({"isIncomplete": false, "items": items}),
                );
            }

            "textDocument/definition" => {
                if !initialized {
                    send_error(&mut stdout, &id, -32002, "server not initialized");
                    continue;
                }
                let uri = params["textDocument"]["uri"].as_str().unwrap_or("");
                let line = params["position"]["line"].as_u64().unwrap_or(0) as usize;
                let col = params["position"]["character"].as_u64().unwrap_or(0) as usize;

                let result = state
                    .source(uri)
                    .and_then(|src| word_at(src, line, col))
                    .and_then(|word| {
                        state
                            .parse(uri)
                            .as_ref()
                            .and_then(|doc| definition_location(uri, doc, &word))
                    })
                    .unwrap_or(Value::Null);

                send_response(&mut stdout, &id, result);
            }

            "textDocument/rename" => {
                if !initialized {
                    send_error(&mut stdout, &id, -32002, "server not initialized");
                    continue;
                }
                let uri = params["textDocument"]["uri"].as_str().unwrap_or("");
                let line = params["position"]["line"].as_u64().unwrap_or(0) as usize;
                let col = params["position"]["character"].as_u64().unwrap_or(0) as usize;
                let new_name = params["newName"].as_str().unwrap_or("");

                let result = state
                    .source(uri)
                    .filter(|_| !new_name.is_empty())
                    .and_then(|src| {
                        word_at(src, line, col)
                            .map(|word| rename_locations(src, uri, &word, new_name))
                    })
                    .unwrap_or(Value::Null);
                send_response(&mut stdout, &id, result);
            }

            "textDocument/semanticTokens/full" => {
                if !initialized {
                    send_error(&mut stdout, &id, -32002, "server not initialized");
                    continue;
                }
                let uri = params["textDocument"]["uri"].as_str().unwrap_or("");
                let doc = state.parse(uri);
                let data = match state.source(uri) {
                    Some(src) => scan_semantic_tokens(src, doc.as_ref()),
                    None => vec![],
                };
                send_response(&mut stdout, &id, json!({"data": data}));
            }

            "textDocument/codeAction" => {
                if !initialized {
                    send_error(&mut stdout, &id, -32002, "server not initialized");
                    continue;
                }
                let uri = params["textDocument"]["uri"].as_str().unwrap_or("");
                let line = params["range"]["start"]["line"].as_u64().unwrap_or(0) as usize;
                let col = params["range"]["start"]["character"].as_u64().unwrap_or(0) as usize;
                let doc = state.parse(uri);
                let src = state.source(uri).unwrap_or("");
                let actions = code_actions(src, uri, doc.as_ref(), line, col);
                send_response(&mut stdout, &id, json!(actions));
            }

            _ => {
                // Unknown request with an id → method not found
                if !id.is_null() {
                    send_error(&mut stdout, &id, -32601, "method not found");
                }
            }
        }
    }
}
