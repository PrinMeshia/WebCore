//! AST definition for `WebCore` with source positions

use std::collections::BTreeMap;
use std::collections::BTreeSet;

/// Source location for error reporting
#[derive(Debug, Clone, Copy, Default)]
pub struct Span {
    /// Byte offset of the start of the span within the source file.
    pub start: usize,
    /// Byte offset of the end of the span within the source file.
    pub end: usize,
    pub line: u32,
    pub col: u32,
}

impl Span {
    pub fn new(start: usize, end: usize, line: u32, col: u32) -> Self {
        Self {
            start,
            end,
            line,
            col,
        }
    }

    pub fn from_pest(span: pest::Span) -> Self {
        let (line, col) = span.start_pos().line_col();
        Self {
            start: span.start(),
            end: span.end(),
            line: line as u32,
            col: col as u32,
        }
    }

    /// Merge two spans into one covering both ranges.
    #[allow(dead_code)]
    pub fn merge(self, other: Self) -> Self {
        Self {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
            line: self.line.min(other.line),
            col: if self.line <= other.line {
                self.col
            } else {
                other.col
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct WebCoreDocument {
    pub app: Option<App>,
    pub store: Vec<StateVar>,
    pub store_computed: Vec<ComputedVar>,
    /// Translations keyed by locale code then message key.
    pub locales: BTreeMap<String, BTreeMap<String, String>>,
    /// Default locale code (e.g. "fr").  Empty string = no i18n configured.
    pub default_locale: String,
    /// Snake-case name of the compiled WASM package, if present.
    pub wasm_module: Option<String>,
    /// `[app] view_transitions` — wrap SPA navigation in the View Transitions
    /// API (`document.startViewTransition`) for animated page changes.
    pub view_transitions: bool,
    pub layouts: BTreeMap<String, Layout>,
    pub pages: BTreeMap<String, Page>,
    pub components: BTreeMap<String, Component>,
    /// Build-time data imports (`import posts from "data/posts.json"`).
    #[allow(dead_code)]
    pub imports: Vec<ImportDecl>,
    /// Resolved data imports: name → JSON string.
    #[allow(dead_code)]
    pub data_imports: BTreeMap<String, String>,
    /// Component import declarations collected during parsing (transient).
    /// Consumed by `resolve_component_imports` to populate `page_imports`.
    pub component_imports: Vec<ComponentImport>,
    /// Per-page component availability graph (v3 import system).
    ///
    /// - Key present → page declared explicit imports; only those component
    ///   names are in scope when compiling that page.
    /// - Key absent → no import declarations in the page file; all components
    ///   in `self.components` are available (v2-compatible behaviour).
    pub page_imports: BTreeMap<String, BTreeSet<String>>,
    /// Source file path for each top-level unit (page / component / layout),
    /// keyed by its name. Populated by the loader so `webc check` can attach
    /// precise `file:line:col` to diagnostics found while walking the AST.
    /// Empty when the document is built outside the file loader (e.g. tests).
    pub source_files: BTreeMap<String, std::path::PathBuf>,
}

/// A `$watch varName => { body }` hook inside a component.
#[derive(Debug, Clone)]
pub struct WatchHook {
    #[allow(dead_code)]
    pub var: String,
    #[allow(dead_code)]
    pub body: String,
}

/// A build-time data import: `import name from "data/posts.json"`.
#[derive(Debug, Clone)]
pub struct ImportDecl {
    #[allow(dead_code)]
    pub name: String,
    #[allow(dead_code)]
    pub path: String,
}

/// A build-time component import: `import Button from "./Button.webc"`.
/// Routed separately from data imports by the parser (`.webc` extension).
#[derive(Debug, Clone)]
pub struct ComponentImport {
    /// Name used in the importing file (e.g. `Button`).
    pub alias: String,
    /// Path as written in the source (relative to the importing file or to the project root).
    pub path: String,
}

#[derive(Debug, Clone)]
pub struct App {
    #[allow(dead_code)]
    pub name: String,
    pub theme: Option<String>,
    pub layout: Option<String>,
    pub routes: BTreeMap<String, String>,
    /// SSG collections: route path → data-import name.
    /// `"/post/:slug": PostPage each posts` → `{"/post/:slug": "posts"}`.
    /// At build time one static page is generated per item of the collection.
    pub collections: BTreeMap<String, String>,
    /// Site-wide `head { }` declared in `app { }`: merged into every page's
    /// `<head>` (favicon, shared meta), overridable per page.
    pub head: Option<HeadBlock>,
    #[allow(dead_code)]
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Layout {
    pub name: String,
    pub content: Vec<Element>,
    #[allow(dead_code)]
    pub span: Span,
}

/// HTTP fetch block inside a component: `http { get: "/api/posts" into: posts }`
#[derive(Debug, Clone)]
pub struct HttpBlock {
    #[allow(dead_code)]
    pub method: String,
    pub url: String,
    pub into: String,
}

/// A `<meta>` declaration: `meta key = "value"` → `<meta name="key"
/// content="value">`, plus any extra attributes (`media`, …).
#[derive(Debug, Clone)]
pub struct HeadMeta {
    /// The `name`/`property` key (mapped in codegen; `og:*` → `property`).
    pub key: String,
    /// The `content` value (may carry `{…}` interpolations).
    pub value: String,
    /// Ordered extra `(attr, value)` pairs emitted verbatim after `content`.
    pub extra: Vec<(String, String)>,
}

/// A recursive JSON-LD value: a scalar string (possibly interpolated), a nested
/// object (ordered fields), or an array. Serialised into `application/ld+json`.
#[derive(Debug, Clone)]
pub enum JsonLdValue {
    /// Scalar string; may carry `{…}` interpolations resolved at build.
    Str(String),
    Array(Vec<JsonLdValue>),
    Object(Vec<(String, JsonLdValue)>),
}

/// Head block inside a page: `head { title "..." meta key="value" }`
#[derive(Debug, Clone)]
pub struct HeadBlock {
    pub title: Option<String>,
    pub metas: Vec<HeadMeta>,
    /// Generic `<link>` tags: each entry is an ordered list of `(attr, value)`
    /// pairs (`link rel="preload" href="..." as="font"`).
    pub links: Vec<Vec<(String, String)>>,
    /// Declarative JSON-LD: the top-level object's ordered fields → serialised
    /// into a `<script type="application/ld+json">`. Empty means no script.
    pub jsonld: Vec<(String, JsonLdValue)>,
    /// Favicon path (`favicon "/logo.png"`) → `<link rel="icon" href="...">`.
    pub favicon: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Page {
    pub name: String,
    #[allow(dead_code)]
    pub head: Option<HeadBlock>,
    pub content: Vec<Element>,
    #[allow(dead_code)]
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Component {
    pub name: String,
    pub props: Vec<Prop>,
    pub state: Vec<StateVar>,
    pub computed: Vec<ComputedVar>,
    pub mount_body: Option<String>,
    pub destroy_body: Option<String>,
    #[allow(dead_code)]
    pub watch_hooks: Vec<WatchHook>,
    pub http: Option<HttpBlock>,
    pub view: Vec<Element>,
    pub style: Vec<StyleItem>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Prop {
    pub name: String,
    pub type_: Option<String>,
    pub default_value: Option<String>,
    #[allow(dead_code)]
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ComputedVar {
    pub name: String,
    pub expr: String,
    #[allow(dead_code)]
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct StateVar {
    pub name: String,
    pub type_: String,
    pub default_value: Option<String>,
    #[allow(dead_code)]
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Element {
    Text(String, Span),
    Tag {
        name: String,
        attributes: Vec<Attribute>,
        content: Vec<Element>,
        span: Span,
    },
    /// Slot placeholder in layouts: `slot header`
    Slot(String, Span),
    /// Slot content provision in pages: `slot header { ... }`
    SlotContent {
        name: String,
        content: Vec<Element>,
        span: Span,
    },
    Component {
        name: String,
        attributes: Vec<Attribute>,
        content: Vec<Element>,
        span: Span,
    },
    Interpolation(String, Span),
    /// Loop: @for item [, index] [key=expr] in items { ... }
    For {
        item: String,
        index: Option<String>,
        iterable: String,
        key: Option<String>,
        content: Vec<Element>,
        span: Span,
    },
    /// Conditional: @if condition { ... } @else { ... }
    If {
        condition: String,
        then_branch: Vec<Element>,
        else_branch: Option<Vec<Element>>,
        span: Span,
    },
    /// Form validation error display: @error "fieldname" { ... }
    ErrorBlock {
        field: String,
        content: Vec<Element>,
        span: Span,
    },
    /// Fragment shorthand: <>...</> renders children inline with no wrapper tag
    Fragment {
        content: Vec<Element>,
        span: Span,
    },
    /// Lazy-render block: hidden until DOMContentLoaded fires
    Defer {
        content: Vec<Element>,
        span: Span,
    },
    /// Inline a Markdown file rendered to HTML at build time: `markdown "x.md"`.
    Markdown(String, Span),
}

impl Element {
    /// Child element vectors of this node (an `@if` contributes both branches).
    ///
    /// This — together with [`Element::child_vecs_mut`] — is the **single place**
    /// that must know every variant's structure: recursive passes (a11y lints,
    /// i18n checks, state scoping, feature detection, …) recurse through it
    /// instead of each re-implementing an exhaustive `match`. Adding a variant
    /// only requires updating these two methods (the compiler enforces it).
    pub fn child_vecs(&self) -> Vec<&Vec<Element>> {
        match self {
            Element::Tag { content, .. }
            | Element::Component { content, .. }
            | Element::For { content, .. }
            | Element::Fragment { content, .. }
            | Element::Defer { content, .. }
            | Element::SlotContent { content, .. }
            | Element::ErrorBlock { content, .. } => vec![content],
            Element::If {
                then_branch,
                else_branch,
                ..
            } => match else_branch {
                Some(eb) => vec![then_branch, eb],
                None => vec![then_branch],
            },
            Element::Text(_, _)
            | Element::Slot(_, _)
            | Element::Interpolation(_, _)
            | Element::Markdown(_, _) => Vec::new(),
        }
    }

    /// Mutable variant of [`Element::child_vecs`].
    pub fn child_vecs_mut(&mut self) -> Vec<&mut Vec<Element>> {
        match self {
            Element::Tag { content, .. }
            | Element::Component { content, .. }
            | Element::For { content, .. }
            | Element::Fragment { content, .. }
            | Element::Defer { content, .. }
            | Element::SlotContent { content, .. }
            | Element::ErrorBlock { content, .. } => vec![content],
            Element::If {
                then_branch,
                else_branch,
                ..
            } => match else_branch {
                Some(eb) => vec![then_branch, eb],
                None => vec![then_branch],
            },
            Element::Text(_, _)
            | Element::Slot(_, _)
            | Element::Interpolation(_, _)
            | Element::Markdown(_, _) => Vec::new(),
        }
    }

    /// Returns the source span of this element (reserved for future LSP/IDE use).
    #[allow(dead_code)]
    pub fn span(&self) -> Span {
        match self {
            Element::Text(_, span)
            | Element::Slot(_, span)
            | Element::Interpolation(_, span)
            | Element::Tag { span, .. }
            | Element::SlotContent { span, .. }
            | Element::Component { span, .. }
            | Element::For { span, .. }
            | Element::If { span, .. }
            | Element::ErrorBlock { span, .. }
            | Element::Fragment { span, .. }
            | Element::Defer { span, .. }
            | Element::Markdown(_, span) => *span,
        }
    }

    /// Returns true if this is a Tag element (reserved for future LSP use).
    #[allow(dead_code)]
    pub fn is_tag(&self) -> bool {
        matches!(self, Element::Tag { .. })
    }
}

/// Depth-first pre-order walk: calls `f` on every element, then recurses into
/// its children (via [`Element::child_vecs`]). Passes that only need a
/// per-node action use this instead of hand-rolled recursive `match`es.
pub fn walk_elements<'a>(elements: &'a [Element], f: &mut dyn FnMut(&'a Element)) {
    for el in elements {
        f(el);
        for child_vec in el.child_vecs() {
            walk_elements(child_vec, f);
        }
    }
}

/// Mutable depth-first pre-order walk (via [`Element::child_vecs_mut`]).
pub fn walk_elements_mut(elements: &mut [Element], f: &mut dyn FnMut(&mut Element)) {
    for el in elements {
        f(el);
        for child_vec in el.child_vecs_mut() {
            walk_elements_mut(child_vec, f);
        }
    }
}

impl Element {
    /// Returns true if this is a Text element (reserved for future LSP use).
    #[allow(dead_code)]
    pub fn is_text(&self) -> bool {
        matches!(self, Element::Text(..))
    }
}

impl Component {
    /// Returns true if this component has an HTTP block (reserved for future LSP use).
    #[allow(dead_code)]
    pub fn has_http(&self) -> bool {
        self.http.is_some()
    }

    /// Returns true if this component has computed vars (reserved for future LSP use).
    #[allow(dead_code)]
    pub fn has_computed(&self) -> bool {
        !self.computed.is_empty()
    }

    /// Returns true if this component has state vars (reserved for future LSP use).
    #[allow(dead_code)]
    pub fn has_state(&self) -> bool {
        !self.state.is_empty()
    }

    /// Returns true if this component is reactive (reserved for future LSP use).
    #[allow(dead_code)]
    pub fn is_reactive(&self) -> bool {
        !self.state.is_empty() || !self.computed.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct Attribute {
    pub name: String,
    pub value: AttributeValue,
    pub span: Span,
}

/// Extract a `client="<strategy>"` partial-hydration directive (islands, #50)
/// from a component instance's attributes. Returns the deferring strategies
/// `"idle"` or `"visible"`; `"load"` (eager, the default) and unknown values
/// yield `None` — an eager component needs no island marker.
pub(crate) fn island_strategy(attributes: &[Attribute]) -> Option<&str> {
    attributes.iter().find_map(|a| {
        if a.name != "client" {
            return None;
        }
        match &a.value {
            AttributeValue::String(v) if matches!(v.as_str(), "idle" | "visible") => {
                Some(v.as_str())
            }
            _ => None,
        }
    })
}

#[derive(Debug, Clone)]
pub enum AttributeValue {
    String(String),
    Expression(String),
    Boolean(bool),
    /// Spread all properties of the named variable onto this element: `...obj`
    Spread(String),
}

#[derive(Debug, Clone)]
pub struct StyleRule {
    pub selector: String,
    pub properties: Vec<StyleProperty>,
    /// Nested CSS rules, e.g. `&:hover { color: red; }` inside this rule.
    pub nested: Vec<StyleRule>,
    #[allow(dead_code)]
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct StyleProperty {
    pub name: String,
    pub value: String,
    #[allow(dead_code)]
    pub span: Span,
}

/// A single step inside a `@keyframes` block (e.g. `from`, `to`, `50%`).
#[derive(Debug, Clone)]
pub struct KeyframeStep {
    pub selector: String,
    pub properties: Vec<StyleProperty>,
}

/// An item inside a `style { }` block — either a plain rule, a @media block, or @keyframes.
#[derive(Debug, Clone)]
pub enum StyleItem {
    Rule(StyleRule),
    Media {
        query: String,
        rules: Vec<StyleRule>,
        #[allow(dead_code)]
        span: Span,
    },
    Keyframes {
        name: String,
        steps: Vec<KeyframeStep>,
    },
}
