# Changelog

Tous les changements notables sont documentés ici.
Format basé sur [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

---

## [0.7.0] — 2026-06-02

### Ajouts

- **WebAssembly (WASM)** — détection automatique de `wasm/Cargo.toml` ; invocation de `wasm-pack build --target web` au build ; loader asynchrone injecté dans le runtime JS : `const WASM={}; globalThis.wasm=WASM; (async()=>{try{...}catch(e){...}})()` — remplit `globalThis.wasm` avec toutes les exports du module et déclenche un rebind complet dès le chargement ; `webc new` crée le scaffold WASM (`wasm/Cargo.toml`, `wasm/src/lib.rs` avec exemple `wasm-bindgen`) ; `wasm_module` ajouté à `WebCoreDocument` ; 2 tests golden

---

## [0.6.0] — 2026-06-02

### Ajouts

- **Internationalisation (i18n)** — fichiers `locales/<code>.toml` (TOML plat `clé = "valeur"`) ; chargés au build dans `document.locales` ; runtime JS : `const LOCALES`, `let LOCALE`, `const t=k=>LOCALES[LOCALE]?.[k]??k`, `const setLocale=l=>{...}` (réactif : rebind de toutes les directives) ; `setLocale` exposé dans `globalThis` ; `locale` configurable dans `webc.toml` (`[app] locale = "fr"`, défaut = valeur de `lang`) ; SSG pré-rend `{t("key")}` avec la locale par défaut ; 3 tests golden

---

## [0.5.0] — 2026-06-02

### Ajouts

- **SSG (Static Site Generation)** — nouveau module `ssg.rs` : `build_initial_state` collecte les valeurs par défaut de tous les composants et du store ; `apply_ssg` post-traite le HTML généré pour (1) pré-remplir les `<span data-webcore-interpolation>` avec les valeurs initiales et (2) pré-définir `style="display:block/none"` sur les divs `@if`/`@else` selon l'état initial — élimine le flash de contenu incorrect au premier chargement ; compatible avec le runtime JS (`bindIf`/`bind` continuent à opérer normalement) ; `evalCond` simple supporte `>`, `<`, `>=`, `<=`, `==`, `!=` sur des variables numériques ; 9 tests unitaires internes + 2 golden tests

---

## [0.4.0] — 2026-06-02

### Ajouts

- **`webc new <nom>`** — commande de scaffolding : crée la structure complète d'un projet (webc.toml, theme.toml, layouts, pages, Counter.webc, public/)
- **Exemples** — trois projets d'exemple dans `examples/` : `counter`, `todo`, `blog`
- **Spec du langage** — référence complète dans `docs/spec.md` (syntaxe, directives, routage, thème, runtime JS)
- **Hot reload WebSocket** — remplacement du polling HTTP toutes les 500 ms par une connexion WebSocket persistante (`ws://localhost:{port+1}`) ; le serveur envoie `"reload"` après chaque rebuild, la page se recharge instantanément ; reconnexion automatique si la connexion est perdue
- **Extension VS Code** — `editors/vscode/` : coloration syntaxique complète pour `.webc` via TextMate grammar (`app`, `layout`, `page`, `component`, `@if`/`@else`/`@for`, HTML tags, attributs, interpolations `{expr}`, CSS scopé dans `style { }`, types, fonctions built-in)
- **Store global partagé** — bloc `store { varName: Type = valeur }` au niveau document ; variables référencées avec `$store.varName` dans les expressions et interpolations ; `STORE.set/get/on` dans le runtime JS ; `$store.var += 1` et `$store.var = val` compilés correctement dans les handlers ; `bindIf/bindFor/bind/bindAttrs` réactifs aux changements du store ; `@for item in $store.list` supporté
- **Validation de formulaires déclarative** — attributs `validate:required`, `validate:minlength`, `validate:maxlength`, `validate:email`, `validate:pattern` sur les inputs ; directive `@error "field" { }` pour l'affichage des erreurs ; validation au blur + soumission ; `validateField()` + `bindValidation()` injectés dans le runtime

---

## [0.3.0] — 2026-06-02

### Ajouts

- **Licence MIT** — fichier `LICENSE` + champ `license = "MIT"` dans `Cargo.toml`
- **Tests golden** — 8 tests de pipeline complet dans `src/tests.rs` (parse → HTML, CSS, JS, props)
- **Props inter-composants** — `substitute_props()` substitue statiquement les valeurs de props déclarées dans `props { ... }` lors du rendu ; les `{propName}` dans la vue deviennent des nœuds `Text` avant la génération HTML
- **Minification JS** — `minify_js()` dans `codegen_js.rs` : strip des commentaires `// ...` et join des lignes ; activé automatiquement en mode `prod`
- **Messages d'erreurs lisibles** — les erreurs de parse utilisent maintenant `Display` de Pest (fichier · ligne · colonne · contexte) au lieu du format `Debug`

### Corrections

- **Dépendances inutilisées** — `miette` et `thiserror` supprimés de `Cargo.toml` (plus référencés depuis la réécriture de `errors.rs`)
- Version bumped `0.1.0 → 0.2.0` (suite v0.2.0 déjà taguée comme release)

### Notes

- La minification CSS via LightningCSS était déjà implémentée depuis v0.2.0 ; elle est maintenant documentée comme fonctionnalité officielle de Phase 1

---

## [0.2.0] — 2026-06-01

### Ajouts

- **CI/CD** : GitHub Actions avec vérification du format (`cargo fmt`), tests (`cargo test`) et lint strict (`cargo clippy -- -D warnings`)
- **Directives réactives** : `@if condition { } @else { }` et `@for item in list { }` avec binding DOM au runtime (`bindIf`, `bindFor`)
- **Attributs dynamiques** : `attr={expr}` compilé en `data-webcore-bound` + `data-webcore-attr-{name}`, évalués au runtime via `bindAttrs()`
- **Interpolation d'expressions** : `{count + 1}`, `{max(a, b)}`, `{user.name}` dans les chaînes de caractères (anciennement limité aux identifiants simples)
- **Contenu mixte dans les tags** : texte et éléments enfants dans le même bloc (`p { "Hello " strong { "World" } "!" }`)
- **`evalCond`** : évaluation sécurisée des expressions via `Function()` avec substitution des variables d'état
- **`VARS`** : tableau des noms de variables d'état pour le suivi réactif au runtime
- **Préfixes d'ID par page** : `safe_id_prefix()` élimine les collisions d'ID de handlers entre pages dans le mode SPA (ex : `homebtn1`, `aboutbtn1` au lieu de `btn1` en double)
- **25 tests unitaires** couvrant parser, codegen HTML/CSS/JS et gestion d'erreurs

### Corrections

- **Hash déterministe** : remplacement de `DefaultHasher` (non-déterministe entre processus) par FNV-1a 32 bits pour les IDs de scope CSS (`data-v`)
- **Double `>`** dans la génération `@for` : la balise `<template ...>` émettait `> data-v="...">` au lieu de `data-v="..." >`
- **Attributs dynamiques cassés** : `attr="{}"` ne rendait plus l'expression — remplacé par le système `data-webcore-attr-*`
- **Tests stale** dans `codegen_js` : références à l'ancienne API `window.__webcore_state__` mises à jour vers `S.get/set`, `nav()`, `U.max`

### Refactoring

- **Suppression du dead code** : 6+ fonctions inutilisées supprimées (`generate_elements`, `generate_element`, `generate_elements_with_components`, etc.)
- **CSS** : import `DefaultHasher/Hash/Hasher` supprimé
- **Errors** : suppression de `WebCoreError`, `format_error`, imports `miette`/`thiserror`
- **Main** : suppression de `generate_index_html`, `handle_request`, import `WebCoreError`
- **Clippy** : correction de tous les avertissements en mode `-D warnings`

### Changements

- **Runtime JS** mis à niveau vers ES2022+ avec champs privés (`class State { #d = new Map() }`)
- **Codegen** découpé en fichiers dédiés : `codegen_html.rs`, `codegen_css.rs`, `codegen_js.rs`
- **`bind()`** utilise `evalCond` au lieu de `S.get` direct pour supporter les expressions complexes
- **`nav()`** appelle `bind(); bindIf(); bindFor(); bindAttrs()` après chaque navigation SPA
- **`DOMContentLoaded`** initialise toutes les directives réactives

---

## [0.1.0] — 2025 (MVP initial)

### Ajouts

- Parser Pest PEG pour les fichiers `.webc`
- AST structuré : apps, layouts, pages, composants (state, view, style, props)
- Génération HTML/CSS/JS basique à partir de l'AST
- CLI : `webc build` et `webc dev`
- Handlers d'événements HTML5 natifs (`on:click`, `on:submit`, `on:change`, `on:input`)
- Routage SPA avec History API
- State management réactif
- Serveur de développement avec hot reload (polling de version)
- CSS scopé par composant
