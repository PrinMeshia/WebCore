# Architecture du runtime JavaScript

> Référence interne du runtime émis par le compilateur (`src/codegen/js/`).
> Public visé : contributeurs au compilateur. Pour la syntaxe `.webc`, voir [spec.md](./spec.md).

## Vue d'ensemble

WebCore ne livre **aucun framework côté client**. En v3, le compilateur émet un
`<script>` inline à la fin du `<body>` de chaque page, **tree-shaké par fonctionnalité** :
si un document n'utilise ni `@for`, ni i18n, ni validation, les fonctions correspondantes
ne sont pas émises. La détection se fait à la compilation via `RuntimeFeatures` (`js_dom.rs`).

Le contrat entre le HTML généré et le runtime passe exclusivement par des
attributs `data-webcore-*` (constantes centralisées dans `src/codegen/attr_names.rs`).

### v2 vs v3 — changement d'architecture

| Aspect | v2.x | v3.0 |
|---|---|---|
| Livraison JS | `webcore.<hash>.js` partagé | `<script>` inline par page |
| Expressions | Strings dans le DOM évaluées par `evalCond` | Fermetures compilées dans `_e` |
| CSP `unsafe-eval` | Requis (nouveau `Function()`) | Non requis — supprimé structurellement |
| Identifiants prod | Lisibles (`bindIf`, `bindFor`, …) | Renommés (`_bi`, `_bf`, …) |
| JS page sans réactivité | Absent | Absent |

---

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

Invariant : **toutes** les fonctions `bind*` s'appuient sur `$effect`, donc un
composant n'est re-rendu que si une dépendance réellement lue a changé.

---

## Expressions compilées — carte `_e` (v3.0)

En v3, chaque expression de binding est compilée au build en une **fermeture JS réelle**
par `compile_read_expr()` (`src/codegen/js/js_events.rs`). Les fermetures sont émises
en tête du `<script>` inline dans une map d'ID :

```js
const _e = {
  e0: () => S.get('count') > 0,
  e1: () => S.get('count') * 2,
  e2: () => S.get('name') + ' World',
};
```

Les attributs `data-webcore-*` stockent l'ID (`e0`, `e1`) au lieu d'une string :

```html
<!-- v2 — expression stockée comme string dans le DOM -->
<div data-webcore-if="count > 0">...</div>
<span data-webcore-interpolation="count * 2"></span>

<!-- v3 — ID vers une fermeture inline -->
<div data-webcore-if="e0">...</div>
<span data-webcore-interpolation="e1"></span>
<script>
const _e={e0:()=>S.get('count')>0, e1:()=>S.get('count')*2};
bindIf(_e); bind(_e);
</script>
```

### `compile_read_expr` — règles de réécriture

Le compilateur réécrit les identifiants de portée `.webc` en accès `State` :

