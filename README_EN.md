![WebCore](https://github.com/PrinMeshia/Webcore/blob/main/Webcore.png)

# WebCore

**A declarative language for building web interfaces — compiled with Rust.**

> French version: [README.md](./README.md)

WebCore (`.webc`) unifies HTML, CSS and JavaScript into a single syntax.
The Rust compiler generates semantic HTML, scoped CSS and a minimal JS runtime
— no framework, no bundler, no client-side dependencies.

---

## Current status

| | |
|---|---|
| **Version** | 2.4.0 |
| **Status** | Stable |
| **Compiler** | Rust + Pest PEG parser |
| **Tests** | 128 unit tests |
| **CI** | GitHub Actions (fmt · test · clippy) |

---

## Features

- **Pest parser**: complete PEG grammar for `.webc` files
- **Structured AST**: apps, layouts, pages, components (state · view · style · props)
- **Expression interpolation**: `{count}`, `{count + 1}`, `{max(a, b)}`
- **Mixed content**: text and nested elements in the same block
- **Reactive directives**: `@if condition { }` · `@else { }` · `@for item in list { }`
- **Events**: `on:click`, `on:submit`, `on:change`, `on:input`
- **Dynamic attributes**: `class={expr}` compiled to runtime bindings
- **Scoped CSS**: per-component isolation via `data-v` (deterministic FNV-1a hash)
- **`@media` in `style { }`**: responsive media queries scoped directly inside components
- **SPA routing**: History API + `nav()` with no full page reload
- **Reactive state**: `class State { #d = new Map() }` ES2022 with `S.get/set/on`
- **Reactive props**: `Component value={expr} />` — props accept dynamic expressions
- **Named slots**: multi-zone layouts (`slot header`, `slot sidebar`, `slot content`)
- **Global store**: `store { ... }` shared across all components via `$store.varName`
- **Form validation**: `validate:required/email/minlength` + `@error "field" { }`
- **SSG**: pre-render initial values, eliminates flash of wrong content
- **i18n**: `locales/*.toml` + `t("key")` + reactive `setLocale()`
- **WASM**: detects `wasm/Cargo.toml`, runs `wasm-pack build`, async loader exposes `globalThis.wasm`
- **Minimal runtime**: `evalCond`, `bindIf`, `bindFor`, `bindAttrs` — <5 KB
- **Derived state**: `computed { fullName = firstName + " " + lastName }` — re-evaluated before every bind
- **Lifecycle hooks**: `on:mount { }` / `on:destroy { }` — run at `DOMContentLoaded` / before navigation
- **Inter-component events**: `emit("event", data)` + `on:event={handler}` on component calls
- **Parameterized routes**: `/post/:slug` accessible via `{$route.slug}` in views; tree-shaken when unused
- **`@for` with key**: `@for item key=item.id in items { }` enables key-based DOM diffing — minimal patch
- **i18n params & plural**: `t("items", count)` → `_one`/`_other` + `{{count}}`; `t("greeting", name)` → `{{0}}`
- **Compound props**: `{step + 1}` and `class={color}` substituted even in composite expressions
- **Enriched parse errors**: source line + caret `^` at the faulty column + contextual hints
- **Multi-statement handlers**: `on:click={a = 1; b = 2}` — multiple `;`-separated instructions in a single handler
- **Multi-element CSS selectors**: `input, textarea { }` supported inside `style { }` blocks
- **Forms example**: `examples/forms/` — `SignupForm` and `ContactForm` with full validation
- **`@switch` / `@case` / `@default`**: multi-branch directive compiled to an `@if`/`@else` chain — zero runtime overhead
- **`bind:` two-way binding**: `bind:value={x}` and `bind:checked={x}` expand to attribute + `on:input`/`on:change` handler
- **`@for item, i in items`**: access the current index in loops via a second variable
- **`webc check`**: validate syntax and references (routes, components, props) without generating any files
- **Clean URLs**: pages served without `.html` extension (e.g. `/about` instead of `/about.html`)
- **`dist/assets/`**: JS, CSS and public assets isolated in a dedicated subfolder
- **Build tree**: `dist/` summary with file sizes printed after every `webc build`
- **`http { }` — declarative fetch**: `get: "/url"  into: var` inside a component; `loading` and `error` auto-injected and reactive; response parsed as JSON automatically
- **`head { }` — per-page head customization**: `title "..."` and `meta name="..."` per page; overrides the global title from `webc.toml`
- **`$query.` — query string params**: `{$query.search}`, `{$query.page}` — access URL parameters; tree-shaken when unused
- **`class:active={expr}` — conditional CSS classes**: conditional class binding via `bindAttrs()`; multiple `class:` per element; tree-shaken
- **`on:event|debounce` — debounced handler**: `on:input|debounce={expr}` — fires only after 300 ms of inactivity; works with any event type
- **`ref:name=true` — direct DOM references**: `input ref:name=true` registers the element in `refs['name']` via `DOMContentLoaded`; direct access without `querySelector`; useful for focus management; tree-shaken
- **`style:prop={expr}` — dynamic inline styles**: `div style:color={myColor}` → `el.style.setProperty('color', ...)` via `bindAttrs()`; can coexist with `style="..."` and `class:`; tree-shaken
- **Slot default content**: `slot sidebar { p "Default sidebar content" }` in a layout — used when the page does not fill the slot; unfilled slots were previously silently dropped
- **`webc:transition="name"` — CSS animations**: `div webc:transition="fade" { ... }` — built-in `fade` and `slide` transitions on `@if` blocks; CSS injected automatically; tree-shaken
- **`webc:img` — optimized images**: `img webc:img src="/hero.png" alt="Hero"` injects `loading="lazy"`, `decoding="async"` and image dimensions (`width`/`height`) read from `public/` at compile time; prevents layout shift (CLS); emits `warning[a11y]` when `alt` is missing; pure compile-time transformation, zero JS emitted
- **Image fingerprinting**: every image in `public/` gets a FNV-1a 32-bit content hash at `webc build` (`logo.png` → `logo.a3f9c1b2.png`); all HTML and CSS references updated automatically; perfect cache busting with no configuration needed
- **Fine-grained signals (`$effect`)**: `$effect(fn)` replaces `VARS.forEach(v=>S.on(v,fn))` — dependency tracking is automatic; components only re-render when their actual dependencies change (v2.0.0, **breaking change**)
- **HMR**: `webc serve` watches source files and reloads the browser automatically via WebSocket — no configuration needed (v2.0.0)
- **Security — path traversal protection**: `resolve_safe_path()` uses `fs::canonicalize()` + `starts_with(dist_root)`; URLs that escape `dist/` return 403 (v2.0.0)
- **Cycle detection**: `webc check` detects circular component references and reports the full cycle (v2.0.0)
- **CSS nesting**: `&:hover { }`, `& > span { }`, `&::before { }` are valid inside `style {}` blocks; flattened to valid scoped CSS at emit time (v2.0.0)
- **Error aggregation**: `webc build` collects ALL errors and reports them in a single pass, just like the Rust compiler (v2.0.0)
- **Bundle analysis report**: after a successful `webc build`, a table shows which runtime features are included vs. tree-shaken with estimated byte sizes (v2.0.0)
- **`$watch varName => { body }`**: observe state changes without a direct DOM side-effect; emits `S.on('varName', varName => { body })` in `DOMContentLoaded`; useful for analytics, logging, and sync (v2.1.0)
- **`@for N..M` — numeric range**: `@for i in 0..5 { }` iterates `i` from 0 to 4; emits `data-webcore-for-range`; runtime generates the array without state data; tree-shaken (v2.1.0)
- **Build-time data imports**: `import posts from "data/posts.json"` — JSON/TOML files loaded at compile time and injected as `S.setQ(name, data)`; path validated via `canonicalize()` (v2.1.0)
- **Nested object literals in `on:click`**: `on:click={handler({key: val})}` — arbitrarily nested braces in event expressions via recursive `expr_brace_seq` rule (v2.1.0)
- **Extended SSG expressions**: `eval_expr_with_locale` supports `.length`, `.toUpperCase()`, `.toLowerCase()`, `.trim()` on state variables — eliminates empty values at pre-render time (v2.1.0)
- **Compile-time prop validation**: `warning[props]: component 'X' received unknown prop 'y'` emitted on stderr when a component receives an undeclared prop; compilation continues (v2.1.0)
- **`@keyframes` in `style {}`**: `@keyframes` blocks supported inside components; emitted as global (unscoped) so they can be referenced by `animation:` properties; parser, AST and CSS codegen updated (v2.2.0)
- **`<script defer>` + `<link rel="preload">`**: runtime script no longer blocks HTML parsing; preload hint in `<head>` parallelises the JS download (v2.2.0)
- **CSS hash**: `theme.css` receives `?v=<hash>` like `webcore.js` — automatic cache-busting on every modification (v2.2.0)
- **HTML minification (prod)**: comments and inter-tag whitespace removed in `prod` mode; reduces distributed page sizes (v2.2.0)
- **CSS scope elision**: components without a `style {}` block no longer emit `data-v="..."` on their elements — cleaner and lighter HTML output (v2.2.0)
- **ReDoS warning**: `validate:pattern` with nested quantifiers (`)+`, `)*`) emits `warning[security]` at compile time — prevents catastrophic backtracking (v2.2.0)
- **SRI — Subresource Integrity**: in `prod` mode, `<script>` and `<link rel="stylesheet">` tags receive `integrity="sha256-..."` + `crossorigin="anonymous"`; SHA-256 hash computed by the compiler (v2.3.0)
- **Zero-JS elision**: purely static pages (no state, no loops, no events) no longer emit `<script defer>` or `<link rel="preload">` — zero JS for content-only pages (v2.3.0)
- **Nesting depth limit**: the parser rejects documents whose elements exceed 128 nesting levels — protects against "nesting bombs" (v2.3.0)
- **Navigation URL JS escaping**: apostrophes and backslashes in `<a onclick="webcore_navigate(...)">` paths are now escaped — prevents JS injection (v2.3.0)
- **Critical CSS inline**: in `prod` mode each page's CSS (global + components actually used) is inlined in `<style>`; `theme.css` loads deferred (`media="print"` + `onload`) — zero render-blocking CSS (v2.4.0)
- **SSG collections**: `"/post/:slug": PostPage each posts` — one static page generated per item of a data import; `{$route.slug}` pre-rendered; output paths validated against path traversal (v2.4.0)

---

## Syntax

### Application

```webc
app MyApp {
    theme: "dark"
    layout: MainLayout
    routes {
        "/": HomePage
        "/about": AboutPage
    }
}
```

### Layout and named slots

```webc
layout MainLayout {
    nav {
        link to="/" { "Home" }
        link to="/about" { "About" }
    }
    main { slot content }
}

// Multi-zone layout (v0.8.0)
layout DashLayout {
    header { slot header }
    aside  { slot sidebar }
    main   { slot content }
}
```

```webc
// Page filling named slots
page "dashboard" {
    slot header  { h1 "Dashboard" }
    slot sidebar { nav { link to="/" "Home" } }
    p "Main content"   // → default content slot
}
```

### Page

```webc
page "home" {
    h1 "Welcome!"
    p "Counter: {count}"
    button on:click={count += 1} { "Increment" }
}
```

### Component with state

```webc
component Counter {
    state {
        count: Number = 0
    }

    view {
        div {
            p "Value: {count}"
            button on:click={count += 1} { "+" }
            button on:click={count = max(0, count - 1)} { "−" }
        }
    }

    style {
        div    { display: flex; gap: 1rem; align-items: center; }
        button { padding: 0.25rem 0.75rem; }
        @media (max-width: 480px) {
            div { flex-direction: column; }
        }
    }
    view { p "Hello {fullName}" }
}

// Inter-component events
component Notifier {
    view { button on:click={emit("ping", count)} { "Ping" } }
}

page "home" {
    Notifier on:ping={count += 1} {}
}
```

### Reactive props (v0.8.0)

```webc
// Component with props
component Badge {
    props { label: String, color: String }
    view { span class={color} "Status: {label}" }
}

// Usage — static or dynamic prop
page "home" {
    Badge label="Active" color="green" {}
    Badge label={statusMsg} color={statusColor} {}
}
```

### Derived state, lifecycle, and inter-component events (v0.9.0)

```webc
component FullName {
    state {
        firstName: String = "John"
        lastName: String = "Doe"
    }
    computed { fullName = firstName + " " + lastName }
    on:mount {
        firstName = "Jane"
    }
    view { p "Hello {fullName}" }
}

// Inter-component events
component Notifier {
    view { button on:click={emit("ping", count)} { "Ping" } }
}

page "home" {
    Notifier on:ping={count += 1} {}
}
```

### Control flow

```webc
@if count > 0 {
    p "Positive"
} @else {
    p "Zero or negative"
}

// Without key — full re-render on every change
@for item in items {
    li "{item}"
}

// With key — DOM diffing (v1.1.0)
@for post key=post.id in posts {
    article "{post.title}"
}

// With index — access the current rank (v1.2.0)
@for item, i in items {
    li "{i}. {item}"
}

// Multi-branch switch (v1.2.0)
@switch status {
    @case "active"   { span class="green"  "Active"  }
    @case "pending"  { span class="yellow" "Pending" }
    @default         { span class="gray"   "Unknown" }
}
```

### `bind:` two-way binding (v1.2.0)

```webc
// Keeps value and state in sync automatically
input bind:value={name}    // ≡ value={name} + on:input={name = event.target.value}
input bind:checked={agree} // ≡ checked={agree} + on:change={agree = event.target.checked}
```

### Parameterized routes (v1.1.0)

```webc
app MyApp {
    routes {
        "/":           HomePage
        "/post/:slug": PostPage
    }
}

component PostPage {
    view { h1 "Article: {$route.slug}" }
}
```

### i18n with parameters and pluralization (v1.1.0)

```toml
# locales/en.toml
items_one   = "{{count}} item"
items_other = "{{count}} items"
greeting    = "Hello, {{0}}!"
```

```webc
p "{t(\"items\", count)}"        // "3 items"
p "{t(\"greeting\", username)}"  // "Hello, Alice!"
```

### Interpolation and mixed content

```webc
p "Result: {a + b}"
p { "Hello " strong { "world" } "!" }
div class={dynamicClass} { "content" }
```

### `http { }` block — declarative fetch (v1.3.0)

```webc
component Posts {
    state { posts: List = null }
    http { get: "/api/posts"  into: posts }
    view {
        @if loading { p "Loading…" }
        @if error   { p "Error: {error}" }
        @for post in posts { li "{post.title}" }
    }
}
```

`loading` and `error` are **auto-injected** — no need to declare them in `state`.

### Conditional classes and debounce (v1.3.0)

```webc
// class:name={expr} — toggles the class based on the expression
div class:active={isOpen} class:hidden={!visible} { "content" }

// on:event|debounce — fires only after 300 ms of inactivity
input on:input|debounce={search = event.target.value}

// $query. — access URL query parameters
p "Search: {$query.search}"
p "Page: {$query.page}"
```

---

## Installation

**Requirements:** Rust 1.70+ with Cargo

```bash
git clone https://github.com/PrinMeshia/Webcore.git
cd Webcore/webcore-compiler
cargo build --release
```

### Build a project

```bash
# From inside a project directory (where webc.toml lives)
cd examples/counter
webc build
```

### Development server

```bash
cd examples/counter
webc dev
# With a custom port
webc dev 3000
```

### Validate without building

```bash
cd examples/counter
webc check
# → parse + validate routes, components and prop types without writing any files
```

---

## Architecture

```
file.webc
    └── Pest Parser
           └── AST (apps · layouts · pages · components)
                  ├── codegen_html.rs  →  semantic HTML
                  ├── codegen_css.rs   →  scoped CSS (data-v)
                  └── codegen_js.rs    →  ES2022+ runtime
```

**Generated JS runtime (excerpt):**

```js
class State { #d = new Map(); #l = new Map();
  set(k, v) { this.#d.set(k, v); this.#l.get(k)?.forEach(f => f(v)) }
  get(k)    { return this.#d.get(k) }
  on(k, f)  { (this.#l.get(k) ?? this.#l.set(k, []).get(k)).push(f) }
}
const S = new State();
```

---

## Project structure

```
Webcore/
├── webcore-compiler/
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs           # CLI: build, dev
│       ├── grammar.pest      # PEG grammar
│       ├── parser.rs         # Pest → AST
│       ├── ast.rs            # AST types
│       ├── errors.rs         # Error handling
│       ├── css_processor.rs  # CSS post-processing
│       ├── theme.rs          # Theme support
│       └── codegen/
│           ├── codegen_html.rs
│           ├── codegen_css.rs
│           └── codegen_js.rs
└── .github/
    └── workflows/
        └── ci.yml
```

---

## Development

```bash
cd webcore-compiler

# Run tests
cargo test

# Lint
cargo clippy -- -D warnings

# Format
cargo fmt
```

---

## Contributing

1. Fork the project
2. Create a branch: `git checkout -b feature/my-feature`
3. Commit: `git commit -m 'feat: description'`
4. Push: `git push origin feature/my-feature`
5. Open a Pull Request

**Workflow for adding a feature:**

1. Modify the grammar → `grammar.pest`
2. Extend the AST → `ast.rs`
3. Update the parser → `parser.rs`
4. Adapt the codegen → `codegen/`
5. Add a unit test

---

## Roadmap

### ✅ v0.2.0 — Shipped

| Feature | Detail |
|---|---|
| PEG parser | Complete Pest grammar for `.webc` |
| AST | Apps · layouts · pages · components · state · view · style · props |
| HTML codegen | Semantic generation with CSS scoping (`data-v`) |
| CSS codegen | Per-component isolation, deterministic FNV-1a hash |
| JS codegen | ES2022+ runtime: `State`, `evalCond`, `bindIf/For/Attrs` |
| `@if` / `@for` | Reactive directives with DOM binding |
| Expression interpolation | `{count + 1}`, `{max(a, b)}` in strings |
| Mixed content | Text and elements in the same block |
| Dynamic attributes | `class={expr}` → runtime binding |
| SPA routing | History API + `nav()` without full reload |
| Event handlers | `on:click`, `on:submit`, `on:change`, `on:input` |
| CI/CD | GitHub Actions: fmt · test · clippy |

---

### ✅ Phase 1 — Stabilisation (complete)

- [x] Error messages with line numbers (Pest `Display` format: file · line · col · context)
- [x] Golden tests (8 full-pipeline tests: parse → HTML/CSS/JS)
- [x] Inter-component props (`props { name: String }` → static substitution at codegen time)
- [x] CSS minification (LightningCSS — already wired, activated in `prod` mode)
- [x] JS minification (strip comments + join lines, activated in `prod` mode)
- [x] License (MIT)

---

### ✅ Phase 2 — Developer experience (complete)

- [x] `webc new <name>` — full project scaffold (webc.toml, theme.toml, layouts, pages, Counter, public/)
- [x] Application examples (`examples/counter`, `examples/todo`, `examples/blog`)
- [x] Language specification (`docs/spec.md`) — complete reference, 16 sections
- [x] True hot reload via WebSockets — persistent connection on `port+1`, instant reload, auto-reconnect
- [x] VS Code extension (`editors/vscode/`) — TextMate syntax highlighting for `.webc`

---

### ✅ Phase 3 — Long-term vision (complete)

- [x] Global store shared between components — `store { ... }` block + `$store.varName` access across the whole project
- [x] Declarative form validation — `validate:required/minlength/maxlength/email/pattern` + `@error "field" { }`
- [x] SSG — Static Site Generation (pre-render initial values, `display` preset on `@if`/`@else`)
- [x] Built-in internationalisation (i18n) — `locales/*.toml` + `t("key")` + reactive `setLocale()`
- [x] WebAssembly support (Rust business logic → WASM) — detects `wasm/Cargo.toml`, runs `wasm-pack build`, async loader exposes `globalThis.wasm`

---

### ✅ v0.8.0 — Component model v2 (complete)

- [x] **Reactive props** — `Component value={expr} />`: props accept dynamic expressions; `{propName}` in the view becomes a reactive span instead of static text
- [x] **Named slots** — multi-zone layouts (`slot header`, `slot sidebar`, `slot content`); pages provide content via `slot header { ... }`; recursive resolution at any depth
- [x] **`@media` in `style { }`** — responsive media queries scoped per component; `data-v` scoping propagated inside `@media` blocks

---

### ✅ v0.9.0 — Component model v3 (complete)

- [x] **Derived state** — `computed { fullName = firstName + " " + lastName }`; re-evaluated via `rebindComputed()` before every DOM bind; `setQ` (silent setter) on `State` to avoid reactive loops
- [x] **`on:mount` lifecycle hook** — `on:mount { }` block: raw JS executed in `DOMContentLoaded` after initialization; wrapped in an IIFE for local variable isolation
- [x] **Inter-component events** — `emit("event", data)` compiled to `CustomEvent` dispatch; `on:event={handler}` on component calls → `document.addEventListener` registered in `DOMContentLoaded`

---

### ✅ v1.0.0 — Stable release (complete)

- [x] **`on:destroy { }`** — lifecycle hook symmetric to `on:mount`; runs before every SPA navigation and on `window.beforeunload`
- [x] **Runtime tree-shaking** — `bindFor`, `bindIf`, `bindAttrs`, `validateField`/`bindValidation`, `nav`, `evalCond`, `VARS`/`STORE_VARS`, `COMPUTED`/`rebindComputed` only emitted when the document actually uses them; significant savings for simple apps
- [x] **Documentation site** — `examples/docs/`: site built entirely with WebCore itself (4 pages, 2 components, layout, styles)

---

### ✅ v1.1.0 — Routes, DX & polish (complete)

- [x] **Parameterized routes** — `/post/:slug`, `/user/:id`: `ROUTES[]` with RegExp + `ROUTE_PARAMS`; accessible via `{$route.slug}` in views; tree-shaken when unused
- [x] **`@for` with key** — `@for item key=item.id in items { }`: key-based DOM diffing, minimal patch instead of full re-render
- [x] **i18n params + pluralization** — `t("key", n)` → `_one`/`_other` keys + `{{count}}`; `t("key", val)` → `{{0}}`
- [x] **Compound props** — `{step + 1}`, `class={color}`: word-boundary substitution in composite expressions and attributes
- [x] **Enriched parse errors** — source line + caret `^` at the faulty column + contextual hints for common mistakes

---

### ✅ v1.2.0 — DX & directives (complete)

- [x] **`@switch` / `@case` / `@default`** — multi-branch directive compiled to an `@if`/`@else` chain at parse time; zero runtime overhead
- [x] **`bind:` two-way binding** — `bind:value={x}` / `bind:checked={x}`: attribute + handler generated automatically by `expand_bind_attrs()`
- [x] **`@for item, i in items`** — current index accessible inside the loop; `data-webcore-for-index` on the `<template>`; `fillItem` injects the value
- [x] **`webc check`** — CLI validation without file generation: parse + routes/components/props consistency check
- [x] **Clean URLs** — `slug/index.html` instead of `slug.html`; dev server resolves `/about` → `dist/about/index.html`
- [x] **`dist/assets/`** — JS/CSS/public assets in a subfolder, HTML at the root; absolute paths `/assets/`
- [x] **Build tree** — `dist/` summary with file sizes printed after every `webc build`
- [x] **Public CSS minified** — `public/*.css` processed by LightningCSS in `prod` mode

---

### ✅ v1.1.1 — Bug fixes & polish (complete)

- [x] **Form validation** — capture-phase listener + `stopImmediatePropagation()` ensures validation runs before `on:submit`; `@error` block content preserved via `firstElementChild`
- [x] **Multi-statement handlers** — `on:click={a = 1; b = 2}`: each `;`-separated instruction compiled independently
- [x] **`on:mount` deep nesting** — Pest grammar now supports arbitrarily nested braces in `on:mount { }` bodies
- [x] **`t()` inside `evalCond`** — passed explicitly to `new Function()` calls; internal variable renamed `_c` to prevent shadowing
- [x] **Multi-element CSS selectors** — `input, textarea { }` now valid inside `style { }` blocks
- [x] **`@for` key without wrapper div** — key placed on `firstElementChild`, eliminating extra spacing between list items
- [x] **`examples/forms/`** — `SignupForm` + `ContactForm` with full validation, computed state, character counter

---

### ✅ v1.3.0 — HTTP, head, query params, conditional classes, debounce (complete)

- [x] **`http { }` — declarative fetch** — `get: "/url"  into: var` inside a component; `loading`/`error` auto-injected and reactive; JSON response parsed automatically
- [x] **`head { }` — head customization** — `title "..."` and `meta name="..."` per page; overrides the global title from `webc.toml`
- [x] **`$query.` — query string params** — `{$query.search}`, `{$query.page}`; tree-shaken when unused
- [x] **`class:name={expr}` — conditional classes** — `class:active={isOpen}`; `bindAttrs()` handles the toggle; tree-shaken
- [x] **`on:event|debounce`** — debounced handler (300 ms default); works with any event type

---

### ✅ v1.4.0 — DOM refs, dynamic styles, slot defaults, transitions (complete)

- [x] **`ref:name=true` — direct DOM references** — `input ref:name=true` emits `data-webcore-ref="name"`; `refs['name']` accessible after `DOMContentLoaded`; direct access without `querySelector`; tree-shaken via `has_refs`
- [x] **`style:prop={expr}` — dynamic inline styles** — `div style:color={myColor}` → `el.style.setProperty('color', ...)` via `bindAttrs()`; can coexist with `style="..."` and `class:`; tree-shaken via `has_style_binding`
- [x] **Slot default content** — `slot sidebar { p "Default content" }` in a layout; used when the page does not fill the slot; unfilled slots were previously silently dropped
- [x] **`webc:transition="name"` — CSS animations** — `div webc:transition="fade" { ... }`; built-in `fade` and `slide` transitions on `@if` blocks; CSS injected automatically; tree-shaken via `has_transition`

---

### ✅ v1.5.0 — Optimized images and fingerprinting (complete)

- [x] **`webc:img` — optimized images** — `img webc:img src="/hero.png" alt="Hero"` compiles to `<img src="/assets/hero.png" loading="lazy" decoding="async" width="1200" height="630" alt="Hero">`; dimensions read from `public/` at compile time (prevents CLS); `warning[a11y]` emitted when `alt` is missing; `webc:img` stripped from HTML output; zero JS emitted; uses `imagesize` crate
- [x] **Image fingerprinting** — every image in `public/` gets a FNV-1a 32-bit content hash at `webc build` (`logo.png` → `logo.a3f9c1b2.png`); extensions: `.png`, `.jpg`, `.jpeg`, `.gif`, `.webp`, `.svg`, `.ico`, `.avif`; all HTML and CSS references updated automatically; perfect cache busting, no configuration required

---

### ✅ v2.0.0 — Fine-grained signals, HMR & security (complete)

- [x] **Fine-grained signals (`$effect`)** — automatic dependency tracking; components only re-render when their actual dependencies change; **breaking change** from v1.x
- [x] **HMR** — `webc serve` watches files and reloads via WebSocket — no configuration needed
- [x] **Path traversal security** — `resolve_safe_path()`: `canonicalize()` + `starts_with(dist_root)` check; 403 returned for URLs outside `dist/`
- [x] **Cycle detection** — `webc check` detects circular component references and reports the full cycle
- [x] **Error aggregation** — all errors collected and reported in a single pass
- [x] **CSS nesting** — `&:hover`, `& > span`, `&::before` flattened to valid scoped CSS
- [x] **Bundle analysis report** — table of included vs. tree-shaken runtime features printed after every build

---

### ✅ v2.1.0 — Advanced reactivity & data (complete)

- [x] **`$watch`** — `$watch varName => { body }`: reactive observer without a DOM side-effect; `S.on('varName', fn)` in `DOMContentLoaded`
- [x] **Nested object literals in `on:click`** — `on:click={handler({key: val})}` via recursive `expr_brace_seq` rule in the grammar
- [x] **Complex `@for key={expr}`** — arbitrary expression as DOM diffing key (`key={item.id + "-" + item.type}`)
- [x] **`@for N..M` — numeric range** — `@for i in 0..5` emits `data-webcore-for-range`; runtime generates the array without state data
- [x] **Extended SSG expressions** — `.length`, `.toUpperCase()`, `.toLowerCase()`, `.trim()` in `eval_expr_with_locale`
- [x] **Prop validation** — `warning[props]` when an undeclared prop is received; compilation continues
- [x] **Build-time data imports** — `import name from "file.json"`; path validated via `canonicalize()`

---

### ✅ v2.2.0 — Performance & tooling (complete)

- [x] **`@keyframes`** — `@keyframes` blocks in `style {}`; emitted as global (unscoped); parser, AST (`StyleItem::Keyframes`), grammar and codegen updated
- [x] **`<script defer>` + preload** — non-blocking script; `<link rel="preload" as="script">` in `<head>` for parallel JS download
- [x] **CSS hash** — `theme.css?v=<hash>` — automatic cache-busting like `webcore.js`
- [x] **HTML minification (prod)** — comments and inter-tag whitespace removed in `prod` mode
- [x] **CSS scope elision** — components without `style {}` no longer emit `data-v` — cleaner HTML output
- [x] **ReDoS warning** — `validate:pattern` with `)+`/`)*` → `warning[security]` at compile time

---

### ✅ v2.3.0 — Security & zero-weight (complete)

- [x] **SRI — Subresource Integrity** — `integrity="sha256-..."` + `crossorigin="anonymous"` on `<script>` and `<link>` in `prod` mode; SHA-256 hash computed by the compiler (`sha2` crate)
- [x] **Zero-JS elision** — purely static pages emit no `<script defer>` or `<link rel="preload">` — zero JS requests for content-only pages
- [x] **Nesting depth limit (128 levels)** — the parser rejects "nesting bombs" with an explicit error message
- [x] **Navigation URL JS escaping** — apostrophes and backslashes in `<a onclick="webcore_navigate(...)">` paths are escaped

---

### ✅ v2.4.0 — Critical CSS & SSG collections (complete)

- [x] **Critical CSS inline (prod)** — per-page CSS (global + components actually used, recursive collection) inlined in `<style>`; `theme.css` deferred via `media="print"` + `onload` swap + `<noscript>` fallback; hash and SRI preserved
- [x] **SSG collections** — `"/post/:slug": PostPage each posts`: one static page per item of the bound data import; `{$route.slug}` pre-rendered; path values validated (rejects `/`, `\`, `..`)
- [x] **Data import resolution** — `import name from "file.json"` actually resolved at build time (JSON/TOML → `S.setQ`) with path-traversal guard
- [x] **Preload fix** — `?v=hash` + SRI now applied to the `<link rel="preload">` hint (attribute order corrected)

---

## Changelog

See [CHANGELOG.md](./CHANGELOG.md).

---

## Acknowledgements

- [Pest](https://pest.rs/) — Rust PEG parser
- [Clap](https://clap.rs/) — CLI
- The Rust community
