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
                            "renameProvider": true
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
                }
            }
            "textDocument/didChange" => {
                if let Some(uri) = params["textDocument"]["uri"].as_str() {
                    if let Some(changes) = params["contentChanges"].as_array() {
                        if let Some(last) = changes.last() {
                            if let Some(text) = last["text"].as_str() {
                                state.change(uri, text.to_string());
                            }
                        }
                    }
                }
            }
            "textDocument/didClose" => {
                if let Some(uri) = params["textDocument"]["uri"].as_str() {
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

            _ => {
                // Unknown request with an id → method not found
                if !id.is_null() {
                    send_error(&mut stdout, &id, -32601, "method not found");
                }
            }
        }
    }
}
