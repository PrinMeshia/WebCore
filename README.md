![WebCore](https://github.com/PrinMeshia/Webcore/blob/main/Webcore.png)

# WebCore

**Un langage déclaratif pour construire des interfaces web — compilé en Rust.**

> Version anglaise : [README_EN.md](./README_EN.md)

WebCore (`.webc`) unifie HTML, CSS et JavaScript dans une syntaxe unique.
Le compilateur Rust génère un HTML sémantique, un CSS scopé et un runtime JS minimaliste
— sans framework, sans bundler, sans dépendances côté client.

---

## État actuel

| | |
|---|---|
| **Version** | 0.7.0 |
| **Statut** | En développement actif |
| **Compilateur** | Rust + Pest PEG parser |
| **Tests** | 55 tests unitaires |
| **CI** | GitHub Actions (fmt · test · clippy) |

---

## Fonctionnalités

- **Parser Pest** : grammaire PEG complète pour les fichiers `.webc`
- **AST structuré** : apps, layouts, pages, composants (state · view · style · props)
- **Interpolation d'expressions** : `{count}`, `{count + 1}`, `{max(a, b)}`
- **Contenu mixte** : texte et éléments imbriqués dans le même bloc
- **Directives réactives** : `@if condition { }` · `@else { }` · `@for item in list { }`
- **Événements** : `on:click`, `on:submit`, `on:change`, `on:input`
- **Attributs dynamiques** : `class={expr}` compilé en binding runtime
- **CSS scopé** : isolation par composant via `data-v` (hash FNV-1a déterministe)
- **Routage SPA** : History API + `nav()` sans rechargement de page
- **État réactif** : `class State { #d = new Map() }` ES2022 avec `S.get/set/on`
- **Runtime minimal** : `evalCond`, `bindIf`, `bindFor`, `bindAttrs` — <5 Ko

---

## Syntaxe

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

### Layout et slot

```webc
layout MainLayout {
    nav {
        link to="/" { "Accueil" }
        link to="/about" { "À propos" }
    }
    main { slot content }
}
```

### Page

```webc
page "home" {
    h1 "Bienvenue !"
    p "Compteur : {count}"
    button on:click={count += 1} { "Incrémenter" }
}
```

### Composant avec état

```webc
component Counter {
    state {
        count: Number = 0
    }

    view {
        div {
            p "Valeur : {count}"
            button on:click={count += 1} { "+" }
            button on:click={count = max(0, count - 1)} { "−" }
        }
    }

    style {
        div  { display: flex; gap: 1rem; align-items: center; }
        button { padding: 0.25rem 0.75rem; }
    }
}
```

### Directives de contrôle

```webc
@if count > 0 {
    p "Positif"
} @else {
    p "Zéro ou négatif"
}

@for item in items {
    li "{item}"
}
```

### Interpolation et contenu mixte

```webc
p "Résultat : {a + b}"
p { "Bonjour " strong { "le monde" } " !" }
div class={dynamicClass} { "contenu" }
```

---

## Installation

**Prérequis :** Rust 1.70+ avec Cargo

```bash
git clone https://github.com/PrinMeshia/Webcore.git
cd Webcore/webcore-compiler
cargo build --release
```

### Compiler un projet

```bash
./target/release/webc build --input examples/basic.webc --out dist
```

### Serveur de développement

```bash
./target/release/webc dev --input examples/basic.webc --out dist --port 3000
```

---

## Architecture

```
fichier.webc
    └── Parser Pest
           └── AST (apps · layouts · pages · composants)
                  ├── codegen_html.rs  →  HTML sémantique
                  ├── codegen_css.rs   →  CSS scopé (data-v)
                  └── codegen_js.rs    →  Runtime ES2022+
```

**Runtime JS généré (extrait) :**

```js
class State { #d = new Map(); #l = new Map();
  set(k, v) { this.#d.set(k, v); this.#l.get(k)?.forEach(f => f(v)) }
  get(k)    { return this.#d.get(k) }
  on(k, f)  { (this.#l.get(k) ?? this.#l.set(k, []).get(k)).push(f) }
}
const S = new State();
```

---

## Structure du projet

```
Webcore/
├── webcore-compiler/
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs           # CLI : build, dev
│       ├── grammar.pest      # Grammaire PEG
│       ├── parser.rs         # Pest → AST
│       ├── ast.rs            # Types AST
│       ├── errors.rs         # Gestion d'erreurs
│       ├── css_processor.rs  # Post-traitement CSS
│       ├── theme.rs          # Support thème
│       └── codegen/
│           ├── codegen_html.rs
│           ├── codegen_css.rs
│           └── codegen_js.rs
└── .github/
    └── workflows/
        └── ci.yml
```

---

## Développement

```bash
cd webcore-compiler

# Tests
cargo test

# Lint
cargo clippy -- -D warnings

# Format
cargo fmt
```

---

## Contribution

1. Fork le projet
2. Crée une branche : `git checkout -b feature/ma-fonctionnalite`
3. Commit : `git commit -m 'feat: description'`
4. Push : `git push origin feature/ma-fonctionnalite`
5. Ouvre une Pull Request

**Workflow pour ajouter une fonctionnalité :**

1. Modifier la grammaire → `grammar.pest`
2. Étendre l'AST → `ast.rs`
3. Mettre à jour le parser → `parser.rs`
4. Adapter le codegen → `codegen/`
5. Ajouter un test unitaire

---

## Roadmap

### ✅ v0.2.0 — Livré

| Fonctionnalité | Détail |
|---|---|
| Parser PEG | Grammaire Pest complète pour `.webc` |
| AST | Apps · layouts · pages · composants · state · view · style · props |
| Codegen HTML | Génération sémantique avec scoping CSS (`data-v`) |
| Codegen CSS | Isolation par composant, hash FNV-1a déterministe |
| Codegen JS | Runtime ES2022+ : `State`, `evalCond`, `bindIf/For/Attrs` |
| `@if` / `@for` | Directives réactives avec binding DOM |
| Interpolation d'expressions | `{count + 1}`, `{max(a, b)}` dans les chaînes |
| Contenu mixte | Texte et éléments dans le même bloc |
| Attributs dynamiques | `class={expr}` → binding runtime |
| Routage SPA | History API + `nav()` sans rechargement |
| Handlers d'événements | `on:click`, `on:submit`, `on:change`, `on:input` |
| CI/CD | GitHub Actions : fmt · test · clippy |

---

### ✅ Phase 1 — Stabilisation (complète)

- [x] Messages d'erreurs avec numéros de ligne (format Pest `Display` : fichier · ligne · col · contexte)
- [x] Tests golden (8 tests pipeline complet : parse → HTML/CSS/JS)
- [x] Props inter-composants (`props { name: String }` → substitution statique au codegen)
- [x] Minification CSS (LightningCSS — déjà câblé, activé en mode `prod`)
- [x] Minification JS (strip commentaires + join lignes, activé en mode `prod`)
- [x] Licence (MIT)

---

### ✅ Phase 2 — Expérience développeur (complète)

- [x] `webc new <nom>` — scaffolding complet (webc.toml, theme.toml, layouts, pages, Counter, public/)
- [x] Exemples d'applications (`examples/counter`, `examples/todo`, `examples/blog`)
- [x] Spécification du langage (`docs/spec.md`) — référence complète, 15 sections
- [x] Vrai hot reload via WebSockets — connexion persistante sur `port+1`, rechargement instantané, reconnexion auto
- [x] Extension VS Code (`editors/vscode/`) — coloration syntaxique TextMate pour `.webc`

---

### 🔄 Phase 3 — Vision long terme (en cours)

- [x] Store global partagé entre composants — bloc `store { ... }` + accès `$store.varName` dans tout le projet
- [x] Validation de formulaires déclarative — `validate:required/minlength/maxlength/email/pattern` + `@error "field" { }`
- [x] SSG — Static Site Generation (pré-rendu des valeurs initiales, `display` initial sur `@if`/`@else`)
- [x] Internationalisation intégrée (i18n) — `locales/*.toml` + `t("key")` + `setLocale()` réactif
- [x] Support WebAssembly (logique métier Rust → WASM) — détection `wasm/Cargo.toml`, `wasm-pack build`, loader async `globalThis.wasm`

---

## Changelog

Voir [CHANGELOG.md](./CHANGELOG.md).

---

## Remerciements

- [Pest](https://pest.rs/) — parser PEG en Rust
- [Clap](https://clap.rs/) — CLI
- La communauté Rust
