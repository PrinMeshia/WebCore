# Architecture du runtime JavaScript

> Référence interne du runtime émis par le compilateur (`src/codegen/js/`).
> Public visé : contributeurs au compilateur. Pour la syntaxe `.webc`, voir [spec.md](./spec.md).

## Vue d'ensemble

WebCore ne livre **aucun framework côté client**. Le compilateur émet un unique
fichier `dist/assets/webcore.js` assemblé à partir de fragments (`src/codegen/js/js_runtime.rs`,
`js_dom.rs`, `js_events.rs`), **tree-shaké par fonctionnalité** : si un document
n'utilise ni `@for`, ni i18n, ni validation, les fonctions correspondantes ne sont
pas émises. La détection se fait à la compilation via `RuntimeFeatures` (`js_dom.rs`).

Le contrat entre le HTML généré et le runtime passe exclusivement par des
attributs `data-webcore-*` (constantes centralisées dans `src/codegen/attr_names.rs`).

## Réactivité : `State` et `$effect`

```js
class State { #d = new Map(); #l = new Map(); #s = new Map(); ... }
const S = new State();      // état des composants
const STORE = new State();  // store global ($store.x)
```

- `S.get(k)` — lit une valeur **et** enregistre l'effet en cours (`__wcfx`) comme
  dépendance de `k` (tracking automatique).
- `S.set(k, v)` — écrit la valeur ; no-op si `Object.is` égal ; notifie les
  listeners `on(k, f)` puis ré-exécute les effets dépendants (une seule fois :
  le set de dépendances est vidé puis reconstruit par la ré-exécution).
- `S.setQ(k, v)` — écriture **silencieuse** (sans notification) ; utilisée pour
  l'injection des imports de données et le recalcul des `computed`.
- `$effect(fn)` — exécute `fn` immédiatement en s'enregistrant dans `__wcfx` ;
  `fn` est ré-exécutée automatiquement quand une de ses dépendances change.
  `__wcfx` est déclaré avec `var` pour être visible depuis les corps `new Function()`.

Invariant : **toutes** les fonctions `bind*` s'appuient sur `$effect`, donc un
composant n'est re-rendu que si une dépendance réellement lue a changé.

## `evalCond(expr)` — évaluation d'expressions

Évalue une expression `.webc` (`count > 0`, `item.price * qty`, `$store.user`)
dans le contexte de l'état courant. Stratégie en trois temps :

1. **Fast paths sans `new Function`** : identifiant simple (`VARS_SET`),
   `$store.x`, `$route.x`, `$query.x` — lecture directe.
2. **Réécriture** : `$store.x` → `STORE.get('x')`, variables d'état → `S.get('x')`
   (regex pré-compilées `_VR`, triées par longueur décroissante pour éviter les
   correspondances partielles).
3. **`new Function('S','STORE','U',…,'"use strict";return(<expr>)')`** — les
   identifiants block-scopés sont passés en paramètres car `Function()` s'exécute
   en portée globale. En cas d'erreur, retourne `undefined` (et non `false`) pour
   que les interpolations affichent `''`.

