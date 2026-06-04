//! AST definition for WebCore with source positions

use std::collections::HashMap;

/// Source location for error reporting
#[derive(Debug, Clone, Copy, Default)]
pub struct Span {
    pub start: usize,
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
    pub name: String,
    pub theme: Option<String>,
    pub layout: Option<String>,
    pub routes: HashMap<String, String>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Layout {
    pub name: String,
    pub content: Vec<Element>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Page {
    pub name: String,
    pub content: Vec<Element>,
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
    pub view: Vec<Element>,
    pub style: Vec<StyleItem>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Prop {
    pub name: String,
    pub type_: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ComputedVar {
    pub name: String,
    pub expr: String,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct StateVar {
    pub name: String,
    pub type_: String,
    pub default_value: Option<String>,
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
    /// Loop: @for item [key=expr] in items { ... }
    For {
        item: String,
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
    pub fn span(&self) -> Span {
        match self {
            Element::Text(_, span) => *span,
            Element::Tag { span, .. } => *span,
            Element::Slot(_, span) => *span,
            Element::SlotContent { span, .. } => *span,
            Element::Component { span, .. } => *span,
            Element::Interpolation(_, span) => *span,
            Element::For { span, .. } => *span,
            Element::If { span, .. } => *span,
            Element::ErrorBlock { span, .. } => *span,
        }
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
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct StyleProperty {
    pub name: String,
    pub value: String,
    pub span: Span,
}

/// An item inside a `style { }` block — either a plain rule or a @media block.
#[derive(Debug, Clone)]
pub enum StyleItem {
    Rule(StyleRule),
    Media {
        query: String,
        rules: Vec<StyleRule>,
        span: Span,
    },
}
