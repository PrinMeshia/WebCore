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
| **Version** | 2.0.0 |
| **Statut** | Stable |
| **Compilateur** | Rust + Pest PEG parser |
| **Tests** | 105 tests unitaires |
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
- **`@switch` / `@case` / `@default`** : directive multi-branches compilée en chaîne `@if`/`@else` — aucun overhead runtime
- **`bind:` two-way binding** : `bind:value={x}` et `bind:checked={x}` expandés en attribut + handler `on:input`/`on:change`
- **`@for item, i in items`** : accès à l'index courant dans les boucles via la seconde variable
- **`webc check`** : valide la syntaxe et les références (routes, composants, props) sans générer de fichiers
- **URLs propres** : les pages sont servies sans extension (ex. `/about` au lieu de `/about.html`)
- **`dist/assets/`** : JS, CSS et assets publics isolés dans un sous-dossier dédié
- **Arborescence du build** : résumé `dist/` avec tailles de fichiers affiché après chaque build
- **`http { }` — fetch déclaratif** : `get: "/url"  into: var` dans un composant ; `loading` et `error` auto-injectés et réactifs ; réponse JSON automatiquement parsée
- **`head { }` — titre et meta par page** : `title "..."` et `meta name="..."` par page ; override le titre global de `webc.toml`
- **`$query.` — query string params** : `{$query.search}`, `{$query.page}` — accès aux paramètres d'URL ; tree-shaké si inutilisé
- **`class:active={expr}` — classes CSS conditionnelles** : binding conditionnel via `bindAttrs()` ; plusieurs `class:` par élément ; tree-shaké
- **`on:event|debounce` — handler debouncé** : `on:input|debounce={expr}` — se déclenche après 300 ms d'inactivité ; fonctionne avec tout type d'événement
- **`ref:name=true` — références DOM directes** : `input ref:name=true` enregistre l'élément dans `refs['name']` via `DOMContentLoaded` ; accès direct sans `querySelector` ; utile pour la gestion du focus ; tree-shaké
- **`style:prop={expr}` — styles inline dynamiques** : `div style:color={myColor}` → `el.style.setProperty('color', ...)` via `bindAttrs()` ; peut coexister avec `style="..."` et `class:` ; tree-shaké
- **Contenu par défaut des slots** : `slot sidebar { p "Contenu par défaut" }` dans un layout — utilisé si la page ne remplit pas le slot ; les slots non remplis étaient précédemment supprimés silencieusement
- **`webc:transition="name"` — animations CSS** : `div webc:transition="fade" { ... }` — transitions intégrées `fade` et `slide` sur les blocs `@if` ; CSS injecté automatiquement ; tree-shaké
- **`webc:img` — images optimisées** : `img webc:img src="/hero.png" alt="Hero"` injecte `loading="lazy"`, `decoding="async"` et les dimensions (`width`/`height`) lues dans `public/` à la compilation ; prévient le layout shift (CLS) ; avertissement `warning[a11y]` si `alt` est absent ; transformation purement compile-time, zéro JS émis
- **Fingerprinting des images** : chaque image dans `public/` reçoit un hash de contenu FNV-1a 32 bits à `webc build` (`logo.png` → `logo.a3f9c1b2.png`) ; toutes les références HTML et CSS mises à jour automatiquement ; cache-busting parfait sans configuration
- **Signaux réactifs fins (`$effect`)** : `$effect(fn)` remplace `VARS.forEach(v=>S.on(v,fn))` — le tracking des dépendances est automatique ; les composants ne se re-rendent que lorsque leurs dépendances réelles changent (v2.0.0, **rupture**)
- **HMR** : `webc serve` surveille les fichiers et recharge le navigateur automatiquement via WebSocket — aucune configuration requise (v2.0.0)
- **Sécurité — path traversal** : `resolve_safe_path()` utilise `fs::canonicalize()` + `starts_with(dist_root)` ; les URLs qui sortent de `dist/` retournent 403 (v2.0.0)
- **Détection de cycles** : `webc check` détecte les références circulaires entre composants et rapporte le cycle complet (v2.0.0)
- **CSS nesting** : `&:hover { }`, `& > span { }`, `&::before { }` valides dans les blocs `style {}` ; aplatis en CSS scopé valide à l'émission (v2.0.0)
- **Agrégation des erreurs** : `webc build` collecte TOUTES les erreurs et les rapporte en une seule passe, comme le compilateur Rust (v2.0.0)
- **Rapport d'analyse du bundle** : après un `webc build` réussi, un tableau affiche les fonctionnalités runtime incluses vs tree-shaquées avec leurs tailles estimées (v2.0.0)

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
        @if loading { p "Chargement…" }
        @if error   { p "Erreur : {error}" }
        @for post in posts { li "{post.title}" }
    }
}
```

`loading` et `error` sont **auto-injectés** — pas besoin de les déclarer dans `state`.

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

---

## Architecture

```
fichier.webc
    └── Parser Pest
           └── AST (apps · layouts · pages · composants)
                  ├── codegen/css.rs   →  CSS scopé (data-v)
                  ├── codegen/html/    →  HTML sémantique
                  └── codegen/js/      →  Runtime ES2022+ (signaux $effect)
