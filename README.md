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
| **Version** | 2.10.1 (publiée) |
| **Statut** | Stable |
| **Compilateur** | Rust + Pest PEG parser |
| **Tests** | 182 tests (unitaires, golden, intégration, perf) |
| **CI** | GitHub Actions (fmt · test · clippy) |

---

## Fonctionnalités

> Le détail de chaque fonctionnalité, version par version, est dans le [CHANGELOG](./CHANGELOG.md) ;
> la référence complète de la syntaxe dans [docs/spec.md](./docs/spec.md).

### Langage & templating

- **Blocs déclaratifs** : `app` (routes, layout, thème), `layout`, `page`, `component` (props · state · computed · view · style), `store` global partagé (`$store.x`)
- **Interpolation d'expressions** : `{count}`, `{count + 1}`, `{max(a, b)}` — y compris dans les chaînes et les attributs
- **Directives** : `@if` / `@else`, `@switch` / `@case` / `@default`, `@for` (avec `key=` pour le DOM diffing, index `item, i`, plages `0..5`), `@error` pour les messages de validation, `@loading` / `@catch` (sucre pour `@if loading` / `@if error`), `@defer` (affichage différé après DOMContentLoaded)
- **Sucre syntaxique props** : `<Component {count}>` ≡ `<Component count={count}>` ; `<div ...attrs>` pour spread d'attributs
- **Fragments** `<>...</>`, contenu mixte texte/éléments, slots nommés multi-zones avec contenu par défaut
- **Props** : statiques, réactives (`value={expr}`), valeurs par défaut (`label: String = "Défaut"`), validation à la compilation (warning sur prop inconnue)
- **Imports de données build-time** : `import posts from "data/posts.json"` (JSON/TOML)

### Réactivité & runtime

- **Signaux fins** : `$effect` avec tracking automatique des dépendances — un composant ne se re-rend que si une dépendance réellement lue change
- **Runtime minimal** (~2-5 Ko) **tree-shaké par fonctionnalité** : ce que le document n'utilise pas n'est pas émis
- **Événements** : `on:click`, `on:submit`, `on:input`… avec modificateurs `|stop` `|prevent` `|once` `|self`, debounce (`on:input|debounce`), handlers multi-instructions, objets imbriqués
- **`bind:value` / `bind:checked`** two-way, **`ref:name`** pour l'accès DOM direct, **`$watch`** pour observer sans effet DOM, **`emit()`** inter-composants, hooks `on:mount` / `on:destroy`
- **État dérivé** : `computed { fullName = firstName + " " + lastName }`
- **`http {}`** : fetch déclaratif (`get:` / `into:`) avec `loading` et `error` auto-injectés, `@loading`/`@catch` comme raccourcis
- **Méthodes `List`** : `items.push(val)`, `items.remove(i)`, `items.clear()` compilent en mutations réactives `S.set()` / `STORE.set()`

### Styles

- **CSS scopé par composant** (`data-v`, hash déterministe) — élidé pour les composants sans styles
- **Nesting** `&:hover`, `@media` et `@keyframes` dans les blocs `style {}`, sélecteurs multi-éléments
- **Bindings** : classes conditionnelles `class:active={expr}`, styles inline dynamiques `style:color={expr}`
- **Thème** centralisé (`theme.toml` → variables CSS), transitions intégrées `webc:transition="fade|slide"`

### Routage & pages

- **SPA** : History API, navigation sans rechargement, routes paramétrées (`/post/:slug` → `{$route.slug}`), query string (`{$query.page}`)
- **`head {}`** par page (titre, meta), URLs propres (`/about` sans `.html`)
- **Collections SSG** : `"/post/:slug": PostPage each posts` — une page statique par élément de données

### Formulaires & i18n

- **Validation déclarative** : `validate:required/email/minlength/maxlength/pattern` + blocs `@error "champ" {}` — au blur, à la saisie et au submit
- **i18n** : `locales/*.toml`, `t("key")` avec paramètres (`{{0}}`) et pluriels (`_one`/`_other`, `{{count}}`), `setLocale()` réactif

### Performance

- **SSG** : interpolations et branches `@if` pré-rendues à la compilation — zéro flash de contenu
- **Pages statiques sans JS** : aucun script émis si la page n'en a pas besoin
- **Mode prod** : minification HTML/CSS/JS, critical CSS inliné (zéro CSS render-blocking), `<script defer>` + preload
- **Cache-busting** : nom de fichier content-hash (`webcore.<hash8>.js`) + fingerprinting des images (`logo.a3f9c1b2.png`), builds **100 % déterministes**
- **`webc:img`** : `loading="lazy"`, `decoding="async"` et dimensions injectées à la compilation (anti-CLS)

### Sécurité

