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
| **Version** | 1.1.1 |
| **Statut** | Stable |
| **Compilateur** | Rust + Pest PEG parser |
| **Tests** | 76 tests unitaires |
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
- **`@media` dans `style { }`** : media queries responsives scopées directement dans les composants
- **Routage SPA** : History API + `nav()` sans rechargement de page
- **État réactif** : `class State { #d = new Map() }` ES2022 avec `S.get/set/on`
- **Props réactives** : `Component value={expr} />` — les props acceptent des expressions dynamiques
- **Named slots** : layouts multi-zones (`slot header`, `slot sidebar`, `slot content`)
- **Store global** : `store { ... }` partagé entre tous les composants via `$store.varName`
- **Validation de formulaires** : `validate:required/email/minlength` + `@error "field" { }`
- **SSG** : pré-rendu des valeurs initiales, élimination du flash de contenu
- **i18n** : `locales/*.toml` + `t("key")` + `setLocale()` réactif
- **WASM** : détection `wasm/Cargo.toml`, `wasm-pack build`, loader async `globalThis.wasm`
- **Runtime minimal** : `evalCond`, `bindIf`, `bindFor`, `bindAttrs` — <5 Ko
- **État dérivé** : `computed { fullName = firstName + " " + lastName }` — réévalué avant chaque bind
- **Lifecycle hooks** : `on:mount { }` / `on:destroy { }` — exécutés au `DOMContentLoaded` / avant navigation
- **Événements inter-composants** : `emit("event", data)` + `on:event={handler}` sur les appels de composants
- **Routes paramétrées** : `/post/:slug` avec accès via `{$route.slug}` dans les vues ; tree-shaqué si inutilisé
- **`@for` avec key** : `@for item key=item.id in items { }` active le DOM diffing par clé — patch minimal
- **i18n params et pluriel** : `t("items", count)` → `_one`/`_other` + `{{count}}` ; `t("greeting", name)` → `{{0}}`
- **Props composées** : `{step + 1}` et `class={color}` substitués même dans les expressions composites
- **Erreurs de parsing enrichies** : ligne source + caret `^` à la colonne fautive + hints contextuels
- **Handlers multi-instructions** : `on:click={a = 1; b = 2}` — plusieurs instructions séparées par `;` dans un même handler
- **Sélecteurs CSS multi-éléments** : `input, textarea { }` supporté dans les blocs `style { }`
- **Exemple formulaires** : `examples/forms/` — `SignupForm` et `ContactForm` avec validation complète

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

### ✅ Phase 3 — Vision long terme (complète)

- [x] Store global partagé entre composants — bloc `store { ... }` + accès `$store.varName` dans tout le projet
- [x] Validation de formulaires déclarative — `validate:required/minlength/maxlength/email/pattern` + `@error "field" { }`
- [x] SSG — Static Site Generation (pré-rendu des valeurs initiales, `display` initial sur `@if`/`@else`)
- [x] Internationalisation intégrée (i18n) — `locales/*.toml` + `t("key")` + `setLocale()` réactif
- [x] Support WebAssembly (logique métier Rust → WASM) — détection `wasm/Cargo.toml`, `wasm-pack build`, loader async `globalThis.wasm`

---

### ✅ v0.8.0 — Modèle de composant v2 (complète)

- [x] **Props réactives** — `Component value={expr} />` : les props acceptent des expressions dynamiques ; `{propName}` dans la vue devient un span réactif au lieu d'un texte figé
- [x] **Named slots** — layouts multi-zones (`slot header`, `slot sidebar`, `slot content`) ; pages fournissent le contenu via `slot header { ... }` ; résolution récursive à n'importe quelle profondeur
- [x] **`@media` dans `style { }`** — media queries responsives scopées par composant ; scoping `data-v` propagé à l'intérieur des blocs `@media`

---

### ✅ v0.9.0 — Modèle de composant v3 (complète)

- [x] **État dérivé** — `computed { fullName = firstName + " " + lastName }` ; réévalué via `rebindComputed()` avant chaque bind DOM ; `setQ` (setter silencieux) sur `State` pour éviter les boucles réactives
- [x] **Lifecycle hook `on:mount`** — bloc `on:mount { }` : code JS exécuté dans `DOMContentLoaded` après l'initialisation ; wrappé dans un IIFE pour l'isolation des variables locales
- [x] **Événements inter-composants** — `emit("event", data)` compilé vers `CustomEvent` ; `on:event={handler}` sur les appels de composants → `document.addEventListener` dans `DOMContentLoaded`

---

### ✅ v1.0.0 — Release stable (complète)

- [x] **`on:destroy { }`** — lifecycle hook symétrique à `on:mount` ; exécuté avant chaque navigation SPA et à `window.beforeunload`
- [x] **Tree-shaking du runtime** — `bindFor`, `bindIf`, `bindAttrs`, `validateField`/`bindValidation`, `nav`, `evalCond`, `VARS`/`STORE_VARS`, `COMPUTED`/`rebindComputed` sont émis uniquement si le document les utilise ; économies significatives pour les applications simples
- [x] **Site de documentation** — `examples/docs/` : site entièrement construit avec WebCore lui-même (4 pages, 2 composants, layout, styles)

---

### ✅ v1.1.0 — Routes, DX et polissage (complète)

- [x] **Routes paramétrées** — `/post/:slug`, `/user/:id` : `ROUTES[]` avec RegExp + `ROUTE_PARAMS` ; accès via `{$route.slug}` dans les vues ; tree-shaké si inutilisé
- [x] **`@for` avec key** — `@for item key=item.id in items { }` : DOM diffing par clé, patch minimal au lieu de re-rendu complet
- [x] **i18n params + pluralisation** — `t("key", n)` → clés `_one`/`_other` + `{{count}}` ; `t("key", val)` → `{{0}}`
- [x] **Props composées** — `{step + 1}`, `class={color}` : substitution word-boundary dans les expressions complexes et les attributs
- [x] **Erreurs de parsing enrichies** — ligne source + caret `^` à la colonne fautive + hints contextuels pour les erreurs fréquentes

---

### ✅ v1.1.1 — Corrections et polissage (complète)

- [x] **Validation de formulaires** — phase de capture + `stopImmediatePropagation()` garantit l'exécution avant `on:submit` ; blocs `@error` préservés via `firstElementChild`
- [x] **Handlers multi-instructions** — `on:click={a = 1; b = 2}` : chaque instruction est compilée indépendamment
- [x] **`on:mount` imbrication profonde** — grammaire Pest récursive pour les accolades JS à profondeur arbitraire
- [x] **`t()` dans `evalCond`** — passé explicitement aux `new Function()` ; variable locale renommée `_c` pour éviter le masquage
- [x] **Sélecteurs CSS multi-éléments** — `input, textarea { }` désormais valide dans les blocs `style { }`
- [x] **DOM diffing `@for` sans wrapper** — clé posée sur `firstElementChild`, éliminant les espaces parasites entre items
- [x] **Exemple `examples/forms/`** — `SignupForm` + `ContactForm` avec validation complète, computed, compteur de caractères

---

## Changelog

Voir [CHANGELOG.md](./CHANGELOG.md).

---

## Remerciements

- [Pest](https://pest.rs/) — parser PEG en Rust
- [Clap](https://clap.rs/) — CLI
- La communauté Rust