```

**Runtime JS généré — signaux à grain fin (extrait v2.0) :**

```js
let __wcfx = null;                         // effet en cours d'exécution
class State {
  #d = new Map(); #l = new Map(); #s = new Map();   // données · listeners · deps
  get(k) {                                 // track auto de la dépendance
    if (__wcfx) (this.#s.get(k) ?? this.#s.set(k, new Set()).get(k)).add(__wcfx);
    return this.#d.get(k);
  }
  set(k, v) {                              // re-exécute uniquement les effets concernés
    if (Object.is(this.#d.get(k), v)) return;
    this.#d.set(k, v);
    [...(this.#s.get(k) ?? [])].forEach(f => f());
  }
}
function $effect(fn) {                      // remplace VARS.forEach(v => S.on(v, fn))
  const r = () => { const p = __wcfx; __wcfx = r; try { fn() } finally { __wcfx = p } };
  r();
}
```

---

## Structure du projet

```
Webcore/
├── webcore-compiler/
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs              # Point d'entrée CLI
│       ├── grammar.pest         # Grammaire PEG
│       ├── core/                # Primitives compilateur (sans I/O)
│       │   ├── ast.rs           #   Types AST
│       │   ├── error.rs         #   Types d'erreur (CompileError)
│       │   ├── ssg.rs           #   Pré-rendu SSG
│       │   ├── theme.rs         #   Support thème
│       │   ├── utils.rs         #   Helpers partagés (html_escape…)
│       │   └── css_processor.rs #   Post-traitement CSS
│       ├── parser/              # Pest → AST
│       │   ├── declarations.rs  #   app · layout · page · component · store
│       │   ├── directives.rs    #   @for · @if · @switch · @error
│       │   └── elements.rs      #   tags · composants · attributs
│       ├── codegen/
│       │   ├── attr_names.rs    #   Constantes data-webcore-*
│       │   ├── css.rs           #   CSS scopé (data-v) + nesting
│       │   ├── html/            #   HTML sémantique
│       │   │   ├── attrs.rs     #     on: · class: · style: · ref: · validate:
│       │   │   ├── props.rs     #     substitution de props
│       │   │   └── component.rs #     rendu des composants
│       │   └── js/              #   Runtime ES2022+ (signaux)
│       │       ├── runtime.rs   #     State · $effect · evalCond
│       │       ├── events.rs    #     handlers d'événements
│       │       └── dom.rs       #     init DOM · HTTP · tree-shaking
│       ├── cli/                 # Pipeline CLI
│       │   ├── build.rs         #   webc build (+ bundle analysis)
│       │   ├── serve.rs         #   webc serve (HMR via WebSocket)
│       │   ├── check.rs         #   webc check (cycles · types)
│       │   └── assets.rs        #   fingerprinting · copie · rewrite
│       └── tests/               # Tests golden par domaine (html · js · css…)
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

### ✅ v1.2.0 — DX & directives (complète)

- [x] **`@switch` / `@case` / `@default`** — directive multi-branches compilée en chaîne `@if`/`@else` au parsing ; aucun overhead runtime
- [x] **`bind:` two-way binding** — `bind:value={x}` / `bind:checked={x}` : attribut + handler générés automatiquement par `expand_bind_attrs()`
- [x] **`@for item, i in items`** — index courant accessible dans la boucle ; `data-webcore-for-index` sur le `<template>` ; `fillItem` injecte la valeur
- [x] **`webc check`** — validation CLI sans génération de fichiers : parse + cohérence routes/composants/props
- [x] **URLs propres** — `slug/index.html` au lieu de `slug.html` ; le serveur dev résout `/about` → `dist/about/index.html`
- [x] **`dist/assets/`** — JS/CSS/assets publics dans un sous-dossier, HTML à la racine ; chemins absolus `/assets/`
- [x] **Arborescence du build** — résumé `dist/` avec tailles de fichiers après chaque `webc build`
- [x] **CSS public minifié** — `public/*.css` traités par LightningCSS en mode `prod`

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

### ✅ v1.3.0 — HTTP, head, query params, classes conditionnelles, debounce (complète)

- [x] **`http { }` — fetch déclaratif** — `get: "/url"  into: var` dans un composant ; `loading`/`error` auto-injectés et réactifs ; réponse JSON parsée automatiquement
- [x] **`head { }` — personnalisation du `<head>`** — `title "..."` et `meta name="..."` par page ; override le titre global de `webc.toml`
- [x] **`$query.` — query string params** — `{$query.search}`, `{$query.page}` ; tree-shaké si inutilisé
- [x] **`class:name={expr}` — classes conditionnelles** — `class:active={isOpen}` ; `bindAttrs()` gère le toggle ; tree-shaké
- [x] **`on:event|debounce`** — handler debouncé (300 ms par défaut) ; fonctionne avec tout type d'événement

---

### ✅ v1.4.0 — Références DOM, styles dynamiques, slot default, transitions (complète)

- [x] **`ref:name=true` — références DOM directes** — `input ref:name=true` émet `data-webcore-ref="name"` ; `refs['name']` accessible après `DOMContentLoaded` ; accès direct sans `querySelector` ; tree-shaké via `has_refs`
- [x] **`style:prop={expr}` — styles inline dynamiques** — `div style:color={myColor}` → `el.style.setProperty('color', ...)` via `bindAttrs()` ; peut coexister avec `style="..."` et `class:` ; tree-shaké via `has_style_binding`
- [x] **Contenu par défaut des slots** — `slot sidebar { p "Contenu par défaut" }` dans un layout ; utilisé si la page ne remplit pas le slot ; les slots non remplis étaient précédemment supprimés silencieusement
- [x] **`webc:transition="name"` — animations CSS** — `div webc:transition="fade" { ... }` ; transitions intégrées `fade` et `slide` sur les blocs `@if` ; CSS injecté automatiquement ; tree-shaké via `has_transition`

---

### ✅ v1.5.0 — Images optimisées et fingerprinting (complète)

- [x] **`webc:img` — images optimisées** — `img webc:img src="/hero.png" alt="Hero"` compile vers `<img src="/assets/hero.png" loading="lazy" decoding="async" width="1200" height="630" alt="Hero">` ; dimensions lues depuis `public/` à la compilation (prévient le CLS) ; avertissement `warning[a11y]` si `alt` est absent ; `webc:img` absent du HTML généré ; zéro JS émis ; crate `imagesize`
- [x] **Fingerprinting des images** — chaque image dans `public/` reçoit un hash FNV-1a 32 bits à `webc build` (`logo.png` → `logo.a3f9c1b2.png`) ; extensions : `.png`, `.jpg`, `.jpeg`, `.gif`, `.webp`, `.svg`, `.ico`, `.avif` ; toutes les références HTML et CSS mises à jour automatiquement ; cache-busting parfait, aucune configuration requise

---

## Changelog

Voir [CHANGELOG.md](./CHANGELOG.md).

---

## Remerciements

- [Pest](https://pest.rs/) — parser PEG en Rust
- [Clap](https://clap.rs/) — CLI
- La communauté Rust