- **CSP stricte** : zéro JS inline — event delegation via `data-webcore-e`, option `csp = true` qui émet la meta Content-Security-Policy
- **SRI** : `integrity="sha256-…"` sur scripts et styles en prod
- **Serveur dev** : protection path-traversal (canonicalisation + 403)
- **Compilation** : échappement HTML/JS systématique, warning ReDoS sur `validate:pattern`, limite d'imbrication (anti nesting-bomb)

### Outillage & DX

- **CLI complète** : `webc new` · `build` · `dev` (HMR via WebSocket) · `watch` · `check` · `fmt` (formateur idempotent) · `lsp` (serveur LSP 3.17 sur stdin/stdout — hover, complétion, go-to-definition, **rename**)
- **Erreurs façon rustc** : ligne source + caret `^` + hints contextuels, toutes les erreurs agrégées en une passe
- **Runtime ES2025** : `RegExp.escape()` dans `evalCond`, `Promise.try()` pour les blocs `http {}`, strip prod des attributs `data-webcore-*` après binding
- **`webc check`** : valide routes, composants, props et détecte les références circulaires sans rien générer
- **Rapport de build** : arborescence `dist/` + analyse du bundle (fonctionnalités incluses vs tree-shakées)
- **WASM** : détection `wasm/Cargo.toml`, build `wasm-pack`, loader async `globalThis.wasm`
- **[Extension VS Code](./editors/vscode)** : coloration, snippets, formatage via `webc fmt`

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

### Layout et slots nommés

```webc
layout MainLayout {
    nav {
        link to="/" { "Accueil" }
        link to="/about" { "À propos" }
    }
    main { slot content }
}

// Layout multi-zones (v0.8.0)
layout DashLayout {
    header { slot header }
    aside  { slot sidebar }
    main   { slot content }
}
```

```webc
// Page qui remplit les slots nommés
page "dashboard" {
    slot header  { h1 "Dashboard" }
    slot sidebar { nav { link to="/" "Accueil" } }
    p "Contenu principal"   // → slot content par défaut
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
        div    { display: flex; gap: 1rem; align-items: center; }
        button { padding: 0.25rem 0.75rem; }
        @media (max-width: 480px) {
            div { flex-direction: column; }
        }
    }
    view { p "Bonjour {nomComplet}" }
}

// Événements inter-composants
component Notificateur {
    view { button on:click={emit("ping", count)} { "Ping" } }
}

page "home" {
    Notificateur on:ping={count += 1} {}
}
```

### Props réactives (v0.8.0)

```webc
// Composant avec prop
component Badge {
    props { label: String, color: String }
    view { span class={color} "Statut : {label}" }
}

// Utilisation — prop statique ou dynamique
page "home" {
    Badge label="Actif" color="green" {}
    Badge label={statusMsg} color={statusColor} {}
}
```

### État dérivé, lifecycle et événements inter-composants (v0.9.0)

```webc
component NomComplet {
    state {
        prenom: String = "Jean"
        nom: String = "Dupont"
    }
    computed { nomComplet = prenom + " " + nom }
    on:mount {
        prenom = "Marie"
    }
    view { p "Bonjour {nomComplet}" }
}

// Événements inter-composants
component Notificateur {
    view { button on:click={emit("ping", count)} { "Ping" } }
}

page "home" {
    Notificateur on:ping={count += 1} {}
}
```

### Directives de contrôle

```webc
@if count > 0 {
    p "Positif"
} @else {
    p "Zéro ou négatif"
}

// Sans key — re-rendu complet à chaque changement
@for item in items {
    li "{item}"
}

// Avec key — DOM diffing (v1.1.0)
@for post key=post.id in posts {
    article "{post.title}"
}

// Avec index — accès au rang courant (v1.2.0)
@for item, i in items {
    li "{i}. {item}"
}

// Multi-branches (v1.2.0)
@switch status {
    @case "active"   { span class="green" "Active" }
    @case "pending"  { span class="yellow" "Pending" }
    @default         { span class="gray" "Unknown" }
}
```

### `bind:` two-way binding (v1.2.0)

```webc
// Synchronise automatiquement la valeur dans les deux sens
input bind:value={name}    // ≡ value={name} + on:input={name = event.target.value}
input bind:checked={agree} // ≡ checked={agree} + on:change={agree = event.target.checked}
```

### Routes paramétrées (v1.1.0)

```webc
app MyApp {
    routes {
        "/":           HomePage
        "/post/:slug": PostPage
    }
}

component PostPage {
    view { h1 "Article : {$route.slug}" }
}
```

### i18n avec paramètres et pluriel (v1.1.0)

```toml
# locales/fr.toml
items_one   = "{{count}} élément"
items_other = "{{count}} éléments"
greeting    = "Bonjour, {{0}} !"
```

```webc
p "{t(\"items\", count)}"        // "3 éléments"
p "{t(\"greeting\", username)}"  // "Bonjour, Alice !"
```

