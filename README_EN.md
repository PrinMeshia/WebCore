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
| **Version** | 0.7.0 |
| **Status** | Active development |
| **Compiler** | Rust + Pest PEG parser |
| **Tests** | 55 unit tests |
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
- **SPA routing**: History API + `nav()` with no full page reload
- **Reactive state**: `class State { #d = new Map() }` ES2022 with `S.get/set/on`
- **Minimal runtime**: `evalCond`, `bindIf`, `bindFor`, `bindAttrs` — <5 KB

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

### Layout and slot

```webc
layout MainLayout {
    nav {
        link to="/" { "Home" }
        link to="/about" { "About" }
    }
    main { slot content }
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
    }
}
```

### Control flow

```webc
@if count > 0 {
    p "Positive"
} @else {
    p "Zero or negative"
}

@for item in items {
    li "{item}"
}
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
./target/release/webc build --input examples/basic.webc --out dist
```

### Development server

```bash
./target/release/webc dev --input examples/basic.webc --out dist --port 3000
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

### 🔄 Phase 3 — Long-term vision (in progress)

- [x] Global store shared between components — `store { ... }` block + `$store.varName` access across the whole project
- [x] Declarative form validation — `validate:required/minlength/maxlength/email/pattern` + `@error "field" { }`
- [x] SSG — Static Site Generation (pre-render initial values, `display` preset on `@if`/`@else`)
- [x] Built-in internationalisation (i18n) — `locales/*.toml` + `t("key")` + reactive `setLocale()`
- [x] WebAssembly support (Rust business logic → WASM) — detects `wasm/Cargo.toml`, runs `wasm-pack build`, async loader exposes `globalThis.wasm`

---

## Changelog

See [CHANGELOG.md](./CHANGELOG.md).

---

## Acknowledgements

- [Pest](https://pest.rs/) — Rust PEG parser
- [Clap](https://clap.rs/) — CLI
- The Rust community
