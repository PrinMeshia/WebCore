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
| **Version** | 3.2.0 |
| **Status** | Preview |
| **Compiler** | Rust + Pest PEG parser |
| **Tests** | 218 tests (unit, golden, integration, perf) |
| **CI** | GitHub Actions (fmt · test · clippy) |

---

## Features

> Per-version details for every feature live in the [CHANGELOG](./CHANGELOG.md);
> the full syntax reference is in [docs/spec.md](./docs/spec.md).

### Language & templating

- **Declarative blocks**: `app` (routes, layout, theme), `layout`, `page`, `component` (props · state · computed · view · style), shared global `store` (`$store.x`)
- **Expression interpolation**: `{count}`, `{count + 1}`, `{max(a, b)}` — including inside strings and attributes
- **Directives**: `@if` / `@else if` / `@else`, `@switch` / `@case` / `@default`, `@for` (with `key=` for DOM diffing, index `item, i`, ranges `0..5`), `@error` for validation messages, `@loading` / `@catch` (shorthand for `@if loading` / `@if error`), `@defer` (deferred render until DOMContentLoaded)
- **Prop shorthand**: `<Component {count}>` ≡ `<Component count={count}>`; `<div ...attrs>` for attribute spreading
- **Fragments** `<>...</>`, mixed text/element content, named multi-zone slots with default content
- **Props**: static, reactive (`value={expr}`), default values (`label: String = "Default"`), compile-time validation (warning on unknown props)
- **Build-time data imports**: `import posts from "data/posts.json"` (JSON/TOML)
- **Build-time component imports** (v3.0.1): `import Button from "./Button.webc"` — resolved at compile time, zero runtime overhead

### Reactivity & runtime