| Pattern source | Code JS dans la fermeture |
|---|---|
| `count` (var d'état) | `S.get('count')` |
| `$store.theme` | `STORE.get('theme')` |
| `$route.slug` | `__route.slug` |
| `$query.q` | `new URLSearchParams(location.search).get('q')` |
| `item.price * qty` | `S.get('item').price * S.get('qty')` |

Conséquence directe : `VARS`, `STORE_VARS`, `_VR`, `VARS_SET` et `evalCond`
**n'existent plus dans le runtime v3**. Une expression qui référence un identifiant
non déclaré dans `state {}` / `props {}` / `computed {}` produit désormais une
**erreur de compilation** (pas un `undefined` silencieux).

### Compilation des méthodes de liste (v2 + v3)

`compile_list_method()` réécrit les appels de mutation réactive **avant** toute
autre transformation :

| Expression source | Code JS émis |
|---|---|
| `items.push(val)` | `S.set('items',[...S.get('items'),val])` |
| `items.remove(i)` | `S.set('items',S.get('items').filter((_,_i)=>_i!==(i)))` |
| `items.clear()` | `S.set('items',[])` |
| `$store.items.push(val)` | `STORE.set('items',[...STORE.get('items'),val])` |

---

## Fonctions de binding (v3)

Les fonctions `bind*` reçoivent la map `_e` en paramètre et appellent directement
`_e[id]()` — aucune évaluation de string.

| Fonction | Signature v3 | Émise si | Rôle |
|---|---|---|---|
| `bind` | `bind(_e)` | interpolation présente | Met à jour `el.textContent` et attributs liés via `_e[id]()`. |
| `bindIf` | `bindIf(_e)` | `@if` présent | Affiche/masque les `div[data-webcore-if]` et leur `else` adjacent ; gère `webc:transition`. |
| `bindFor` | `bindFor()` | `@for` présent | Rend les `<template data-webcore-for>` dans le conteneur adjacent. |
| `bindAttrs` | `bindAttrs(_e)` | attr dynamique présent | `_e[id]()` pour chaque `data-webcore-attr-<name>` / `data-webcore-style-*`. |
| `bindClassBindings` | `bindClassBindings(_e)` | `class:` présent | `classList.toggle(cls, !!_e[id]())` pour chaque `data-webcore-class-*`. |
| `bindValidation` | `bindValidation()` | `validate:` présent | Validation `blur`/`submit` ; messages dans `[data-webcore-error]`. |
| `bindDefer` | `bindDefer()` | `@defer` présent | Révèle le contenu masqué après `DOMContentLoaded`. |

### `DOMContentLoaded` — séquence de rebind v3

```js
document.addEventListener('DOMContentLoaded', () => {
  bind(_e); bindIf(_e); bindFor(); bindAttrs(_e); bindClassBindings(_e);
});
```

En mode `prod`, les noms courts sont utilisés : `_b(_e);_bi(_e);_bf();_ba(_e);_bc(_e);`

### `bindFor` en détail

- `fillItem(el, val, i)` : remplit les spans `data-webcore-interpolation` (chemins
  `item.prop.sub` supportés), écrit `data-webcore-idx`.
- **Diffing par clé** (`@for item key=item.id`) : clé sur `firstElementChild.dataset.webcoreKey` ;
  les éléments existants sont réutilisés.
- **Boucles imbriquées** : `_wc_ctx` propagé aux templates internes.
- Garde-fous : `_wc_b` (anti double-binding), `tmpl.isConnected`.

---

## Délégation d'événements (CSP-safe)

Aucun `onclick=` inline. Chaque élément interactif reçoit un `id` unique
(`<prefix>btn<n>`) et `data-webcore-e="<type>[|mod…]"` ; les handlers compilés
sont dans la map `H[id]`. Un seul listener par type d'événement :

```js
const D = (t, p) => document.addEventListener(t, e => { /* closest('[data-webcore-e]') → H[el.id](e) */ });
```

Modificateurs `on:click|stop|prevent|once|self` encodés dans la valeur de l'attribut.
Liens SPA : `data-webcore-nav` + History API.

---

## Renommage prod (v3.0.5)

Appliqué en post-pass par `rename_runtime_ids()` dans `generate_inline_js` :

```
bindClassBindings → _bc    rebindComputed → _rc    bindValidation → _bv
validateField     → _vf    matchRoute     → _mr    bindAttrs      → _ba
bindDefer         → _bd    bindFor        → _bf    bindIf         → _bi
bind(             → _b(    const bind=    → const _b=
```

L'ordre de remplacement est du plus long au plus court pour éviter les collisions
de sous-chaînes (ex. `bindFor` avant `bind(`).

---

## Référence des attributs `data-webcore-*`

| Attribut | Émis par | Consommé par | v3 : valeur |
|---|---|---|---|
| `data-webcore-if` / `data-webcore-else` | `render_if_element` | `bindIf(_e)`, SSG | ID expression (`e0`) |
| `data-webcore-for`, `-in`, `-for-key`, `-for-index`, `-for-range` | `render_for_element` | `bindFor()` | — |
| `data-webcore-for-container` | `render_for_element` | `bindFor()` | — |
| `data-webcore-interpolation` | `Element::Interpolation` | `bind(_e)`, SSG | ID expression (`e1`) |
| `data-webcore-bound`, `data-webcore-attr-*` | `generate_tag_element` | `bindAttrs(_e)` | ID expression |
| `data-webcore-class-bound`, `data-webcore-class-*` | `handle_class_binding` | `bindClassBindings(_e)` | ID expression |
| `data-webcore-style-*` | `handle_style_binding` | `bindAttrs(_e)` | ID expression |
| `data-webcore-field`, `data-webcore-validate-*`, `data-webcore-error` | validation attrs | `bindValidation()` | — |
| `data-webcore-e` | `handle_event_attr` | délégation `D(t, p)` | — |
| `data-webcore-nav` | liens internes | délégation SPA | — |
| `data-webcore-ref` | `handle_ref_attr` | `refs[name]` | — |
| `data-webcore-transition` | `webc:transition` | `bindIf(_e)` | — |
| `data-webcore-defer` | critical CSS (prod) | swap `media` à DCL | — |
| `data-v` | scoping CSS | sélecteurs CSS scopés (pas de JS) | — |
| `data-webcore-key`, `data-webcore-idx`, `data-webcore-onced` | **runtime** | runtime uniquement | — |

---

## Cycle de vie d'une page (v3)

1. **HTML parsé** — le `<script>` inline en fin de `<body>` est exécuté.
2. **Déclarations** : classe `State`, `const S`, `const STORE`, map `_e`, handlers `H`,
   fonctions `bind*`, délégation `D`.
3. **`DOMContentLoaded`** : `bind(_e)`, `bindIf(_e)`, `bindFor()`, `bindAttrs(_e)`,
   `bindClassBindings(_e)`, `bindValidation()`, `bindDefer()`, router (si SPA),
   `on:mount` des composants.
4. **Navigation SPA** : `on:destroy` des composants quittés, puis `bind(_e)` + rebind
   séquence sur la nouvelle page.

Le SSG (`src/core/ssg.rs`) pré-remplit interpolations et `display` initiaux dans
le HTML ; le runtime ré-écrit ces valeurs au premier bind, les deux mécanismes
étant indépendants mais cohérents.
