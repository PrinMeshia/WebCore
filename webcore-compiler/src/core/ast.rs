//! AST definition for `WebCore` with source positions

use std::collections::HashMap;

/// Source location for error reporting
#[derive(Debug, Clone, Copy, Default)]
pub struct Span {
    /// Byte offset of the start of the span within the source file.
    /// Not used by the current compiler but retained for future LSP/IDE integration
    /// (go-to-definition, inline diagnostics, hover ranges, etc.).
    #[allow(dead_code)]
    pub start: usize,
    /// Byte offset of the end of the span within the source file.
    /// Not used by the current compiler but retained for future LSP/IDE integration.
    #[allow(dead_code)]
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
    /// Retained for future LSP/IDE integration (e.g. error range spanning multiple tokens).
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
    /// Translations keyed by locale code then message key.
    pub locales: HashMap<String, HashMap<String, String>>,
    /// Default locale code (e.g. "fr").  Empty string = no i18n configured.
    pub default_locale: String,
    /// Snake-case name of the compiled WASM package, if present.
    pub wasm_module: Option<String>,
    pub layouts: HashMap<String, Layout>,
    pub pages: HashMap<String, Page>,
    pub components: HashMap<String, Component>,
}

#[derive(Debug, Clone)]
pub struct App {
    #[allow(dead_code)]
    pub name: String,
    pub theme: Option<String>,
    pub layout: Option<String>,
    pub routes: HashMap<String, String>,
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

/// Head block inside a page: `head { title "..." meta key="value" }`
#[derive(Debug, Clone)]
pub struct HeadBlock {
    #[allow(dead_code)]
    pub title: Option<String>,
    #[allow(dead_code)]
    pub metas: Vec<(String, String)>,
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
    pub http: Option<HttpBlock>,
    pub view: Vec<Element>,
    pub style: Vec<StyleItem>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Prop {
    pub name: String,
    pub type_: Option<String>,
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
}

impl Element {
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
            | Element::ErrorBlock { span, .. } => *span,
        }
    }

    /// Returns true if this is a Tag element (reserved for future LSP use).
    #[allow(dead_code)]
    pub fn is_tag(&self) -> bool {
        matches!(self, Element::Tag { .. })
    }

    /// Returns true if this is a Text element (reserved for future LSP use).
    #[allow(dead_code)]
    pub fn is_text(&self) -> bool {
        matches!(self, Element::Text(..))
    }

    /// Returns the direct children of this element, or an empty slice.
    pub fn children(&self) -> &[Element] {
        match self {
            Element::Tag { content, .. }
            | Element::Component { content, .. }
            | Element::SlotContent { content, .. }
            | Element::For { content, .. }
            | Element::ErrorBlock { content, .. } => content,
            Element::If { then_branch, .. } => then_branch,
            _ => &[],
        }
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

#[derive(Debug, Clone)]
pub enum AttributeValue {
    String(String),
    Expression(String),
    Boolean(bool),
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

/// An item inside a `style { }` block — either a plain rule or a @media block.
#[derive(Debug, Clone)]
pub enum StyleItem {
    Rule(StyleRule),
    Media {
        query: String,
        rules: Vec<StyleRule>,
        #[allow(dead_code)]
        span: Span,
    },
}
