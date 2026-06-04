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
| **Version** | 1.1.1 |
| **Status** | Stable |
| **Compiler** | Rust + Pest PEG parser |
| **Tests** | 76 unit tests |
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

### ✅ v1.1.1 — Bug fixes & polish (complete)

- [x] **Form validation** — capture-phase listener + `stopImmediatePropagation()` ensures validation runs before `on:submit`; `@error` block content preserved via `firstElementChild`
- [x] **Multi-statement handlers** — `on:click={a = 1; b = 2}`: each `;`-separated instruction compiled independently
- [x] **`on:mount` deep nesting** — Pest grammar now supports arbitrarily nested braces in `on:mount { }` bodies
- [x] **`t()` inside `evalCond`** — passed explicitly to `new Function()` calls; internal variable renamed `_c` to prevent shadowing
- [x] **Multi-element CSS selectors** — `input, textarea { }` now valid inside `style { }` blocks
- [x] **`@for` key without wrapper div** — key placed on `firstElementChild`, eliminating extra spacing between list items
- [x] **`examples/forms/`** — `SignupForm` + `ContactForm` with full validation, computed state, character counter

---

## Changelog

See [CHANGELOG.md](./CHANGELOG.md).

---

## Acknowledgements

- [Pest](https://pest.rs/) — Rust PEG parser
- [Clap](https://clap.rs/) — CLI
- The Rust community
