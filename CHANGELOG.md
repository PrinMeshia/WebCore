# Changelog

Tous les changements notables sont documentés ici.
Format basé sur [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

---

## [2.7.0]

### Ajouts (v2.7.0)

- **`@for` imbriqué — accès aux variables externes** — les variables de boucle du `@for` parent sont désormais accessibles dans les boucles internes via un mécanisme de contexte (`_wc_ctx`) propagé aux templates imbriqués ; `fillItem()` résout les interpolations dans le contexte parent en cascade ; `bindFor()` accepte un paramètre `root=document` pour les appels récursifs ; garde `isConnected` pour les callbacks `$effect` stales ; flag `_wc_b` pour éviter le double-binding
- **Fix grammaire `expression`** — la règle PEG `expression` utilisait un lookahead `element` en contexte atomique, ce qui empêchait `@for` et `@if` d'être des fils directs d'un autre `@for` ; la règle est simplifiée en `@{ (!("{" | "}") ~ ANY)+ }`, plus correcte et plus robuste
- **`webc fmt`** — nouvelle commande CLI pour formater automatiquement les fichiers `.webc` ; implémente la conversion AST → source formatée avec 4 espaces d'indentation (configurable via `[fmt] indent = N` dans `webc.toml`) ; `webc fmt --check` sort avec code 1 si des fichiers seraient modifiés (CI) ; formatage idempotent garanti (parse → format → re-parse produit le même HTML)

### Corrections (v2.7.0)

- **`@for` / `@if` imbriqués sans wrapper** — auparavant, placer un `@for` ou un `@if` directement à l'intérieur d'un `@for` (sans balise wrapper) provoquait une erreur de parse `expected EOI, import_decl` ; corrigé par la simplification de la règle `expression`
- **Build déterministe** — les maps du document, du thème et de l'état initial étaient des `HashMap` (ordre d'itération aléatoire) : l'ordre des règles dans `theme.css`, l'ordre des handlers dans `webcore.js` et les ids générés variaient d'un build à l'autre, rendant les hash de cache-busting (`?v=…`) instables ; toutes les maps passent en `BTreeMap` — deux builds identiques produisent désormais un `dist/` identique au byte près
- **Éléments void valides** — les balises fermantes ne sont plus émises pour les éléments void HTML (`input`, `img`, `br`, `hr`, …) ; `</input>` était du HTML invalide ; règle verrouillée en un point unique (`push_close_tag` + `debug_assert`)
- **Nœuds texte propres** — suppression du retour à la ligne parasite avant les balises fermantes (`<span>x\n</span>` → `<span>x</span>`) ; rendu inchangé, l'espacement inter-éléments étant déjà fourni après chaque élément
- **`webc:img` réparé** — l'injection des dimensions `width`/`height` ne s'exécutait **jamais** lors d'un vrai `webc build` (la génération recevait toujours `project_root = None`) ; `build.rs` passe désormais la racine du projet ; test de régression avec un vrai PNG

### Qualité & outillage (v2.7.0)

