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
| **Version** | 2.6.0 |
| **Statut** | Stable |
| **Compilateur** | Rust + Pest PEG parser |
| **Tests** | 147 tests unitaires |
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
- **`$watch varName => { body }`** : observe les changements d'état sans effet DOM direct ; émet `S.on('varName', varName => { body })` dans `DOMContentLoaded` ; idéal pour analytics, logs ou synchronisation (v2.1.0)
- **`@for N..M` — plage numérique** : `@for i in 0..5 { }` itère `i` de 0 à 4 ; émet `data-webcore-for-range` ; le runtime génère le tableau sans donnée d'état ; tree-shaké (v2.1.0)
- **Imports de données build-time** : `import posts from "data/posts.json"` — fichiers JSON/TOML chargés à la compilation et injectés comme `S.setQ(name, data)` ; validation du chemin via `canonicalize()` (v2.1.0)
- **`on:click` avec objets imbriqués** : `on:click={handler({key: val})}` — accolades imbriquées arbitrairement dans les expressions d'événements via `expr_brace_seq` récursif (v2.1.0)
- **Expressions SSG étendues** : `eval_expr_with_locale` supporte `.length`, `.toUpperCase()`, `.toLowerCase()`, `.trim()` sur les variables d'état — élimine les valeurs vides au pré-rendu (v2.1.0)
- **Validation des props à la compilation** : `warning[props]: component 'X' received unknown prop 'y'` émis sur stderr si un composant reçoit une prop non déclarée ; la compilation continue (v2.1.0)
- **`@keyframes` dans `style {}`** : blocs `@keyframes` supportés dans les composants ; émis globaux (non scopés) pour être référencés par la propriété `animation:` ; parser, AST et codegen CSS mis à jour (v2.2.0)
- **`<script defer>` + `<link rel="preload">`** : le script runtime ne bloque plus le parsing HTML ; `<link rel="preload" as="script">` parallélise le téléchargement dès le `<head>` (v2.2.0)
- **Hash CSS** : `theme.css` reçoit `?v=<hash>` comme `webcore.js` — cache-busting automatique à chaque modification (v2.2.0)
- **Minification HTML (prod)** : commentaires et espaces inter-balises supprimés en mode `prod` ; réduit la taille des pages distribuées (v2.2.0)
- **Élision du scope CSS** : composants sans bloc `style {}` n'émettent plus `data-v="..."` sur leurs éléments — HTML plus propre et plus léger (v2.2.0)
- **Avertissement ReDoS** : `validate:pattern` avec quantificateurs imbriqués (`)+`, `)*`) émet `warning[security]` à la compilation — prévient le backtracking catastrophique (v2.2.0)
- **SRI — Subresource Integrity** : en mode `prod`, `<script>` et `<link rel="stylesheet">` reçoivent `integrity="sha256-..."` + `crossorigin="anonymous"` ; hash SHA-256 calculé par le compilateur (v2.3.0)
- **Élision JS zéro** : pages purement statiques (sans état, sans boucle, sans événements) n'émettent plus `<script defer>` ni `<link rel="preload">` — zéro JS pour les pages de contenu (v2.3.0)
- **Limite d'imbrication** : le parser rejette tout document dont les éléments dépassent 128 niveaux d'imbrication — protège contre les "nesting bombs" (v2.3.0)
- **Escape JS des URLs de navigation** : apostrophes et backslashes dans les chemins `<a onclick="webcore_navigate(...)">` échappés — empêche l'injection JS (v2.3.0)
- **Critical CSS inline** : en mode `prod`, le CSS de chaque page (global + composants utilisés) est inliné dans `<style>` ; `theme.css` chargé en différé (`media="print"` + `data-webcore-defer`) — zéro CSS render-blocking (v2.4.0)
- **Collections SSG** : `"/post/:slug": PostPage each posts` — une page statique générée par élément d'un import de données ; `{$route.slug}` pré-rendu ; chemins de sortie validés contre le path traversal (v2.4.0)
- **CSP stricte — event delegation** : tous les `onclick=`/`onsubmit=` inline remplacés par `data-webcore-e="<type>"` + listener délégué `D(t,p)` ; SPA links via `data-webcore-nav` ; CSS déféré via `data-webcore-defer` + `DOMContentLoaded` ; option `csp = true` dans `webc.toml` émet le meta `Content-Security-Policy` (v2.5.0)
- **Corrections v2.5.1** : escape de `</style>` dans le CSS critique inline ; inclusion de `webcore.js` sur les pages zero-JS avec CSS différé ; `.length` correct sur les arrays contenant des virgules dans les chaînes et sur les chaînes Unicode (v2.5.1)
- **Erreurs de parsing enrichies v2.5.2** : format `error[parse]: fichier:ligne:col` + ligne source + caret `^` + hints contextuels ; couleurs ANSI conditionnelles ; chemin de fichier propagé depuis tous les points de chargement (v2.5.2)
- **Fragment shorthand `<>...</>`** : groupe d'éléments sans balise wrapper — compilé en nœuds inline ; supporte directives, composants et imbrication arbitraire (v2.6.0)
- **Modificateurs d'événements** : `on:click|stop`, `on:click|prevent`, `on:click|once`, `on:click|self` — encodés dans `data-webcore-e` ; gérés par le listener délégué sans JS inline ; combinables (v2.6.0)
- **Valeurs de props par défaut** : `props { label: String = "Défaut" }` — la valeur par défaut est injectée si la prop est omise à l'instanciation (v2.6.0)
- **`webc watch`** : surveille les fichiers sources et rebuilde automatiquement à chaque modification sans serveur de développement ; debounce 200 ms (v2.6.0)

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
2. Crée une branche : `git checkout -b feature/ma-fonctionnalite`
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