Sécurité : les expressions proviennent **exclusivement des fichiers `.webc`
compilés** (entrée du développeur, pas de l'utilisateur final) ; aucune chaîne
d'origine runtime n'atteint `new Function`.

## Fonctions de binding

| Fonction | Émise si | Rôle |
|---|---|---|
| `bindIf` | `@if` présent | Affiche/masque les `div[data-webcore-if]` et leur `else` adjacent ; gère `webc:transition` (classes `webc-<nom>-enter/leave`). |
| `bindFor` | `@for` présent | Rend les `<template data-webcore-for>` dans le conteneur `data-webcore-for-container` adjacent. |
| `bindAttrs` | attribut dynamique présent | Pour chaque `data-webcore-attr-<name>` sur un élément `data-webcore-bound` : propriété DOM si elle existe, sinon `setAttribute`. Gère aussi `data-webcore-style-<prop>` → `style.setProperty`. |
| `bindClassBindings` | `class:` présent | `classList.toggle(cls, !!evalCond(expr))` pour chaque `data-webcore-class-<cls>` (sélecteur ciblé `data-webcore-class-bound`). |
| `bindValidation` | `validate:` présent | Validation au `blur`/`input` (après premier blur) et au `submit` (bloquant) ; messages dans `[data-webcore-error="<field>"]`. |

### `bindFor` en détail (la partie la plus complexe du runtime)

- `fillItem(el, val, i)` : remplit les spans `data-webcore-interpolation`
  (chemins `item.prop.sub` supportés), écrit `data-webcore-idx`, et reflète les
  propriétés scalaires de l'objet en `data-*` (ciblage CSS).
- **Diffing par clé** (`@for item key=item.id`) : la clé est stockée sur
  `firstElementChild.dataset.webcoreKey` ; les éléments existants sont réutilisés
  (suppression de ceux absents, remplissage de ceux conservés, clonage des nouveaux).
- **Boucles imbriquées** : les templates internes reçoivent `_wc_ctx`
  (map `{varExterne: valeur}`) ; `fillItem` et `getItems()` y puisent les
  variables des boucles englobantes (`post.comments`, etc.).
- Garde-fous : `_wc_b` (anti double-binding lors des appels récursifs
  `bindFor(cont)`), `tmpl.isConnected` (les effets de templates retirés du DOM
  sortent immédiatement).
- `@for i in 0..5` : `data-webcore-for-range` — le tableau est généré côté
  runtime, sans donnée d'état.

## Délégation d'événements (CSP-safe)

Aucun `onclick=` inline. Chaque élément interactif reçoit un `id` unique
(`<prefix>btn<n>`) et `data-webcore-e="<type>[|mod…]"` ; les handlers compilés
sont dans la map `H[id]`. Un seul listener par type d'événement :

```js
const D = (t, p) => document.addEventListener(t, e => { /* closest('[data-webcore-e]') → H[el.id](e) */ });
```

Modificateurs `on:click|stop|prevent|once|self` encodés dans la valeur de
l'attribut et interprétés par `D`. Liens SPA : `data-webcore-nav` + History API.
CSS différé (critical CSS) : `data-webcore-defer` basculé en `media="all"` à
`DOMContentLoaded`.

## Référence des attributs `data-webcore-*`

| Attribut | Émis par | Consommé par |
|---|---|---|
| `data-webcore-if` / `data-webcore-else` | `render_if_element` | `bindIf`, SSG (`display` initial) |
| `data-webcore-for`, `-in`, `-for-key`, `-for-index`, `-for-range` | `render_for_element` | `bindFor` |
| `data-webcore-for-container` | `render_for_element` | `bindFor` (cible de rendu) |
| `data-webcore-interpolation` | `Element::Interpolation` | `bind` / `fillItem`, SSG (valeur initiale) |
| `data-webcore-bound`, `data-webcore-attr-*` | `generate_tag_element` | `bindAttrs` |
| `data-webcore-class-bound`, `data-webcore-class-*` | `handle_class_binding` | `bindClassBindings` |
| `data-webcore-style-*` | `handle_style_binding` | `bindAttrs` |
| `data-webcore-field`, `data-webcore-validate-*`, `data-webcore-error` | attrs de validation | `bindValidation`, `validateField` |
| `data-webcore-e` | `handle_event_attr` | délégation `D(t, p)` |
| `data-webcore-nav` | liens internes | délégation SPA |
| `data-webcore-ref` | `handle_ref_attr` | enregistrement `refs[name]` |
| `data-webcore-transition` | `webc:transition` | `bindIf` |
| `data-webcore-defer` | critical CSS (prod) | swap `media` à `DOMContentLoaded` |
| `data-v` | scoping CSS | sélecteurs CSS scopés (pas de JS) |
| `data-webcore-key`, `data-webcore-idx`, `data-webcore-onced` | **runtime** (`bindFor`, `D`) | runtime uniquement — jamais émis par le compilateur |

## Cycle de vie d'une page

1. `<script defer>` : le runtime s'exécute après le parsing HTML.
2. Déclarations : `State`, `VARS`, `evalCond`, fonctions `bind*`, `H`, `D`.
3. `DOMContentLoaded` : `refs`, `on:mount`, `$watch`, `bind()` (interpolations),
   `bindIf()`, `bindFor()`, `bindAttrs()`, `bindValidation()`, router.
4. Navigation SPA : `on:destroy` des composants quittés, puis re-bind de la
   nouvelle page (`__docsEnhance` compris).

Le SSG (`src/core/ssg.rs`) pré-remplit interpolations et `display` initiaux dans
le HTML ; le runtime ré-écrit ces valeurs au premier bind, ce qui rend les deux
mécanismes indépendants mais cohérents.
