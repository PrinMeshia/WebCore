//! HTML attribute name constants used by the `WebCore` runtime.
pub const IF: &str = "data-webcore-if";
pub const IF_ELSE: &str = "data-webcore-else";
pub const FOR: &str = "data-webcore-for";
pub const FOR_IN: &str = "data-webcore-in";
pub const FOR_KEY: &str = "data-webcore-for-key";
pub const FOR_INDEX: &str = "data-webcore-for-index";
pub const FOR_CONTAINER: &str = "data-webcore-for-container";
pub const FOR_RANGE: &str = "data-webcore-for-range";
pub const INTERPOLATION: &str = "data-webcore-interpolation";
pub const BOUND: &str = "data-webcore-bound";
pub const ERROR: &str = "data-webcore-error";
pub const REF: &str = "data-webcore-ref";
pub const TRANSITION: &str = "data-webcore-transition";
pub const SCOPE: &str = "data-v";
/// Lazy-render: element is hidden until DOMContentLoaded fires
pub const DEFER: &str = "data-webcore-defer";
/// Spread operator: all properties of the expression are applied as attributes
pub const SPREAD: &str = "data-webcore-spread";
// CSS class prefix constants (used in bindAttrs)
pub const CLASS_PREFIX: &str = "data-webcore-class-";
pub const CLASS_BOUND: &str = "data-webcore-class-bound";
pub const STYLE_PREFIX: &str = "data-webcore-style-";