- **CI GitHub Actions** — workflow `fmt --check` · `clippy -D warnings` · `cargo test`, en matrice Linux / Windows / macOS (le README annonçait une CI qui n'existait pas)
- **Tests d'intégration full-build** — chaque projet de `examples/` est compilé de bout en bout dans un dossier temporaire : fichiers attendus, JS validé syntaxiquement via `node --check`, déterminisme byte-à-byte vérifié
- **Test de performance** — projet synthétique (50 composants, 20 pages) compilé en ~70 ms ; garde-fou à 60 s contre les régressions de complexité
- **Test prod de bout en bout** — minification, SRI, critical CSS inliné, stylesheet différée, meta CSP et déterminisme vérifiés sur un build `mode = "prod"`
- **`t()` exécuté sous Node** — sélection de pluriel `_one`/`_other`, replis (forme plurielle absente, clé absente) et substitution `{{0}}`/`{{count}}` vérifiés en exécutant réellement le runtime émis
- **SSG au niveau AST** — le pré-rendu (interpolations, `display` des `@if`/`@else`) se fait à l'émission du HTML via `SsgContext`, au lieu de trois regex appliquées au HTML généré ; sortie strictement identique, mécanisme nettement plus robuste
- **`codegen/html` découpé** — `mod.rs` (1 483 lignes) éclaté en modules ciblés (`shell`, `slots`, `elements`, `tags`, `components`) ; les signatures à 8 paramètres remplacées par un `GenContext` partagé
- **Zéro `unwrap()` hors tests** — lint `clippy::unwrap_used` actif au niveau du crate ; les écritures infaillibles documentées par `.expect()`, les vrais risques éliminés
- **`docs/runtime.md`** — référence d'architecture du runtime JS : modèle de réactivité, contrat des `data-webcore-*`, `bindFor` détaillé, délégation d'événements
- 161 tests au total (unitaires, golden, intégration, perf)

---

## [2.6.0] 

### Ajouts (v2.6.0)

- **Fragment shorthand `<>...</>`** — groupe d'éléments sans balise wrapper ; compilé en nœuds inline ; supporte les directives de contrôle, les composants et l'imbrication arbitraire ; `Element::Fragment` dans l'AST, `<>` / `</>` dans la grammaire PEG
- **Modificateurs d'événements** — `on:click|stop`, `on:click|prevent`, `on:click|once`, `on:click|self` — encodés dans `data-webcore-e="click|stop|prevent"` ; gérés par le listener délégué `D()` sans JS inline ; combinables (ex. `on:click|stop|prevent`) ; `|once` utilise un marqueur `data-webcore-onced` pour garantir l'exécution unique ; `|self` s'exécute uniquement si `e.target === el`
- **Valeurs de props par défaut** — `props { label: String = "Défaut" }` — si la prop est omise à l'instanciation, la valeur par défaut est injectée statiquement ; compatible avec les props statiques et dynamiques ; types string, numérique et booléen supportés
- **Commande `webc watch`** — surveille les fichiers sources et rebuilde automatiquement à chaque modification sans serveur de développement ; debounce 200 ms via la crate `notify` v8.2 ; idéal pour les pipelines CI/CD ou les builds continus
- **Analyse de bundle améliorée** — détection du core bytes corrigée (`class State{` au lieu de `class _S`) ; ajout de `bindClassBindings` et `evalCond` dans le tableau d'analyse ; les tailles estimées reflètent mieux le bundle réel

### Corrections (v2.6.0)

- **Détection core bytes** — `class State{` remplace `class _S` dans `output.rs` ; le core était systématiquement compté à 420 octets fixes au lieu de refléter le nombre réel de composants réactifs
- 8 nouveaux tests — 147 tests au total

---

## [2.5.2]

### Améliorations DX (v2.5.2)

- **Messages d'erreur parse enrichis** — les erreurs de compilation affichent désormais un en-tête structuré `error[parse]: fichier:ligne:col`, la ligne source fautive, un caret `^` sous la colonne exacte, et la clause `expected` extraite du message Pest
- **Chemin de fichier dans les erreurs** — le fichier `.webc` source est propagé dans `ParseError` depuis tous les points de chargement (`app.webc`, `layouts/`, `components/`, `pages/`) ; chaque erreur indique son fichier exact
- **Couleurs ANSI conditionnelles** — `error[parse]` (rouge gras), gutter `|` (cyan), caret `^` (rouge) ; désactivés automatiquement si `NO_COLOR=1` ou `TERM=dumb` ; `CompileError::Io` et `MissingLayout/Page/Component` reçoivent aussi des préfixes colorés
- **Hints contextuels élargis** — cinq patterns déclenchent un message `= hint:` : `{}` vide, accolade fermante manquante, guillemets attendus, expression JS attendue, nom sans espaces
- **`webc build` — suppression du préfixe redondant** — `"Build failed:"` retiré de `cli.rs` ; `CompileErrors::Display` affiche directement les erreurs puis le compte final
- 2 nouveaux tests (NO_COLOR, file path) — 139 tests au total

### Performances internes (v2.5.2)

- **`bindFor` non-clé — mutation DOM atomique** — le chemin sans `key=` remplace désormais
  `innerHTML=''` + N `appendChild` par un `DocumentFragment` accumulé puis un seul
  `replaceChildren(frag)` ; une seule mutation DOM atomique élimine les reflows intermédiaires
  (bénéfice direct sur les longues listes)
- **`evalCond` — `VARS_SET` et regexes pré-compilées** — `const VARS_SET=new Set(VARS)` pour
  un lookup O(1) des variables simples (remplace `VARS.indexOf` O(n)) ; `const _VR=[...VARS].sort(...).map(v=>[RegExp,...])` pré-compile les regexes de substitution une seule fois au
  chargement de la page plutôt qu'à chaque appel `evalCond` sur une expression composite
- **SSG — `OnceLock<Regex>`** — les 3 expressions régulières de `apply_ssg_with_locales`
  (interpolation, `@if`, `@else`) sont compilées une fois par processus via `OnceLock` au lieu
  d'être recompilées à chaque page générée
- **SSG — `html_unescape` / `html_escape_text` passe unique** — les 5 appels `.replace()` chaînés
  et les 3 appels `.replace()` chaînés sont remplacés par des scanners passe unique avec sortie
  anticipée quand aucun caractère spécial n'est présent
- **`resolve_slots` — court-circuit** — ajout de `contains_slot()` ; `resolve_slots` retourne
  `elements.to_vec()` immédiatement si aucun slot n'est présent dans l'arbre, évitant la
  reconstruction match-par-élément pour les sous-arbres sans slot dans les layouts

### Refactorisation interne (v2.5.2)

- **Module split CLI** — `build.rs`, `serve.rs`, `check.rs`, `cli.rs` réorganisés dans `src/cli/`
  avec sous-modules dédiés (`config.rs`, `loader.rs`, `output.rs`, `assets.rs`)
- **Module split codegen** — `codegen_html.rs` scindé en `html/mod.rs`, `html/attrs.rs`,
  `html/analysis.rs`, `html/minify.rs`, `html/props.rs`, `html/utils.rs` ;
  `codegen_css.rs` → `css.rs` ; `codegen_js/` → `js/`
- **Module `core/`** — `ast.rs`, `ssg.rs`, `error.rs`, `css_processor.rs`, `theme.rs`
  regroupés dans `src/core/`
- **Précompilation des regexes de variables** — les N regexes de substitution de variables
  d'état sont compilées une seule fois par document au lieu d'une recompilation par expression

---

## [2.5.1] 

### Corrections de sécurité (v2.5.1)

- **Injection via CSS inline** — les séquences `</style>` dans le CSS critique inliné sont désormais échappées en `<\/style>` ; empêchait une sortie prématurée du bloc `<style>` pouvant injecter du HTML/JS arbitraire
- **Zero-JS + critical CSS** — les pages purement statiques avec `critical_css` activé incluent désormais `webcore.js` (requis pour le swap `data-webcore-defer` → `media="all"`) ; avant, le `<link media="print">` restait bloqué indéfiniment
- **Composant avec seulement un handler d'événement** — `document_needs_js()` vérifie maintenant `elements_need_js()` sur les vues des composants (pas seulement sur leur `state`/`computed`) ; évitait d'omettre `webcore.js` pour des composants n'ayant que des handlers `on:click`/`on:submit`

### Corrections (v2.5.1)

- **Longueur de tableau avec virgules dans des chaînes** — `eval_expr_with_locale("items.length")` utilise désormais `serde_json` pour compter les éléments d'un tableau JSON ; la heuristique par `split(',')` renvoyait un compte incorrect pour `["a,b","c"]` (3 au lieu de 2)
- **Longueur de chaîne Unicode** — `val.chars().count()` remplace `val.len()` pour les expressions `.length` sur les variables de type string ; `"café".length` retournait 5 (octets) au lieu de 4 (caractères)
- 5 nouveaux tests — 137 tests au total

---

## [2.5.0] 

### Ajouts (v2.5.0)

- **CSP stricte — event delegation** — tous les attributs `onclick=`, `onsubmit=`, `onchange=`, `oninput=` inline sont remplacés par `data-webcore-e="<type>"` ; un listener unique par type d'événement est enregistré via `document.addEventListener` (fonction `D(t,p)`) ; élimine la nécessité de `script-src 'unsafe-inline'` dans la Content-Security-Policy
- **SPA links `data-webcore-nav`** — les liens de navigation interne (`link to="/path"`) reçoivent `data-webcore-nav` à la place de `onclick="webcore_navigate(...)"` ; le JS délègue via `document.addEventListener('click', ...)` sur `a[data-webcore-nav]`
- **CSS déféré `data-webcore-defer`** — le lien feuille `media="print"` reçoit l'attribut `data-webcore-defer` à la place de `onload="this.media='all'"` ; le swap vers `media="all"` est effectué dans le callback `DOMContentLoaded` (100% CSP-safe)
- **Meta `Content-Security-Policy`** — quand `csp = true` est posé dans `webc.toml` (mode `prod`), chaque page reçoit `<meta http-equiv="Content-Security-Policy" content="default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self' data:">` dans son `<head>`
- **Option `csp` dans `webc.toml`** — `[app] csp = true` active l'émission du meta CSP en mode prod

### Améliorations (v2.5.0)

- Exports `globalThis` nettoyés : `webcore_handle_click`, `webcore_handle_submit`, etc. retirés (plus nécessaires avec la délégation) ; seuls `webcore_navigate` (si routing) et `setLocale` (si i18n) restent exportés
- 4 nouveaux tests — 132 tests au total

---

## [2.4.0] 

### Ajouts (v2.4.0)

- **Critical CSS inline (prod)** — en mode `prod`, chaque page reçoit dans son `<head>` un `<style>` contenant uniquement le CSS dont elle a besoin (styles globaux + composants réellement utilisés, collectés récursivement) ; la feuille `theme.css` complète est chargée en différé (`media="print"` + swap `onload`, fallback `<noscript>`) ; élimine le CSS render-blocking — gain direct sur le First Contentful Paint ; le lien différé et le fallback reçoivent hash `?v=` et SRI comme avant
- **Collections SSG** — `"/post/:slug": PostPage each posts` dans le bloc `routes {}` : une page statique est générée par élément de l'import de données lié (`import posts from "data/posts.json"`) ; le champ correspondant au paramètre (`slug`) détermine le chemin de sortie (`dist/post/<slug>/index.html`) et `{$route.slug}` est pré-rendu dans le HTML ; transforme WebCore en vrai générateur de site statique (blog, docs, portfolio) ; sécurité : les valeurs contenant `/`, `\`, `..` ou vides sont rejetées (le champ devient un nom de répertoire)
- **Résolution des imports de données câblée au build** — les déclarations `import name from "file.json"` parsées depuis `app.webc`, layouts, composants et pages sont désormais réellement résolues par `webc build` : lecture du fichier, validation JSON/TOML, conversion TOML→JSON (crate `serde_json`), injection `S.setQ(name, data)` ; les chemins canonicalisés doivent rester dans le répertoire projet

### Corrections (v2.4.0)

- **Hash + SRI sur `<link rel="preload">`** — le patch cherchait `href="..." as="script"` alors que la balise est émise `as="script" href="..."` ; le hint preload ne recevait donc jamais son `?v=hash` ni son attribut `integrity` ; corrigé
- **Exemple `docs`** — `"style {}"` littéral dans `syntax.webc` était interprété comme interpolation vide ; échappé en `"style \{\}"`

### Améliorations

- 10 nouveaux tests — 128 tests au total

---

## [2.3.0] 

### Ajouts (v2.3.0)

- **Subresource Integrity (SRI)** — en mode `prod`, les balises `<script>` et `<link rel="stylesheet">` reçoivent automatiquement un attribut `integrity="sha256-<base64>"` + `crossorigin="anonymous"` ; les hash sont calculés avec SHA-256 via la crate `sha2` ; les hint `<link rel="preload">` reçoivent également leur SRI
- **Zero-JS elision** — les pages purement statiques (sans état réactif, sans boucle, sans interpolation, sans événements, sans composants réactifs) n'émettent plus de `<script defer>` ni de `<link rel="preload">` dans le `<head>` ; réduit le poids et les requêtes réseau pour les pages de contenu

### Corrections de sécurité (v2.3.0)

- **Limite de profondeur d'imbrication** — le parser rejette désormais tout document dont les éléments dépassent 128 niveaux d'imbrication avec un message d'erreur explicite ; protège contre les "nesting bombs" qui provoquaient un stack overflow pendant la compilation
- **Escape JS des URLs de navigation** — dans les balises `<a onclick="webcore_navigate(...)">`, les apostrophes et backslashes sont maintenant échappés dans le chemin JS (`\'`, `\\`) ; empêche une injection JS si le chemin contient ces caractères

---

## [2.1.0] 

### Ajouts

- **`$watch varName => { body }`** — nouvelle directive dans les composants pour observer les changements d'état sans effet DOM direct ; émet `S.on('varName', varName => { body })` dans le bloc `DOMContentLoaded` ; permet d'exécuter du code réactif (logs, analytics, synchronisation) quand une variable change
- **`on:click` avec objets littéraux imbriqués** — `on:click={handler({key: val})}` est maintenant supporté ; `expression_content` utilise une règle récursive `expr_brace_seq` qui gère les accolades imbriquées arbitrairement ; `on:click={x = {val: 1}.val}` parse correctement
- **`@for key={expr}` — expressions de clé complexes** — en plus de `key=item.id`, la syntaxe `key={item.id + "-" + item.type}` permet des expressions arbitraires comme clé de diffing DOM ; le parser détecte `for_key_braced` vs `for_key_expr` automatiquement
- **`@for N..M` — syntaxe de plage** — `@for i in 0..5 { ... }` itère `i` de 0 à 4 ; détecté à la compilation via le pattern `N..M` dans l'itérable ; émet `data-webcore-for-range="0..5"` ; le runtime JS génère le tableau `["0","1","2","3","4"]` sans donnée d'état
- **Expressions SSG étendues** — `eval_expr_with_locale` supporte maintenant : `items.length` (nombre d'éléments d'un tableau ou longueur d'une chaîne), `name.toUpperCase()`, `name.toLowerCase()`, `str.trim()` ; élimine les valeurs vides au pré-rendu SSG
- **Validation des props à la compilation** — si un composant reçoit un prop non déclaré dans son bloc `props {}`, un avertissement `warning[props]: component 'X' received unknown prop 'y'` est émis sur stderr ; avertissement uniquement (la compilation continue)
- **Imports de données build-time (JSON/TOML)** — `import posts from "data/posts.json"` dans un fichier `.webc` injecte les données à la compilation ; les fichiers JSON sont validés et émis comme `S.setQ("posts", <json>)` dans le runtime ; les fichiers TOML sont convertis en JSON via la crate `toml` ; sécurité : les chemins qui sortent du répertoire projet sont refusés

### Améliorations

- 5 nouveaux tests — 118 tests au total

---

## [2.2.0] 

### Ajouts

- **CSS `@keyframes`** — les blocs `@keyframes` sont désormais supportés dans les blocs `style {}` des composants ; les keyframes sont émis globaux (non scopés) car ils sont référencés par nom depuis la propriété `animation:` ; le parser, l'AST (`StyleItem::Keyframes`, `KeyframeStep`), la grammaire PEG et le codegen CSS sont tous mis à jour
- **Préchargement `<link rel="preload">`** — le shell HTML émet `<link rel="preload" as="script" href="/assets/webcore.js">` dans le `<head>` pour les pages interactives ; accélère le chargement initial en parallélisant le téléchargement du runtime JS
- **`<script defer>`** — toutes les balises `<script src="webcore.js">` utilisent désormais l'attribut `defer` ; le script ne bloque plus le parsing HTML et s'exécute après le DOM
- **Hash CSS** — `theme.css` reçoit désormais un paramètre de version `?v=<hash>` comme `webcore.js` ; casse le cache navigateur à chaque modification du CSS
- **Minification HTML (prod)** — en mode `prod`, les commentaires HTML et les espaces inter-balises sont supprimés ; réduit la taille des fichiers HTML distribués
- **Élision du scope CSS pour composants sans style** — les composants sans bloc `style {}` n'émettent plus d'attribut `data-v="..."` sur leurs éléments ; réduit le bruit HTML et la taille des pages

### Corrections

- **Avertissement ReDoS** — `validate:pattern` émet un avertissement `warning[security]` à la compilation si le pattern contient des quantificateurs imbriqués (`)+`, `)*`) qui peuvent causer un backtracking catastrophique dans le moteur regex du navigateur

---

## [2.0.0] 

### Rupture avec v1.x

- **Signaux réactifs fins (`$effect`)** — l'abonnement `VARS.forEach(v=>S.on(v,fn))` est remplacé par `$effect(fn)` ; le tracking des dépendances est automatique via `var __wcfx=null` et le champ `#s` de la classe `State` ; un composant ne se re-rend que lorsque ses dépendances réelles changent ; réduction de la mémoire et des re-renders inutiles
- **HMR (rechargement automatique)** — `webc serve` surveille les fichiers source et recharge le navigateur automatiquement via WebSocket ; aucune configuration requise
- **Sécurité : path traversal corrigé** — `webc serve` utilisait `format!("dist{url}")` directement ; `resolve_safe_path()` utilise maintenant `fs::canonicalize()` + `starts_with(dist_root)` ; toute URL qui sort de `dist/` retourne 403
- **Détection de cycles** — `webc check` détecte les références circulaires entre composants (A utilise B qui utilise A) et rapporte le cycle complet

### Ajouts

- **Agrégation des erreurs de compilation** — `webc build` collecte désormais TOUTES les erreurs avant de s'arrêter et les affiche en une seule passe, comme le compilateur Rust ; `CompileErrors(Vec<CompileError>)` encapsule la liste complète
- **CSS nesting** — les règles imbriquées (`&:hover { }`, `& > span { }`, `&::before { }`) sont désormais valides dans les blocs `style {}` ; aplaties en CSS scopé valide à l'émission ; le parser, l'AST (`StyleRule.nested`) et le codegen CSS sont tous mis à jour
- **Rapport d'analyse du bundle** — après un `webc build` réussi, un tableau affiche les fonctionnalités runtime incluses (`✓`) ou tree-shaquées (`-`) avec leurs tailles estimées ; aide à diagnostiquer ce qui contribue au bundle JS final

### Refactorisation

- Modules JS scindés : `js_runtime.rs`, `js_events.rs`, `js_dom.rs`
- Modules parser scindés : `parser/elements.rs`, `parser/directives.rs`, `parser/declarations.rs`
- `CompileError` : enum typée remplaçant `Result<T, String>` dans tout le codegen
- `attr_names.rs` : constantes centralisées pour tous les attributs `data-webcore-*`
- Macro `write!()` : élimine les allocations `String` intermédiaires dans les émetteurs HTML/CSS/JS
- Substitution O(n) des props : `HashMap<&str, (bool, &str)>` construit une seule fois
- Validation CSS : avertissement sur les noms de propriétés CSS inconnus (les variables `--custom-var` sont toujours autorisées)

### Améliorations

- 19 nouveaux tests par rapport à v1.5.0 — 105 tests au total
- **Extension VSCode** — support de la coloration syntaxique pour `ref:`, `style:`, `webc:img`, `webc:transition`, CSS nesting (`&:hover`), `on:mount`/`on:destroy`, `key={}` dans `@for` ; 25 snippets ajoutés

---

## [1.5.0] 

### Ajouts

- **`webc:img` — images optimisées** — directive `img webc:img src="/hero.png" alt="Hero"` compilée en `<img src="/assets/hero.png" loading="lazy" decoding="async" width="1200" height="630" alt="Hero">` ; `loading="lazy"` et `decoding="async"` injectés automatiquement ; dimensions (`width`/`height`) lues dans `public/` à la compilation (prévient le layout shift / CLS) ; avertissement `warning[a11y]: <img> with webc:img is missing alt attribute` si `alt` est absent ; `webc:img` n'apparaît pas dans le HTML généré ; aucun JS émis — transformation purement compile-time ; nécessite le crate `imagesize`
- **Fingerprinting des images** — chaque image dans `public/` reçoit un hash de contenu à `webc build` : `logo.png` → `logo.a3f9c1b2.png` ; extensions concernées : `.png`, `.jpg`, `.jpeg`, `.gif`, `.webp`, `.svg`, `.ico`, `.avif` ; algorithme : FNV-1a 32 bits sur les octets du fichier → 8 caractères hex ; toutes les références dans les `.html` et `.css` générés sont mises à jour automatiquement ; toujours actif (aucune configuration nécessaire) ; avantage : cache-busting parfait — le navigateur met les images en cache indéfiniment, un nouveau contenu produit un nouveau nom de fichier

### Améliorations

- 3 nouveaux tests (fingerprinting, `webc:img`, avertissement `alt`) — 92 tests au total

---

## [1.4.0] 

### Ajouts

- **`ref:name=true`** — références DOM directes : `input ref:name=true` émet `data-webcore-ref="name"` sur l'élément ; `const refs={}` déclaré à la portée du bloc ; `refs['name'] = document.querySelector('[data-webcore-ref="name"]')` enregistré dans `DOMContentLoaded` — accès direct sans `querySelector` ; utile pour la gestion du focus et les manipulations DOM impératives ; tree-shaké via le flag `has_refs`
- **`style:prop={expr}`** — styles inline dynamiques : `div style:color={myColor}` émet `data-webcore-style-color="myColor"` ; `bindAttrs()` appelle `el.style.setProperty('color', evalCond(myColor, ...))` ; les tirets dans le nom de propriété sont préservés (`style:background-color`) ; peut coexister avec `style="..."` statique et `class:` sur le même élément ; tree-shaké via le flag `has_style_binding`
- **Contenu par défaut des slots** — les layouts peuvent définir un contenu de repli pour les slots nommés : `slot sidebar { p "Contenu par défaut" }` ; si la page remplit le slot → contenu de la page utilisé ; si la page ne remplit pas le slot → contenu par défaut du layout utilisé ; les slots non remplis étaient précédemment supprimés silencieusement ; le slot `content` par défaut continue d'utiliser le corps de la page
- **`webc:transition="name"`** — animations CSS sur les blocs conditionnels : `div webc:transition="fade" { ... }` ou `div webc:transition="slide" { ... }` ; transitions intégrées : `fade` (opacité 0→1) et `slide` (translateY -10px→0) ; fonctionne avec les blocs `@if` : entrée avec animation d'entrée, sortie avec animation de sortie ; attribut HTML `data-webcore-transition="name"` ; le JS injecte le CSS et utilise `requestAnimationFrame` + `transitionend` ; tree-shaké via le flag `has_transition`

### Améliorations

- 4 nouveaux golden tests (`ref:`, `style:`, slot default content, `webc:transition`) — 89 tests au total

---

## [1.3.0] 

### Ajouts

- **`http { }` — requêtes HTTP déclaratives** — bloc `http` dans les composants : `get: "/url"  into: varName` déclenche un `fetch()` JSON dans `DOMContentLoaded` ; `loading: Boolean = true` et `error: String = ""` sont **auto-injectés** par le parser (pas besoin de les déclarer dans `state`) et deviennent pleinement réactifs ; le bloc `try/catch` généré pose `S.set('loading', false)` dans les deux branches et `S.set('error', __e.message)` en cas d'échec
- **`head { }` — personnalisation du `<head>` par page** — bloc `head` dans une déclaration `page` : `title "Mon titre"` génère `<title>` ; `meta name="..."` et `meta og:title="..."` génèrent les balises `<meta name="..." content="...">` correspondantes ; override le titre global défini dans `webc.toml`
- **`$query.` — paramètres query string** — accès aux paramètres d'URL avec `{$query.search}`, `{$query.page}`, etc. ; tree-shaké : n'émet `const QUERY_PARAMS = new Proxy({}, {get:(_,k)=>new URLSearchParams(location.search).get(String(k))??""})` que si au moins une référence `$query.` est présente dans le document
- **`class:name={expr}` — classes CSS conditionnelles** — `class:active={isOpen}` émet `data-webcore-class-active="isOpen"` ; `bindAttrs()` active/désactive la classe selon l'expression booléenne ; plusieurs `class:` peuvent coexister sur le même élément ; tree-shaké avec la logique class-toggle
- **`on:event|debounce` — handlers debouncés** — `on:input|debounce={expr}` enveloppe le handler dans `setTimeout(..., 300)` — le handler ne se déclenche qu'après 300 ms d'inactivité ; fonctionne avec tout type d'événement (`on:input|debounce`, `on:keyup|debounce`, etc.)

### Corrections

- **Auto-injection `loading` / `error`** — les variables `loading: Boolean = true` et `error: String = ""` sont désormais injectées automatiquement par le parser lorsqu'un composant possède un bloc `http {}` ; les développeurs n'ont plus besoin de les déclarer manuellement dans `state`

### Améliorations

- 5 nouveaux golden tests (`http {}`, `head {}`, `$query.`, `class:`, `|debounce`) — 85 tests au total

---

## [1.2.0] 

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

## [1.1.1] 

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

## [1.1.0] 

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

## [1.0.0] 

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

## [0.9.0] 

### Ajouts

- **État dérivé (`computed { }`)** — bloc `computed` dans les composants : `fullName = firstName + " " + lastName` ; les expressions sont compilées avec remplacement des variables d'état (`S.get(...)`) et des fonctions utilitaires (`U.max(...)`) ; `COMPUTED` tableau JS contenant `{name, fn}` pour chaque var dérivée ; `rebindComputed()` réévalue toutes les vars dérivées via `S.setQ(...)` (setter silencieux, sans déclenchement de listeners) avant chaque bind DOM ; `setQ` ajouté à la classe `State` ; `bind()` appelle `rebindComputed()` en premier
- **Lifecycle hooks (`on:mount { }`)** — bloc `on:mount` dans les composants : code JS brut exécuté dans `DOMContentLoaded` après `bind()`/`bindIf()`/etc. ; chaque corps est wrappé dans un IIFE pour éviter la fuite de variables locales ; `mount_body: Option<String>` ajouté à `Component` dans l'AST
- **Événements inter-composants (`emit` + `on:event`)** — `emit("eventName")` et `emit("eventName", data)` dans les expressions d'événements compilés vers `document.dispatchEvent(new CustomEvent(...))` ; `on:eventName={handler}` sur un appel de composant (ex. `Notifier on:notify={handler} {}`) enregistre `document.addEventListener('eventName', e => { handler })` dans `DOMContentLoaded` ; `EventListenerMapping` struct dans `codegen_js.rs` ; collecte récursive depuis pages, composants et layouts via `collect_component_event_listeners()`

### Améliorations

- Classe `State` : ajout de `setQ(k,v)` — setter silencieux qui met à jour la map sans déclencher les abonnés (utilisé par `rebindComputed` pour éviter des boucles)
- `bind()` enchaîne maintenant `rebindComputed()` → les vars dérivées sont toujours à jour avant le rebind des interpolations

---

## [0.8.0] 

### Ajouts

- **Props réactives** — les props acceptent désormais des expressions dynamiques en plus des chaînes statiques : `Counter value={count} />` ; `Interpolation(propName)` dans la vue du composant est remplacée par `Interpolation(expr)` → reste un span réactif `data-webcore-interpolation` au lieu d'un `Text` figé ; les props statiques (`name="Alice"`) continuent de fonctionner comme avant ; `substitute_props` étendu avec paramètre `dynamic_props`
- **Named slots** — les layouts peuvent déclarer plusieurs slots nommés (`slot header`, `slot sidebar`, `slot content`) ; les pages fournissent le contenu via `slot header { ... }` (nouvelle syntaxe) ; les éléments non rattachés à un slot nommé alimentent le slot `content` par défaut ; résolution récursive via `resolve_slots()` — fonctionne à n'importe quelle profondeur dans l'arbre du layout ; rétrocompatibilité totale avec `main { slot content }`
- **`@media` dans les blocs `style { }`** — support des media queries directement dans les composants : `@media (max-width: 768px) { .card { ... } }` ; le scoping CSS (`data-v`) est propagé à l'intérieur des blocs `@media` ; nouveau type `StyleItem { Rule | Media { query, rules } }` dans l'AST ; `Component.style` passe de `Vec<StyleRule>` à `Vec<StyleItem>`

### Limites supprimées

- Props : les valeurs d'expressions dynamiques (`value={expr}`) sont maintenant supportées (était `String` statique uniquement)
- Un seul slot `content` par layout (maintenant N slots nommés)
- Pas de `@media` dans `style { }` (maintenant supporté)

---

## [0.7.0] 

### Ajouts

- **WebAssembly (WASM)** — détection automatique de `wasm/Cargo.toml` ; invocation de `wasm-pack build --target web` au build ; loader asynchrone injecté dans le runtime JS : `const WASM={}; globalThis.wasm=WASM; (async()=>{try{...}catch(e){...}})()` — remplit `globalThis.wasm` avec toutes les exports du module et déclenche un rebind complet dès le chargement ; `webc new` crée le scaffold WASM (`wasm/Cargo.toml`, `wasm/src/lib.rs` avec exemple `wasm-bindgen`) ; `wasm_module` ajouté à `WebCoreDocument` ; 2 tests golden

---

## [0.6.0] 

### Ajouts

- **Internationalisation (i18n)** — fichiers `locales/<code>.toml` (TOML plat `clé = "valeur"`) ; chargés au build dans `document.locales` ; runtime JS : `const LOCALES`, `let LOCALE`, `const t=k=>LOCALES[LOCALE]?.[k]??k`, `const setLocale=l=>{...}` (réactif : rebind de toutes les directives) ; `setLocale` exposé dans `globalThis` ; `locale` configurable dans `webc.toml` (`[app] locale = "fr"`, défaut = valeur de `lang`) ; SSG pré-rend `{t("key")}` avec la locale par défaut ; 3 tests golden

---

## [0.5.0] 

### Ajouts

- **SSG (Static Site Generation)** — nouveau module `ssg.rs` : `build_initial_state` collecte les valeurs par défaut de tous les composants et du store ; `apply_ssg` post-traite le HTML généré pour (1) pré-remplir les `<span data-webcore-interpolation>` avec les valeurs initiales et (2) pré-définir `style="display:block/none"` sur les divs `@if`/`@else` selon l'état initial — élimine le flash de contenu incorrect au premier chargement ; compatible avec le runtime JS (`bindIf`/`bind` continuent à opérer normalement) ; `evalCond` simple supporte `>`, `<`, `>=`, `<=`, `==`, `!=` sur des variables numériques ; 9 tests unitaires internes + 2 golden tests

---

## [0.4.0] 

### Ajouts

- **`webc new <nom>`** — commande de scaffolding : crée la structure complète d'un projet (webc.toml, theme.toml, layouts, pages, Counter.webc, public/)
- **Exemples** — trois projets d'exemple dans `examples/` : `counter`, `todo`, `blog`
- **Spec du langage** — référence complète dans `docs/spec.md` (syntaxe, directives, routage, thème, runtime JS)
- **Hot reload WebSocket** — remplacement du polling HTTP toutes les 500 ms par une connexion WebSocket persistante (`ws://localhost:{port+1}`) ; le serveur envoie `"reload"` après chaque rebuild, la page se recharge instantanément ; reconnexion automatique si la connexion est perdue
- **Extension VS Code** — `editors/vscode/` : coloration syntaxique complète pour `.webc` via TextMate grammar (`app`, `layout`, `page`, `component`, `@if`/`@else`/`@for`, HTML tags, attributs, interpolations `{expr}`, CSS scopé dans `style { }`, types, fonctions built-in)
- **Store global partagé** — bloc `store { varName: Type = valeur }` au niveau document ; variables référencées avec `$store.varName` dans les expressions et interpolations ; `STORE.set/get/on` dans le runtime JS ; `$store.var += 1` et `$store.var = val` compilés correctement dans les handlers ; `bindIf/bindFor/bind/bindAttrs` réactifs aux changements du store ; `@for item in $store.list` supporté
- **Validation de formulaires déclarative** — attributs `validate:required`, `validate:minlength`, `validate:maxlength`, `validate:email`, `validate:pattern` sur les inputs ; directive `@error "field" { }` pour l'affichage des erreurs ; validation au blur + soumission ; `validateField()` + `bindValidation()` injectés dans le runtime

---

## [0.3.0] 

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

## [0.2.0] 

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

## [0.1.0] — (MVP initial)

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
