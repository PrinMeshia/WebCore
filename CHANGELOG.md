# Changelog

Tous les changements notables sont documentés ici.
Format basé sur [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

---

## [1.2.0] — 2026-06-05

### Ajouts

- **`@switch` / `@case` / `@default`** — nouvelle directive de contrôle multi-branches ; compilée en chaîne `@if`/`@else` au parsing, sans changement du codegen JS
- **`bind:` two-way binding** — `bind:value={x}` expande en `value={x}` + `on:input={x = event.target.value}` ; `bind:checked={x}` → `on:change={x = event.target.checked}` ; traitement en pré-passe dans `expand_bind_attrs()` avant la génération du tag
- **`@for item, i in items`** — accès à l'index courant dans les boucles ; `i` est disponible dans les interpolations et expressions imbriquées ; émet `data-webcore-for-index` sur le `<template>` ; `bindFor()` injecte la valeur d'index dans `fillItem`
- **`webc check`** — commande CLI : parse et valide les références (routes ↔ pages, composants instanciés, types de props) sans générer de fichiers ; rapporte les erreurs de cohérence avec fichier et ligne
- **URLs propres** — les pages sont générées dans `slug/index.html` au lieu de `slug.html` ; les liens SPA et le serveur dev résolvent correctement les chemins sans extension
- **`dist/assets/`** — JS, CSS et assets publics placés dans `dist/assets/` ; les HTML restent à la racine de `dist/` ; les chemins d'assets sont absolus (`/assets/theme.css`) pour les sous-répertoires
- **Arborescence du build** — `webc build` affiche un récapitulatif `dist/` avec tailles de fichiers et total
- **CSS public minifié** — les fichiers `.css` dans `public/` sont traités par LightningCSS en mode `prod`

### Améliorations

- 4 nouveaux golden tests (`@switch`, `bind:value`, `bind:checked`) — 80 tests au total

---

## [1.1.1] — 2026-06-04 · [GitHub Release](https://github.com/PrinMeshia/Webcore/releases/tag/v1.1.1)

### Corrections

- **Validation de formulaires** — le listener de soumission tourne maintenant en **phase de capture** avec `stopImmediatePropagation()`, garantissant qu'il s'exécute avant tout handler `on:submit` inline ; le contenu des blocs `@error` est préservé via `firstElementChild` (le texte d'erreur remplace le span interne sans supprimer la structure)
- **Handlers multi-instructions** — `on:click={a = 1; b = 2}` : les instructions séparées par `;` sont désormais compilées indépendamment (`S.set('a',1);S.set('b',2)`) au lieu de générer une expression JS invalide
- **`on:mount` imbrication profonde** — la grammaire Pest supporte désormais des accolades imbriquées à profondeur arbitraire dans le corps `on:mount { }` (règles récursives silencieuses `on_mount_nested`) ; les callbacks JS complexes (ex. `setInterval`, `addEventListener` avec corps multi-ligne) ne provoquent plus d'erreur de parse
- **`t()` dans `evalCond`** — la fonction i18n `t()` est maintenant passée en paramètre explicite aux `new Function()` générés par `evalCond` ; la variable locale interne a été renommée `_c` pour éviter le masquage
- **Sélecteurs CSS multi-éléments** — `input, textarea { }` est désormais valide dans les blocs `style { }` (virgule ajoutée à la règle `selector` dans `grammar.pest`)
- **DOM diffing `@for` avec key** — la clé DOM est posée sur `firstElementChild` de l'élément cloné au lieu d'un `<div>` wrapper, éliminant les espaces parasites entre éléments de liste
- **Navigation SPA** — les chemins dans `nav()` utilisent maintenant `/` comme préfixe (`/about.html`) pour éviter les 404 en mode dev

### Ajouts

- **Exemple `examples/forms/`** — site de démonstration complet avec deux composants de formulaire : `SignupForm` (username, email, password avec validate:pattern, website optionnel) et `ContactForm` (textarea avec compteur de caractères via `on:input` + variable `computed remaining`, bannière de succès, styles dark)
- **Todo list enrichi** — `examples/todo/` : les items peuvent être marqués comme faits (texte barré) ou supprimés ; délégation d'événements via `data-webcore-idx` ; helper `window.mkTodo` illustre le pattern pour créer des objets literals dans les expressions `on:click`

---

## [1.1.0] — 2026-06-03

### Ajouts

- **Routes paramétrées** — les routes peuvent désormais contenir des segments `:param`
  (ex. `"/post/:slug": PostPage`). Le compilateur génère un tableau `ROUTES` avec
  patterns RegExp, une fonction `matchRoute()` et un objet `ROUTE_PARAMS` mis à jour
  à chaque navigation. Les paramètres sont accessibles dans les vues via `{$route.slug}`.
  Tree-shaké : `ROUTES` / `ROUTE_PARAMS` sont émis uniquement si au moins une route
  est paramétrée.
- **`@for` avec key** — syntaxe `@for item key=item.id in items { ... }` pour activer
  le DOM diffing par clé. Émet `data-webcore-for-key` sur le `<template>` ;
  `bindFor()` patche uniquement les nœuds modifiés au lieu de re-rendre toute la liste.
- **i18n : paramètres et pluralisation** — `t("key", n)` pour la pluralisation
  (`_one` / `_other` + `{{count}}`) et `t("key", value)` pour la substitution
  positionnelle (`{{0}}` dans le TOML).
- **Props composées** — `{prop + 1}` ou `{step * count}` sont maintenant substitués
  même lorsque l'expression n'est pas une correspondance exacte de nom de prop.
  Même correction sur les valeurs d'attributs dynamiques (`class={color}`).
- **Messages d'erreur enrichis** — les erreurs de parsing affichent désormais la ligne
  source avec un caret pointant vers la colonne fautive, plus des hints contextuels
  pour les erreurs les plus fréquentes.

### Améliorations

- `evalCond` gère le préfixe `$route.xxx` → `ROUTE_PARAMS['xxx']` lorsque des routes
  paramétrées sont présentes.
- 7 nouveaux golden tests (76 au total).

---

## [1.0.0] — 2026-06-02

### Ajouts

- **`on:destroy { }`** — lifecycle hook symétrique à `on:mount` : le corps JS est exécuté avant chaque navigation SPA (`nav()`) et à `window.beforeunload` ; `DESTROY_HOOKS` tableau + `runDestroyHooks()` injectés dans le runtime ; `destroy_body: Option<String>` ajouté à l'AST `Component` ; `on:destroy` règle de grammaire Pest partagée avec `on_mount_body`
- **Tree-shaking du runtime** — seules les fonctions réellement utilisées sont émises :
  - `bindFor` : uniquement si le document contient des directives `@for`
  - `bindIf` : uniquement si le document contient des directives `@if`
  - `bindAttrs` : uniquement si le document contient des attributs dynamiques `class={expr}`
  - `validateField` + `bindValidation` : uniquement si le document contient des attributs `validate:*` ou des blocs `@error`
  - `nav` + `toFile` + listener `popstate` : uniquement si le document définit des routes ou des appels `webcore_navigate(...)`
  - `evalCond` : uniquement si l'une des fonctions ci-dessus est présente
  - `VARS` / `STORE_VARS` : uniquement si au moins une fonction de bind reactive est présente
  - `COMPUTED` + `rebindComputed` : uniquement si le composant contient un bloc `computed { }`
- **`runDestroyHooks` dans `nav()`** — avant chaque navigation, tous les hooks `on:destroy` sont exécutés ; `window.addEventListener('beforeunload', runDestroyHooks)` ajouté pour le déchargement de page

### Améliorations

- `webcore_navigate` n'est plus exporté dans `globalThis` si aucune navigation n'est détectée
- `setLocale` utilise la séquence `all_rebinds` contextuelle plutôt qu'un appel fixe à toutes les fonctions
- Le loader WASM utilise `all_rebinds` dynamique au lieu d'un appel hardcodé à toutes les fonctions de bind

---

## [0.9.0] — 2026-06-02

### Ajouts

- **État dérivé (`computed { }`)** — bloc `computed` dans les composants : `fullName = firstName + " " + lastName` ; les expressions sont compilées avec remplacement des variables d'état (`S.get(...)`) et des fonctions utilitaires (`U.max(...)`) ; `COMPUTED` tableau JS contenant `{name, fn}` pour chaque var dérivée ; `rebindComputed()` réévalue toutes les vars dérivées via `S.setQ(...)` (setter silencieux, sans déclenchement de listeners) avant chaque bind DOM ; `setQ` ajouté à la classe `State` ; `bind()` appelle `rebindComputed()` en premier
- **Lifecycle hooks (`on:mount { }`)** — bloc `on:mount` dans les composants : code JS brut exécuté dans `DOMContentLoaded` après `bind()`/`bindIf()`/etc. ; chaque corps est wrappé dans un IIFE pour éviter la fuite de variables locales ; `mount_body: Option<String>` ajouté à `Component` dans l'AST
- **Événements inter-composants (`emit` + `on:event`)** — `emit("eventName")` et `emit("eventName", data)` dans les expressions d'événements compilés vers `document.dispatchEvent(new CustomEvent(...))` ; `on:eventName={handler}` sur un appel de composant (ex. `Notifier on:notify={handler} {}`) enregistre `document.addEventListener('eventName', e => { handler })` dans `DOMContentLoaded` ; `EventListenerMapping` struct dans `codegen_js.rs` ; collecte récursive depuis pages, composants et layouts via `collect_component_event_listeners()`

### Améliorations

- Classe `State` : ajout de `setQ(k,v)` — setter silencieux qui met à jour la map sans déclencher les abonnés (utilisé par `rebindComputed` pour éviter des boucles)
- `bind()` enchaîne maintenant `rebindComputed()` → les vars dérivées sont toujours à jour avant le rebind des interpolations

---

## [0.8.0] — 2026-06-02

### Ajouts

- **Props réactives** — les props acceptent désormais des expressions dynamiques en plus des chaînes statiques : `Counter value={count} />` ; `Interpolation(propName)` dans la vue du composant est remplacée par `Interpolation(expr)` → reste un span réactif `data-webcore-interpolation` au lieu d'un `Text` figé ; les props statiques (`name="Alice"`) continuent de fonctionner comme avant ; `substitute_props` étendu avec paramètre `dynamic_props`
- **Named slots** — les layouts peuvent déclarer plusieurs slots nommés (`slot header`, `slot sidebar`, `slot content`) ; les pages fournissent le contenu via `slot header { ... }` (nouvelle syntaxe) ; les éléments non rattachés à un slot nommé alimentent le slot `content` par défaut ; résolution récursive via `resolve_slots()` — fonctionne à n'importe quelle profondeur dans l'arbre du layout ; rétrocompatibilité totale avec `main { slot content }`
- **`@media` dans les blocs `style { }`** — support des media queries directement dans les composants : `@media (max-width: 768px) { .card { ... } }` ; le scoping CSS (`data-v`) est propagé à l'intérieur des blocs `@media` ; nouveau type `StyleItem { Rule | Media { query, rules } }` dans l'AST ; `Component.style` passe de `Vec<StyleRule>` à `Vec<StyleItem>`

### Limites supprimées

- Props : les valeurs d'expressions dynamiques (`value={expr}`) sont maintenant supportées (était `String` statique uniquement)
- Un seul slot `content` par layout (maintenant N slots nommés)
- Pas de `@media` dans `style { }` (maintenant supporté)

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
