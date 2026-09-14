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
/// Per-item dynamic attribute inside a runtime `@for`: `data-webcore-fattr-<name>`
/// carries the raw loop-scoped expression (e.g. `it.url`), resolved per item by
/// `fillItem` instead of the global `_e` closure map.
pub const FOR_ATTR_PREFIX: &str = "data-webcore-fattr-";
pub const BOUND: &str = "data-webcore-bound";
pub const ERROR: &str = "data-webcore-error";
pub const REF: &str = "data-webcore-ref";
pub const TRANSITION: &str = "data-webcore-transition";
pub const SCOPE: &str = "data-v";
/// Lazy-render: element is hidden until DOMContentLoaded fires
pub const DEFER: &str = "data-webcore-defer";
/// Responsive-image marker (#74): `"<public-rel-path>|<original-width>"`,
/// consumed and removed by the build's `srcset` post-pass.
pub const IMG_MARKER: &str = "data-webcore-img";
/// Spread operator: all properties of the expression are applied as attributes
pub const SPREAD: &str = "data-webcore-spread";
/// Island (partial hydration, #50): strategy = "idle" | "visible"
pub const ISLAND: &str = "data-webcore-island";
/// Island (#50): component name, so the runtime can defer its `on:mount`
pub const ISLAND_COMPONENT: &str = "data-webcore-island-comp";
// CSS class prefix constants (used in bindAttrs)
pub const CLASS_PREFIX: &str = "data-webcore-class-";
pub const CLASS_BOUND: &str = "data-webcore-class-bound";
pub const STYLE_PREFIX: &str = "data-webcore-style-";
