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
| **Version** | 2.5.2 |
| **Status** | Stable |
| **Compiler** | Rust + Pest PEG parser |
| **Tests** | 139 unit tests |
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
- **Critical CSS inline**: in `prod` mode each page's CSS (global + components actually used) is inlined in `<style>`; `theme.css` loads deferred (`media="print"` + `data-webcore-defer`) — zero render-blocking CSS (v2.4.0)
- **SSG collections**: `"/post/:slug": PostPage each posts` — one static page generated per item of a data import; `{$route.slug}` pre-rendered; output paths validated against path traversal (v2.4.0)
- **Strict CSP — event delegation**: all inline `onclick=`/`onsubmit=` replaced by `data-webcore-e="<type>"` + delegated `D(t,p)` listener; SPA links via `data-webcore-nav`; deferred CSS via `data-webcore-defer` + `DOMContentLoaded`; `csp = true` in `webc.toml` emits `Content-Security-Policy` meta tag (v2.5.0)
- **v2.5.1 fixes**: `</style>` escaped in inlined critical CSS; `webcore.js` now included on zero-JS pages with deferred CSS; `.length` correctly counts elements in arrays with commas inside strings and characters in Unicode strings (v2.5.1)

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
    └── parser/                        # Pest → AST
           └── AST (apps · layouts · pages · components)
                  ├── codegen/html/    →  semantic HTML
                  ├── codegen/css.rs   →  scoped CSS (data-v)
                  └── codegen/js/      →  ES2022+ runtime
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
│       ├── main.rs            # CLI entry point
│       ├── grammar.pest       # PEG grammar
│       ├── parser/            # Pest → AST
│       │   ├── mod.rs
│       │   ├── declarations.rs
│       │   ├── directives.rs
│       │   └── elements.rs
│       ├── cli/               # build · serve · check commands
│       │   ├── build.rs · serve.rs · check.rs
│       │   └── config.rs · loader.rs · output.rs · assets.rs
│       ├── core/              # Types and business logic
│       │   ├── ast.rs · error.rs · ssg.rs
│       │   └── css_processor.rs · theme.rs · utils.rs
│       └── codegen/           # Code generation
│           ├── html/          # mod.rs · attrs.rs · analysis.rs · minify.rs · props.rs
│           ├── css.rs
│           └── js/            # mod.rs · js_runtime.rs · js_dom.rs · js_events.rs
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
2. Extend the AST → `core/ast.rs`
3. Update the parser → `parser/`
4. Adapt the codegen → `codegen/`
5. Add a unit test in `src/tests/`

---

## Changelog

See [CHANGELOG.md](./CHANGELOG.md).

---

## Acknowledgements

- [Pest](https://pest.rs/) — Rust PEG parser
- [Clap](https://clap.rs/) — CLI
- The Rust community