### Interpolation et contenu mixte

```webc
p "Résultat : {a + b}"
p { "Bonjour " strong { "le monde" } " !" }
div class={dynamicClass} { "contenu" }
```

### Bloc `http { }` — fetch déclaratif (v1.3.0)

```webc
component Posts {
    state { posts: List = null }
    http { get: "/api/posts"  into: posts }
    view {
        @loading { p "Chargement…" }   // sucre pour @if loading (v2.8.0)
        @catch   { p "Erreur : {error}" }  // sucre pour @if error (v2.8.0)
        @for post in posts { li "{post.title}" }
    }
}
```

`loading` et `error` sont **auto-injectés** — pas besoin de les déclarer dans `state`.

### Méthodes réactives sur `List` (v2.8.0)

```webc
component TodoList {
    state { todos: List = null }
    view {
        input ref:draft=true placeholder="Nouvelle tâche"
        button on:click={todos.push(refs.draft.value)} { "Ajouter" }
        @for todo, i in todos {
            li { "{todo}" button on:click={todos.remove(i)} { "×" } }
        }
        button on:click={todos.clear()} { "Tout effacer" }
    }
}
```

### Classes conditionnelles et debounce (v1.3.0)

```webc
// class:name={expr} — active/désactive la classe selon l'expression
div class:active={isOpen} class:hidden={!visible} { "contenu" }

// on:event|debounce — se déclenche après 300 ms d'inactivité
input on:input|debounce={search = event.target.value}

// $query. — accès aux paramètres d'URL
p "Recherche : {$query.search}"
p "Page : {$query.page}"
```

---

## Installation

### Binaires précompilés

Chaque release publie des binaires `webc` pour Linux, macOS (Intel et
Apple Silicon) et Windows : téléchargez l'archive de votre plateforme
depuis la [page des releases](https://github.com/PrinMeshia/Webcore/releases),
extrayez `webc` et placez-le dans votre `PATH`.

### Depuis les sources

**Prérequis :** Rust 1.70+ avec Cargo

```bash
git clone https://github.com/PrinMeshia/Webcore.git
cd Webcore/webcore-compiler
cargo build --release
```

### Compiler un projet

```bash
# Depuis le répertoire d'un projet (là où se trouve webc.toml)
cd examples/counter
webc build
```

### Serveur de développement

```bash
cd examples/counter
webc dev
# Avec un port personnalisé
webc dev 3000
```

### Validation sans build

```bash
cd examples/counter
webc check
# → parse + valide routes, composants et types de props sans écrire de fichiers
```

### Rebuild automatique (sans serveur)

```bash
cd examples/counter
webc watch
# → rebuilde à chaque modification de fichier .webc ou de configuration
```

---

## Architecture

```
fichier.webc
    └── parser/                        # Pest → AST
           └── AST (apps · layouts · pages · composants)
                  ├── codegen/html/    →  HTML sémantique
                  ├── codegen/css.rs   →  CSS scopé (data-v)
                  └── codegen/js/      →  Runtime ES2022+
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
│       ├── main.rs            # Point d'entrée CLI
│       ├── grammar.pest       # Grammaire PEG
│       ├── parser/            # Pest → AST
│       │   ├── mod.rs
│       │   ├── declarations.rs
│       │   ├── directives.rs
│       │   └── elements.rs
│       ├── cli/               # Commandes build · serve · check
│       │   ├── build.rs · serve.rs · check.rs
│       │   └── config.rs · loader.rs · output.rs · assets.rs
│       ├── core/              # Types et logique métier
│       │   ├── ast.rs · error.rs · ssg.rs
│       │   └── css_processor.rs · theme.rs · utils.rs
│       └── codegen/           # Génération de code
│           ├── html/          # mod.rs · attrs.rs · analysis.rs · minify.rs · props.rs
│           ├── css.rs
│           └── js/            # mod.rs · js_runtime.rs · js_dom.rs · js_events.rs
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
2. Crée une branche depuis `develop` : `git checkout -b feature/ma-fonctionnalite origin/develop`
3. Commit : `git commit -m 'feat: description'`
4. Push : `git push origin feature/ma-fonctionnalite`
5. Ouvre une Pull Request

**Workflow pour ajouter une fonctionnalité :**

1. Modifier la grammaire → `grammar.pest`
2. Étendre l'AST → `core/ast.rs`
3. Mettre à jour le parser → `parser/`
4. Adapter le codegen → `codegen/`
5. Ajouter un test unitaire dans `src/tests/`

---

## Changelog

Voir [CHANGELOG.md](./CHANGELOG.md).

---

## Remerciements

- [Pest](https://pest.rs/) — parser PEG en Rust
- [Clap](https://clap.rs/) — CLI
- La communauté Rust