- **Fine-grained signals**: `$effect` with automatic dependency tracking — a component only re-renders when a dependency it actually read changes
- **Minimal runtime** (~2-5 KB), **tree-shaken per feature**: anything the document doesn't use isn't emitted
- **Shared, cached runtime** (v3.2): a single `/assets/webcore.<hash>.js` for the whole site (union of every page's expressions/handlers) — downloaded and cached once, no more per-page inlined/duplicated runtime
- **Events**: `on:click`, `on:submit`, `on:input`… with `|stop` `|prevent` `|once` `|self` modifiers, debounce (`on:input|debounce`), multi-statement handlers, nested objects
- **`bind:value` / `bind:checked`** two-way binding, **`ref:name`** for direct DOM access, **`$watch`** for DOM-free observation, cross-component **`emit()`**, `on:mount` / `on:destroy` hooks
- **Derived state**: `computed { fullName = firstName + " " + lastName }`
- **`http {}`**: declarative fetch (`get:` / `into:`) with auto-injected `loading` and `error`, `@loading`/`@catch` as shorthands
- **`List` methods**: `items.push(val)`, `items.remove(i)`, `items.clear()` compile to reactive `S.set()` / `STORE.set()` mutations

### Styling

- **Component-scoped CSS** (`data-v`, deterministic hash) — elided for unstyled components
- **Nesting** `&:hover`, `@media` and `@keyframes` inside `style {}` blocks, multi-element selectors
- **Bindings**: conditional classes `class:active={expr}`, dynamic inline styles `style:color={expr}`
- **Centralized theme** (`theme.toml` → CSS variables), built-in `webc:transition="fade|slide"` transitions

### Routing & pages

- **SPA**: History API, no-reload navigation, parameterized routes (`/post/:slug` → `{$route.slug}`), query string (`{$query.page}`)
- **Per-page `head {}`** (title, meta), clean URLs (`/about` without `.html`)
- **SSG collections**: `"/post/:slug": PostPage each posts` — one static page per data item

### Forms & i18n

- **Declarative validation**: `validate:required/email/minlength/maxlength/pattern` + `@error "field" {}` blocks — on blur, input and submit
- **i18n**: `locales/*.toml`, `t("key")` with parameters (`{{0}}`) and plurals (`_one`/`_other`, `{{count}}`), reactive `setLocale()`

### Performance

- **SSG**: interpolations and `@if` branches pre-rendered at compile time — zero content flash
- **Zero-JS static pages**: no script emitted when a page doesn't need one
- **Prod mode**: HTML/CSS/JS minification, inlined critical CSS (zero render-blocking CSS), `<script defer>` + preload
- **Cache busting**: content-hash filename (`webcore.<hash8>.js`) and image fingerprinting (`logo.a3f9c1b2.png`), **fully deterministic builds**
- **`webc:img`**: `loading="lazy"`, `decoding="async"` and dimensions injected at compile time (CLS prevention)

### Security

- **Strict CSP**: zero inline JS — event delegation via `data-webcore-e`, `csp = true` option emitting the Content-Security-Policy meta; v3.0 guarantees `script-src 'self'` without `'unsafe-eval'` (compiled expressions — `new Function()` removed)
- **SRI**: `integrity="sha256-…"` on scripts and stylesheets in prod
- **Dev server**: path-traversal protection (canonicalization + 403)
- **Compilation**: systematic HTML/JS escaping, ReDoS warning on `validate:pattern`, nesting limit (anti nesting-bomb)

### Tooling & DX

- **Full CLI**: `webc new` · `build` · `dev` (HMR over WebSocket) · `watch` · `check` · `fmt` (idempotent formatter) · `lsp` (LSP 3.17 server over stdin/stdout — hover, completion, go-to-definition, rename, **real-time diagnostics**, **semantic tokens**, **code actions**)
- **rustc-style errors**: source line + `^` caret + contextual hints, all errors aggregated in one pass
- **ES2022+ runtime**: private class fields, optional chaining, nullish coalescing — zero dependencies, zero transpiler; v3.0: expressions compiled to JS closures (`const _e={e0:()=>...}`) — `evalCond` / `new Function()` removed
- **`webc check`**: validates routes, components, props and detects circular references without generating anything
- **Build report**: `dist/` tree + bundle analysis (included vs tree-shaken features)
- **WASM**: `wasm/Cargo.toml` detection, `wasm-pack` build, async `globalThis.wasm` loader
- **[VS Code extension](./editors/vscode)**: highlighting, snippets, formatting via `webc fmt`

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

// @else if chaining (v3.0.7)
@if count > 9 {
    p "Double digits!"
} @else if count > 0 {
    p "Keep going…"
} @else {
    p "Press +"
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

### Component imports (v3.0.1)

```webc
import Button from "./components/Button.webc"
import Card   from "./components/Card.webc"

page "home" {
    Card {
        Button label="Submit" {}
    }
}
```

The import is resolved at compile time — no client-side loader, no runtime overhead.

### Compiled expressions (v3.0.2/v3.0.3)

In v3.0, every reactive expression is compiled to a JS closure at build time:

```js
// Generated by webc build (inline excerpt)
const _e = {
  e0: ()=>S.get('count')>0,
  e1: ()=>S.get('count')*2,
};
```

No more `evalCond()` / `new Function()` — `script-src 'self'` without `unsafe-eval` is guaranteed **structurally**.

### JS source maps — native devtools (v3.2)

In dev mode, each inline script includes a `//# sourceMappingURL=<page>.js.map` comment
and a `.map` file (source map v3, Base64-VLQ encoding) is written to `dist/<page>/`.
Browser devtools show the original `.webc` source when debugging.
Source maps are disabled in prod mode.

### Prod mode — identifier renaming (v3.0.5)

With `webc build --prod`, runtime function names are shortened:

| Before (dev) | After (prod) |
|---|---|
| `bindIf` | `_bi` |
| `bindFor` | `_bf` |
| `bindAttrs` | `_ba` |
| `bind(` | `_b(` |
| `$effect` | `_ef` |

### `http { }` block — declarative fetch (v1.3.0)

```webc
component Posts {
    state { posts: List = null }
    http { get: "/api/posts"  into: posts }
    view {
        @loading { p "Loading…" }       // shorthand for @if loading (v2.8.0)
        @catch   { p "Error: {error}" } // shorthand for @if error  (v2.8.0)
        @for post in posts { li "{post.title}" }
    }
}
```

`loading` and `error` are **auto-injected** — no need to declare them in `state`.

### Reactive `List` methods (v2.8.0)

```webc
component TodoList {
    state { todos: List = null }
    view {
        input ref:draft=true placeholder="New task"
        button on:click={todos.push(refs.draft.value)} { "Add" }
        @for todo, i in todos {
            li { "{todo}" button on:click={todos.remove(i)} { "×" } }
        }
        button on:click={todos.clear()} { "Clear all" }
    }
}
```

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

### Prebuilt binaries

Every release ships `webc` binaries for Linux, macOS (Intel and Apple
Silicon) and Windows: download the archive for your platform from the
[releases page](https://github.com/PrinMeshia/Webcore/releases), extract
`webc` and put it in your `PATH`.

### From source

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

### Auto-rebuild (without a dev server)

```bash
cd examples/counter
webc watch
# → rebuilds on every .webc or config file change
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

// v3.0 — compiled expressions (no more evalCond / new Function)
const _e = {
  e0: ()=>S.get('count')>0,
  e1: ()=>S.get('count')*2,
};
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
2. Create a branch from `develop`: `git checkout -b feature/my-feature origin/develop`
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
