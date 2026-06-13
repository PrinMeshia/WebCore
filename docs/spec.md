# Spécification du langage WebCore

> Version : 2.6.0 — Référence complète de la syntaxe `.webc`

---

## Sommaire

1. [Structure d'un projet](#structure-dun-projet)
2. [Configuration (`webc.toml`)](#configuration-webctoml)
3. [Application (`app`)](#application-app)
4. [Store global](#store-global)
5. [Layouts](#layouts)
6. [Pages](#pages)
7. [Composants](#composants)
   - [Props](#props)
   - [State](#state)
   - [Computed — État dérivé](#computed--état-dérivé)
   - [View](#view)
   - [Style](#style)
8. [Lifecycle hooks](#lifecycle-hooks)
   - [`on:mount`](#onmount)
   - [`on:destroy`](#ondestroy)
9. [Événements inter-composants](#événements-inter-composants)
10. [Éléments HTML](#éléments-html)
11. [Attributs](#attributs)
12. [Événements](#événements)
13. [Interpolation](#interpolation)
14. [Directives de contrôle](#directives-de-contrôle)
15. [Routage](#routage)
16. [Slots](#slots)
17. [Thème (`theme.toml`)](#thème-themetoml)
18. [Sortie compilée](#sortie-compilée)
19. [Validation de formulaires](#validation-de-formulaires)
20. [Internationalisation (i18n)](#internationalisation-i18n)
21. [SSG — Static Site Generation](#ssg--static-site-generation)
22. [WebAssembly (WASM)](#webassembly-wasm)
23. [Bloc `http { }` — Requêtes HTTP déclaratives](#bloc-http----requêtes-http-déclaratives)
24. [Bloc `head { }` — Personnalisation du `<head>`](#bloc-head----personnalisation-du-head)
25. [`$query.` — Paramètres query string](#query--paramètres-query-string)
26. [`class:name` — Classes conditionnelles](#classname--classes-conditionnelles)
27. [`on:event|debounce` — Handlers debouncés](#oneventdebounce--handlers-debouncés)
28. [Commande `webc check`](#commande-webc-check)
29. [`ref:name` — Références DOM directes](#refname--références-dom-directes)
30. [`style:prop` — Styles inline dynamiques](#styleprop--styles-inline-dynamiques)
31. [Contenu par défaut des slots](#contenu-par-défaut-des-slots)
32. [`webc:transition` — Animations CSS](#webctransition--animations-css)
33. [`webc:img` — Images optimisées](#webcimg--images-optimisées)
34. [Fingerprinting des images](#fingerprinting-des-images)
35. [Nouveautés v1.1.1](#nouveautés-v111)
36. [Nouveautés v1.2.0](#nouveautés-v120)
37. [Nouveautés v1.3.0](#nouveautés-v130)
38. [Nouveautés v1.4.0](#nouveautés-v140)
39. [Nouveautés v1.5.0](#nouveautés-v150)
40. [Nouveautés v2.0.0](#nouveautés-v200)
41. [Signaux réactifs fins](#signaux-réactifs-fins)
42. [CSS nesting](#css-nesting)
43. [Rapport d'analyse du bundle](#rapport-danalyse-du-bundle)
44. [Nouveautés v2.1.0](#nouveautés-v210)
45. [Nouveautés v2.2.0](#nouveautés-v220)
46. [Nouveautés v2.3.0](#nouveautés-v230)
47. [Nouveautés v2.4.0](#nouveautés-v240)
48. [Nouveautés v2.5.0](#nouveautés-v250)
49. [Nouveautés v2.5.1](#nouveautés-v251)
50. [Nouveautés v2.5.2](#nouveautés-v252)
51. [`<>...</>` — Fragment shorthand](#--fragment-shorthand)
52. [`on:event|stop|prevent|once|self` — Modificateurs d'événements](#oneventstoppreventselfonce--modificateurs-dévénements)
53. [Props — Valeurs par défaut](#props--valeurs-par-défaut)
54. [Commande `webc watch`](#commande-webc-watch)
55. [Nouveautés v2.6.0](#nouveautés-v260)
56. [Limites actuelles (v2.6.0)](#limites-actuelles-v260)

---

## Structure d'un projet

```
mon-app/
├── webc.toml              # Configuration du projet
├── theme.toml             # Tokens de design (optionnel)
├── locales/               # Traductions i18n (optionnel)
│   ├── fr.toml
│   └── en.toml
├── wasm/                  # Module Rust/WASM (optionnel)
│   ├── Cargo.toml
│   └── src/lib.rs
├── src/
│   ├── app.webc           # Déclaration de l'application
│   ├── layouts/           # Layouts (un fichier par layout)
│   │   └── MainLayout.webc
│   ├── pages/             # Pages (une par route)
│   │   ├── home.webc
│   │   └── about.webc
│   └── components/        # Composants réutilisables
│       └── Counter.webc
└── public/                # Assets statiques copiés tel quel dans dist/
```

---

## Configuration (`webc.toml`)

```toml
[app]
title  = "Mon Application"   # Titre HTML des pages
lang   = "fr"                # Attribut lang de <html>
mode   = "dev"               # "dev" ou "prod" (active la minification en prod)
locale = "fr"                # Locale de rendu par défaut (optionnel, hérite de lang)
```

---

## Application (`app`)

Déclare la configuration globale : layout par défaut et table de routage.

```webc
app MonApp {
    theme: "default"       // nom du thème (réservé pour usage futur)
    layout: MainLayout     // layout utilisé pour toutes les pages
    routes {
        "/": HomePage      // "/" → page ou composant "HomePage"
        "/about": AboutPage
        "/contact": ContactPage
    }
}
```

---

## Store global

Le store est un état réactif **partagé entre tous les composants** du projet.
Il se déclare avec le bloc `store` au niveau document (généralement dans `src/app.webc`).

```webc
store {
    count:  Number  = 0
    theme:  String  = "dark"
    active: Boolean = false
    items:  List    = []
}
```

La syntaxe est identique à `state { ... }` (voir [State](#state)).

### Accès aux variables du store

Les variables du store sont référencées avec le préfixe `$store.` :

```webc
component Counter {
    view {
        div {
            p "Total global : {$store.count}"
            button on:click={$store.count += 1} { "+" }
            button on:click={$store.count = 0}  { "Reset" }
        }
    }
}

component Display {
    view {
        p "Valeur courante : {$store.count}"
    }
}
```

Les deux composants réagissent automatiquement aux changements du store.

### Expressions supportées

| Expression | Description |
|---|---|
| `$store.count += 1` | Incrément |
| `$store.count -= 1` | Décrément |
| `$store.count *= 2` | Multiplication |
| `$store.count = 0` | Affectation |
| `$store.theme = "light"` | Affectation string |
| `{$store.count}` | Interpolation |
| `@if $store.active { ... }` | Condition sur store |
| `@for item in $store.items { ... }` | Boucle sur liste du store |

### Mix store et state local

```webc
component TodoCounter {
    state {
        label: String = "Tâches"
    }
    view {
        p "{label} : {$store.count}"
    }
}
```

---

## Layouts

Un layout définit la structure commune à toutes les pages (header, nav, footer…).
Il utilise `slot content` pour marquer où le contenu de la page sera injecté.

```webc
layout MainLayout {
    header {
        nav {
            link to="/" { "Accueil" }
            link to="/blog" { "Blog" }
        }
    }
    main { slot content }
    footer {
        p "© 2025 Mon App"
    }
}
```

### Layouts multi-zones (named slots)

Un layout peut déclarer plusieurs zones nommées avec `slot <nom>` :

```webc
layout DashLayout {
    div {
        header { slot header }
        aside  { slot sidebar }
        main   { slot content }
    }
}
```

Voir [Slots](#slots) pour la description complète.

---

## Pages

Une page est associée à une route via la table `routes` dans `app`.
Son contenu remplace `slot content` dans le layout.

```webc
page "home" {
    h1 "Bienvenue !"
    p "Voici la page d'accueil."
}
```

Le nom de la page (ici `"home"`) est une chaîne de caractères.

---

## Composants

Les composants sont des blocs réutilisables avec état local, vue et styles scopés.

```webc
component NomComposant {
    props    { ... }    // optionnel
    state    { ... }    // optionnel
    computed { ... }    // optionnel
    on:mount { ... }    // optionnel
    on:destroy { ... }  // optionnel
    view     { ... }    // obligatoire
    style    { ... }    // optionnel
}
```

Le nom doit commencer par une majuscule. Un composant s'utilise comme un tag HTML :

```webc
NomComposant {}
NomComposant prop1="valeur" {}
NomComposant prop1={expr} {}
```

### Props

Les props permettent de passer des valeurs à un composant à l'instanciation.
Elles acceptent des **chaînes statiques** ou des **expressions dynamiques**.

```webc
component Badge {
    props {
        label: String
        color: String
    }
    view {
        span class={color} "Statut : {label}"
    }
}
```

Utilisation :

```webc
// Prop statique
Badge label="Actif" color="green" {}

// Prop dynamique (expression réactive)
Badge label={statusMsg} color={statusColor} {}
```

**Valeurs par défaut** : chaque prop peut avoir une valeur par défaut utilisée si la prop est omise à l'instanciation :

```webc
component Badge {
    props {
        label: String = "N/A"
        color: String = "gray"
    }
    view {
        span class={color} "Statut : {label}"
    }
}

// Utilisation sans props → valeurs par défaut injectées
Badge {}          // → label="N/A", color="gray"
Badge label="OK"  // → color="gray" (défaut)
Badge label="OK" color="green" {}  // → override complet
```

La valeur par défaut est injectée **statiquement** à la compilation — elle n'est pas évaluée au runtime.

**Props statiques** (`name="valeur"`) : substituées statiquement à la compilation.  
**Props dynamiques** (`name={expr}`) : `{propName}` dans la vue devient un span réactif
`data-webcore-interpolation` évalué à la même expression que la prop passée.

### State

L'état local d'un composant est réactif : tout changement met à jour le DOM automatiquement.

```webc
state {
    count:  Number  = 0
    name:   String  = "World"
    active: Boolean = true
    items:  List    = []
}
```

Syntaxe : `nomVar : Type = valeurParDéfaut`

| Type | Valeur par défaut si omise |
|---|---|
| `Number` | `0` |
| `String` | `""` |
| `Boolean` | `false` |
| `List` | `[]` |

### Computed — État dérivé

Le bloc `computed` déclare des variables dérivées du state local. Elles sont
réévaluées automatiquement **avant chaque bind DOM** via `rebindComputed()`.

```webc
component FullName {
    state {
        firstName: String = "Jean"
        lastName:  String = "Dupont"
    }
    computed {
        fullName = firstName + " " + lastName
    }
    view {
        p "Bonjour {fullName}"
    }
}
```

- Les variables computed utilisent `S.setQ(k, v)` — un setter **silencieux** qui met à jour la
  valeur sans déclencher les listeners, évitant ainsi les boucles réactives.
- Les expressions computed supportent les mêmes opérations que les interpolations (`+`, `-`, `*`,
  `/`, `max()`, `min()`, etc.).
- Une variable computed peut être utilisée dans `view`, `@if`, `@for` et `style`.
- Plusieurs variables computed peuvent être déclarées dans le même bloc.

```webc
computed {
    fullName  = firstName + " " + lastName
    initials  = firstName + "." + lastName
    charCount = firstName + lastName
}
```

**Ordre d'exécution :** `rebindComputed()` est toujours appelé en premier dans `bind()`,
garantissant que les valeurs dérivées sont à jour avant tout bind.

### View

La vue définit l'arbre HTML du composant. Elle supporte tous les éléments HTML,
l'interpolation, les directives de contrôle et l'imbrication de composants.

```webc
view {
    div {
        p "Valeur : {count}"
        button on:click={count += 1} { "+" }
        @if count > 5 { p "Grand !" }
    }
}
```

### Style

Les styles sont scopés au composant via un attribut `data-v` unique (hash FNV-1a).

```webc
style {
    div    { display: flex; gap: 1rem; }
    button { padding: 0.5rem 1rem; border-radius: 4px; }
    p.large { font-size: 2rem; }
}
```

**Media queries responsives** sont supportées directement dans le bloc `style` :

```webc
style {
    div    { display: flex; gap: 1rem; align-items: center; }
    button { padding: 0.25rem 0.75rem; }
    @media (max-width: 480px) {
        div { flex-direction: column; }
    }
}
```

Le scoping CSS (`data-v`) est automatiquement propagé à l'intérieur des blocs `@media`.

Les sélecteurs `*`, `:root`, `html`, `body` ne sont **pas** scopés (globaux).

**Sélecteurs multi-éléments (virgule)** : un même bloc de règles peut cibler plusieurs éléments :

```webc
style {
    input, textarea {
        padding: 0.5rem;
        border: 1px solid #334155;
        border-radius: 6px;
    }
    input:focus, textarea:focus { border-color: #3b82f6; }
}
```

---

## Lifecycle hooks

Les hooks de cycle de vie permettent d'exécuter du code JS brut à des moments précis
du cycle de vie du composant.

### `on:mount`

S'exécute une fois dans `DOMContentLoaded`, après l'initialisation complète du runtime.
Le corps est wrappé dans un IIFE pour isoler les variables locales.

```webc
component Timer {
    state {
        elapsed: Number = 0
    }
    on:mount {
        const id = setInterval(() => {
            S.set("elapsed", S.get("elapsed") + 1);
            bind();
        }, 1000);
        window.__timerId = id;
    }
    view {
        p "Temps écoulé : {elapsed}s"
    }
}
```

- L'accès au state se fait via `S.get("varName")` / `S.set("varName", value)` en JS brut.
- `bind()` doit être appelé manuellement pour mettre à jour le DOM après un `S.set`.
- Plusieurs composants avec `on:mount` voient leurs corps exécutés dans l'ordre d'apparition.
- Le corps `on:mount { }` supporte les accolades imbriquées à **profondeur arbitraire** — les callbacks JS complexes (`setTimeout`, `setInterval`, `addEventListener` avec corps multi-ligne, objets littéraux) sont entièrement supportés.

### `on:destroy`

S'exécute **avant chaque navigation SPA** (`nav()`) et sur l'événement `window.beforeunload`
(fermeture ou rechargement de l'onglet).

```webc
component Timer {
    state { elapsed: Number = 0 }
    on:mount {
        window.__timerId = setInterval(() => {
            S.set("elapsed", S.get("elapsed") + 1);
            bind();
        }, 1000);
    }
    on:destroy {
        clearInterval(window.__timerId);
    }
    view {
        p "Temps : {elapsed}s"
    }
}
```

**Utilisation typique :** nettoyage de timers, annulation de requêtes, désenregistrement de listeners.

Le runtime génère :
```js
const DESTROY_HOOKS = [
    () => { clearInterval(window.__timerId); }
];
function runDestroyHooks() { DESTROY_HOOKS.forEach(h => h()); }
```

`runDestroyHooks()` est appelé en tête de `nav()` et dans `window.addEventListener('beforeunload', ...)`.

---

## Événements inter-composants

WebCore fournit un mécanisme de communication entre composants via des événements DOM personnalisés.

### Émettre un événement — `emit()`

`emit("eventName")` ou `emit("eventName", data)` dans une expression d'événement :

```webc
component Notifier {
    view {
        button on:click={emit("ping", count)} { "Ping" }
        button on:click={emit("reset")} { "Reset" }
    }
}
```

Compilé vers :

```js
document.dispatchEvent(new CustomEvent("ping", { detail: count }))
document.dispatchEvent(new CustomEvent("reset"))
```

### Écouter un événement — `on:eventName`

Sur un appel de composant, `on:eventName={handler}` enregistre un listener global :

```webc
page "home" {
    Notifier on:ping={count += 1} on:reset={count = 0} {}
    p "Reçu : {count} pings"
}
```

Compilé vers un `document.addEventListener('ping', e => { ... })` enregistré dans `DOMContentLoaded`.

### Données de l'événement

Les données passées à `emit("event", data)` sont accessibles via `e.detail` dans le handler :

```webc
// Émetteur
component Slider {
    state { value: Number = 50 }
    view {
        input type="range" on:input={emit("slide", value)} {}
    }
}

// Récepteur
page "home" {
    Slider on:slide={level = e.detail} {}
    p "Niveau : {level}"
}
```

### Portée

Les événements sont dispatché sur `document` — ils sont **globaux**. Si plusieurs instances
d'un composant émettent le même événement, tous les listeners le reçoivent.

---

## Lifecycle hooks

Les hooks de cycle de vie permettent d'exécuter du code JS brut à des moments précis
du cycle de vie du composant.

### `on:mount`

S'exécute une fois dans `DOMContentLoaded`, après l'initialisation complète du runtime.
Le corps est wrappé dans un IIFE pour isoler les variables locales.

```webc
component Timer {
    state {
        elapsed: Number = 0
    }
    on:mount {
        const id = setInterval(() => {
            S.set("elapsed", S.get("elapsed") + 1);
            bind();
        }, 1000);
        window.__timerId = id;
    }
    view {
        p "Temps écoulé : {elapsed}s"
    }
}
```

- L'accès au state se fait via `S.get("varName")` / `S.set("varName", value)` en JS brut.
- `bind()` doit être appelé manuellement pour mettre à jour le DOM après un `S.set`.
- Plusieurs composants avec `on:mount` voient leurs corps exécutés dans l'ordre d'apparition.

### `on:destroy`

S'exécute **avant chaque navigation SPA** (`nav()`) et sur l'événement `window.beforeunload`
(fermeture ou rechargement de l'onglet).

```webc
component Timer {
    state { elapsed: Number = 0 }
    on:mount {
        window.__timerId = setInterval(() => {
            S.set("elapsed", S.get("elapsed") + 1);
            bind();
        }, 1000);
    }
    on:destroy {
        clearInterval(window.__timerId);
    }
    view {
        p "Temps : {elapsed}s"
    }
}
```

**Utilisation typique :** nettoyage de timers, annulation de requêtes, désenregistrement de listeners.

Le runtime génère :
```js
const DESTROY_HOOKS = [
    () => { clearInterval(window.__timerId); }
];
function runDestroyHooks() { DESTROY_HOOKS.forEach(h => h()); }
```


`runDestroyHooks()` est appelé en tête de `nav()` et dans `window.addEventListener('beforeunload', ...)`.

---

## Éléments HTML

Tout tag HTML standard est supporté directement :

```webc
div {
    h1 "Titre"
    p "Paragraphe"
    span "Texte inline"
    img src="/logo.png" alt="Logo"
    ul {
        li "Item 1"
        li "Item 2"
    }
}
```

### Contenu mixte

Un même bloc peut mélanger texte et éléments enfants :

```webc
p { "Bonjour " strong { "le monde" } " !" }
```

### Élément `link` → `<a>`

`link` est un alias pour `<a>` avec gestion automatique du routage SPA :

```webc
link to="/about" { "À propos" }   // → <a href="/about">À propos</a>
link href="https://example.com" { "Externe" }
```

---

## `<>...</>` — Fragment shorthand

Un **fragment** groupe plusieurs éléments sans élément DOM wrapper. Il est utile pour rendre plusieurs nœuds adjacents là où un seul élément est attendu.

### Syntaxe

```webc
<>
    p "Premier"
    p "Deuxième"
</>
```

### Compilation

Le compilateur rend les éléments du fragment **inline**, sans aucune balise wrapper :

```html
<!-- Source .webc -->
<>
    p "A"
    p "B"
</>

<!-- HTML généré — pas de div intermédiaire -->
<p>A</p>
<p>B</p>
```

### Cas d'usage

```webc
// Plusieurs éléments à la racine d'une vue
component CardContent {
    view {
        <>
            h2 "Titre"
            p "Description"
            span class="badge" { "Nouveau" }
        </>
    }
}

// Dans un @if — évite un wrapper inutile
@if isLoggedIn {
    <>
        p "Bienvenue !"
        button on:click={logout} { "Déconnexion" }
    </>
}

// Dans un slot
page "home" {
    slot sidebar {
        <>
            nav { link to="/a" { "A" } }
            nav { link to="/b" { "B" } }
        </>
    }
}
```

### Imbrication

Les fragments peuvent contenir n'importe quel élément, directive de contrôle ou composant, y compris d'autres fragments.

---

## Attributs

### Attribut statique (chaîne)

```webc
div class="container" id="main" { }
img src="/logo.png" alt="Logo"
```

### Attribut booléen

```webc
input disabled
input required
```

### Attribut dynamique (expression)

```webc
div class={dynamicClass} { }
input value={count} type="number"
```

Les attributs dynamiques sont évalués au runtime via `bindAttrs()`.

### `bind:` — liaison bidirectionnelle

`bind:attr={var}` est un raccourci qui génère simultanément l'attribut de valeur et le handler de mise à jour :

```webc
input bind:value={name}       // → value={name} + on:input={name = event.target.value}
input bind:checked={accepted} // → checked={accepted} + on:change={accepted = event.target.checked}
```

Équivalent développé :

```webc
// Sans bind: — version longue
input value={name} on:input={name = event.target.value}

// Avec bind: — version courte
input bind:value={name}
```

`bind:value` utilise l'événement `on:input` (mise à jour à chaque frappe).
`bind:checked` utilise `on:change` (mise à jour au changement d'état de la case).

---

## Événements

Les événements utilisent le préfixe `on:` suivi du type d'événement.

```webc
button on:click={count += 1} { "+" }
form   on:submit={handleSubmit} { ... }
input  on:input={value = event.target.value} { }
select on:change={selected = event.target.value} { }
```

### Expressions d'événements

```webc
// Affectation simple
button on:click={count = 0} { "Reset" }

// Opérateurs composés
button on:click={count += 1} { "+" }
button on:click={count -= 1} { "−" }
button on:click={count *= 2} { "×2" }

// Expression avec fonctions
button on:click={count = max(0, count - 1)} { "−" }
button on:click={count = min(100, count + 1)} { "+" }

// Émission d'événement inter-composants
button on:click={emit("ping", count)} { "Ping" }

// Navigation
button on:click={webcore_navigate(/about)} { "Aller à propos" }
```

### Handlers multi-instructions

Plusieurs instructions peuvent être séparées par `;` dans un même handler. Chacune est compilée indépendamment :

```webc
// Ajouter un item et vider le champ de saisie
button on:click={items = [...(items ?? []), newItem]; draft = ""} { "Ajouter" }

// Réinitialiser plusieurs variables
button on:click={count = 0; label = "Nouveau"} { "Reset" }
```

Chaque instruction du style `var = expr` ou `var += expr` est compilée en un `S.set(...)` distinct. L'expression RHS ne doit pas contenir de `;` littéral.

### Fonctions utilitaires disponibles dans les expressions

| Fonction | Description |
|---|---|
| `max(a, b)` | Maximum de a et b |
| `min(a, b)` | Minimum de a et b |
| `abs(x)` | Valeur absolue |
| `emit("event")` | Émet un CustomEvent sur `document` |
| `emit("event", data)` | Émet un CustomEvent avec données |

---

## Interpolation

### Dans les chaînes

```webc
p "Bonjour {name} !"
p "Total : {count + tax}"
p "Max : {max(a, b)}"
p "Résultat : {a * b + c}"
p "Complet : {fullName}"     // variable computed
p "Traduction : {t("key")}"  // i18n
```

### Expression arbitraire

L'expression entre `{` et `}` est évaluée au runtime. Elle peut référencer
n'importe quelle variable du state, variable computed, variable store, et appeler `max`, `min`, `abs`.

```webc
p "Pair : {count % 2 == 0}"
p "Catégorie : {count > 10 ? 'grand' : 'petit'}"
```

---

## Directives de contrôle

### `@if` / `@else`

```webc
@if condition {
    // contenu si vrai
} @else {
    // contenu si faux (optionnel)
}
```

Exemples :

```webc
@if count > 0 {
    p "Positif"
} @else {
    p "Zéro ou négatif"
}

@if logged_in {
    button on:click={logout} { "Déconnexion" }
} @else {
    link to="/login" { "Se connecter" }
}
```

La condition est évaluée au runtime et réactive (mise à jour si une variable du state change).

### `@for`

```webc
@for item in items {
    li "{item}"
}
```

`items` doit être une variable du state de type `List`.
`item` est la variable locale représentant l'élément courant.

```webc
component TaskList {
    state {
        tasks: List = []
    }

    view {
        ul {
            @for task in tasks {
                li "{task}"
            }
        }
    }
}
```

### `@for` avec index

La seconde variable optionnelle dans `@for item, i in list` reçoit l'index (0-based) de l'élément courant :

```webc
@for item, i in items {
    li "{i}. {item}"
}
```

```webc
component Ranking {
    state { scores: List = [] }
    view {
        ol {
            @for entry, rank in scores {
                li "#{rank + 1} — {entry.name} : {entry.score}"
            }
        }
    }
}
```

### `@switch` / `@case` / `@default`

La directive `@switch` est une alternative lisible à une chaîne `@if`/`@else` :

```webc
@switch status {
    @case "active"   { span class="badge green"  "Active" }
    @case "pending"  { span class="badge yellow" "Pending" }
    @case "archived" { span class="badge gray"   "Archived" }
    @default         { span class="badge"        "Unknown" }
}
```

- L'expression après `@switch` est comparée avec `===` à la valeur de chaque `@case`.
- Le bloc `@default` est facultatif. S'il est absent et qu'aucun `@case` ne correspond, rien n'est rendu.
- Compilée en chaîne `@if`/`@else` par le parser — aucun overhead runtime.

```webc
// Avec une expression complexe
@switch count % 3 {
    @case 0 { p "Divisible par 3" }
    @case 1 { p "Reste 1" }
    @case 2 { p "Reste 2" }
}
```

### `@for` avec des items objets

Quand les items de la liste sont des **objets**, les propriétés sont accessibles via la notation pointée dans les interpolations :

```webc
component TodoList {
    state { items: List }
    on:mount {
        S.set('items', [
            {text: "Acheter des courses", done: false},
            {text: "Lire un livre",       done: true}
        ])
    }
    view {
        @for item in items {
            li "{item.text}"
        }
    }
}
```

Avec key pour le DOM diffing :

```webc
@for item key=item.text in items {
    li "{item.text}"
}
```

> **Pattern `window.helper`** : la grammaire interdit les accolades dans les expressions `on:click`. Pour créer des objets, définir un helper dans `on:mount` :
> ```webc
> on:mount { window.mkItem = text => ({text, done: false}) }
> // Puis dans la vue :
> button on:click={items = [...(items ?? []), mkItem(draft)]; draft = ""} { "Ajouter" }
> ```

---

## Routage

WebCore génère un mode multi-pages (un fichier HTML par page) avec navigation SPA.

### Déclaration des routes

```webc
app MonApp {
    routes {
        "/": HomePage
        "/about": AboutPage
        "/contact": ContactPage
    }
}
```

### Navigation programmatique

```webc
button on:click={webcore_navigate(/about)} { "À propos" }
button on:click={webcore_navigate(root)} { "Accueil" }   // "/"
button on:click={webcore_navigate("/contact")} { "Contact" }
```

### Lien de navigation

```webc
link to="/about" { "À propos" }
```

La navigation SPA utilise `history.pushState` + `fetch` pour charger les pages
sans rechargement complet. Les URL restent propres (`/about`, non `/about.html`).

### `on:destroy` et navigation

Avant chaque navigation SPA, `runDestroyHooks()` est exécuté automatiquement pour
nettoyer les ressources de la page courante (voir [on:destroy](#ondestroy)).

---

## Slots

### Slot unique (défaut)

`slot content` est le point d'injection du contenu de la page dans un layout.

```webc
layout MainLayout {
    main { slot content }  // contenu de la page ici
}
```

### Named slots (multi-zones)

Un layout peut définir plusieurs zones nommées. Chaque `slot <nom>` est remplacé par le
contenu correspondant fourni par la page.

```webc
layout DashLayout {
    header { slot header }
    aside  { slot sidebar }
    main   { slot content }
}
```

La page fournit le contenu avec la syntaxe `slot <nom> { ... }` :

```webc
page "dashboard" {
    slot header  { h1 "Dashboard" }
    slot sidebar { nav { link to="/" "Accueil" } }
    p "Contenu principal"   // → slot content par défaut
}
```

**Règles :**
- Les éléments de la page sans `slot name { }` explicite alimentent le slot `content`.
- Un slot sans contenu fourni par la page est simplement vide (aucune erreur).
- La résolution est récursive — fonctionne à n'importe quelle profondeur dans l'arbre du layout.
- Rétrocompatibilité totale : `main { slot content }` fonctionne comme avant.

---

## Thème (`theme.toml`)

Définit des tokens de design exportés comme variables CSS dans `dist/theme.css`.

```toml
[colors]
primary    = "#4F46E5"
background = "#FFFFFF"
text       = "#1F2937"

[fonts]
sans = "system-ui, sans-serif"

[spacing]
sm = "0.5rem"
md = "1rem"
lg = "2rem"
```

Sortie CSS générée :

```css
:root {
  --color-primary: #4F46E5;
  --color-background: #FFFFFF;
  --color-text: #1F2937;
  --font-sans: system-ui, sans-serif;
  --spacing-sm: 0.5rem;
  --spacing-md: 1rem;
  --spacing-lg: 2rem;
}
```

Utilisation dans les styles d'un composant :

```webc
style {
    h1 { color: var(--color-primary); }
    p  { font-family: var(--font-sans); margin: var(--spacing-md) 0; }
}
```

---

## Sortie compilée

```
dist/
├── index.html          # Page "home" avec layout
├── about/
│   └── index.html      # Page "about" — URL propre : /about
├── assets/
│   ├── theme.css       # Variables CSS + styles scopés des composants
│   ├── webcore.js      # Runtime réactif (état, routage, événements)
│   └── logo.png        # Assets publics (copiés depuis public/)
```

Les pages sont générées dans `slug/index.html` (URLs propres sans `.html`).
Les assets (JS, CSS, images) sont isolés dans `dist/assets/` ; les chemins sont absolus (`/assets/theme.css`)
pour que les pages de sous-dossiers résolvent correctement leurs ressources.

`webc build` affiche un récapitulatif de l'arborescence générée avec les tailles de fichiers.

### Runtime JS (`webcore.js`)

Le runtime généré contient uniquement les fonctions **réellement utilisées** par le document
(tree-shaking automatique) :

| Fonction / Constante | Émise si… | Description |
|---|---|---|
| `class State` | Toujours | État réactif avec `S.get`, `S.set`, `S.setQ`, `S.on` |
| `COMPUTED` + `rebindComputed()` | `computed {}` présent | Variables dérivées |
| `evalCond(expr)` | Interpolation, `@if`, `@for`, ou attributs dynamiques | Évaluation sécurisée des expressions |
| `bind()` | Interpolation ou `computed {}` | Mise à jour des `data-webcore-interpolation` |
| `bindIf()` | `@if` présent | Réactivité pour `@if`/`@else` |
| `bindFor()` | `@for` présent | Réactivité pour `@for` |
| `bindAttrs()` | Attribut dynamique présent | Réactivité pour `class={expr}` |
| `validateField()` + `bindValidation()` | `validate:*` ou `@error` présent | Validation de formulaires |
| `VARS` + `STORE_VARS` | Au moins une fonction de bind réactive | Tableaux de noms de variables |
| `nav()` + `toFile()` + `popstate` | Routes déclarées ou `webcore_navigate()` | Navigation SPA |
| `DESTROY_HOOKS` + `runDestroyHooks()` | `on:destroy {}` présent | Nettoyage avant navigation |
| `LOCALES` + `t()` + `setLocale()` | `locales/` présents | Internationalisation |
| Loader WASM | `wasm/Cargo.toml` présent | Module WebAssembly |

**Exemple pour un composant simple sans `@for`, `@if`, ni validation :**

```js
class State { #d=new Map(); #l=new Map();
  set(k,v){ this.#d.set(k,v); this.#l.get(k)?.forEach(f=>f(v)) }
  setQ(k,v){ this.#d.set(k,v) }
  get(k){ return this.#d.get(k) }
  on(k,f){ (this.#l.get(k)??this.#l.set(k,[]).get(k)).push(f) }
}
const S = new State();
// evalCond, bind — puis DOMContentLoaded
```

`bindFor`, `bindIf`, `bindAttrs`, `nav`, etc. sont absents — overhead zéro pour les apps simples.

### Mode prod

Avec `mode = "prod"` dans `webc.toml` :
- CSS minifié par **LightningCSS**
- JS minifié (suppression des commentaires + compactage)
- Cible : navigateurs Chrome 90+, Firefox 88+, Safari 14+

---

## Validation de formulaires

La validation déclarative s'applique sur les éléments `input`, `textarea` et `select` directement dans la vue.

### Attributs de validation

| Attribut | Exemple | Description |
|---|---|---|
| `validate:required` | `validate:required="Champ requis"` | Champ obligatoire |
| `validate:minlength` | `validate:minlength="3,Min 3 chars"` | Longueur minimale |
| `validate:maxlength` | `validate:maxlength="50,Max 50 chars"` | Longueur maximale |
| `validate:email` | `validate:email="Email invalide"` | Format email |
| `validate:pattern` | `validate:pattern="^[A-Z]+$,Majuscules uniquement"` | Regex personnalisée |

Pour `minlength`, `maxlength` et `pattern`, la valeur est `contrainte,message` (le message est optionnel).

L'attribut `name` de l'input **doit être présent** pour associer les erreurs de validation.

### Directive `@error`

Affiche un bloc uniquement si le champ correspondant est invalide :

```webc
@error "nomDuChamp" {
    // contenu affiché si le champ est invalide
}
```

### Exemple complet

```webc
component ContactForm {
    view {
        form on:submit={handleSubmit} {
            div {
                label "Nom"
                input type="text" name="name"
                      validate:required="Le nom est requis"
                      validate:minlength="2,Au moins 2 caractères"
                @error "name" { span class="error" "Erreur" }
            }
            div {
                label "Email"
                input type="email" name="email"
                      validate:required="L'email est requis"
                      validate:email="Adresse email invalide"
                @error "email" { span class="error" "Erreur" }
            }
            button type="submit" { "Envoyer" }
        }
    }
}
```

### Comportement runtime

- La validation se déclenche au **blur** (sortie du champ) et à la **soumission du formulaire**
- Le listener de soumission tourne en **phase de capture** avec `stopImmediatePropagation()` — il s'exécute avant tout handler `on:submit` inline ; si la validation échoue, la soumission est bloquée avant d'atteindre le handler
- Si la validation échoue, `e.preventDefault()` empêche la soumission
- Les blocs `@error` sont masqués (`display:none`) par défaut ; le runtime injecte le message d'erreur dans le **premier enfant** du bloc (le texte de l'élément enfant est remplacé, la structure reste intacte)
- La validation au blur ne s'active qu'après la première interaction de l'utilisateur (évite les erreurs prématurées)

---

## Internationalisation (i18n)

### Fichiers de traduction

Créer un fichier TOML par langue dans le répertoire `locales/` du projet :

```
mon-app/
└── locales/
    ├── fr.toml
    └── en.toml
```

Chaque fichier est un dictionnaire plat `clé = "valeur"` :

```toml
# locales/fr.toml
welcome   = "Bienvenue"
counter   = "Compteur"
increment = "Incrémenter"
```

```toml
# locales/en.toml
welcome   = "Welcome"
counter   = "Counter"
increment = "Increment"
```

### Configuration

Déclarer la locale par défaut dans `webc.toml` :

```toml
[app]
title  = "Mon Application"
lang   = "fr"
locale = "fr"    # locale de rendu par défaut (optionnel, hérite de lang si omis)
```

### Utilisation dans les vues

La fonction `t("clé")` retourne la traduction de la locale active :

```webc
component Header {
    view {
        h1 "{t("welcome")}"
        p  "{t("counter")}: {count}"
        button on:click={count += 1} { "{t("increment")}" }
    }
}
```

### Changer de locale au runtime

`setLocale(code)` bascule la locale active et rebind toutes les directives réactives :

```webc
page "home" {
    button on:click={setLocale("fr")} { "FR" }
    button on:click={setLocale("en")} { "EN" }
    h1 "{t("welcome")}"
}
```

### Runtime généré

Quand le projet contient des locales, le compilateur injecte dans `webcore.js` :

```js
const LOCALES = {
    "en": { "counter": "Counter", "welcome": "Welcome" },
    "fr": { "counter": "Compteur", "welcome": "Bienvenue" }
};
let LOCALE = "fr";
const t = (k, a) => {
  if (a === undefined) return LOCALES[LOCALE]?.[k] ?? k;
  if (typeof a === 'number') {
    const pk = a === 1 ? k + '_one' : k + '_other';
    return (LOCALES[LOCALE]?.[pk] ?? LOCALES[LOCALE]?.[k] ?? k).replace(/\{\{count\}\}/g, String(a));
  }
  return (LOCALES[LOCALE]?.[k] ?? k).replace(/\{\{0\}\}/g, String(a));
};
const setLocale = l => { if (LOCALES[l]) { LOCALE = l; bind(); bindIf(); bindFor(); bindAttrs() } };
```

`setLocale` est exposé dans `globalThis`.

### Fallback

Si une clé est absente de la locale active, `t("clé")` retourne la clé telle quelle.  
Si aucun fichier `locales/` n'existe, le runtime ne génère pas de `LOCALES` ni de `t()` (zéro overhead).

### SSG et i18n

Le compilateur pré-rend `{t("clé")}` avec la locale par défaut, comme les autres interpolations.  
Après `DOMContentLoaded`, `bind()` met à jour les spans de manière réactive.

---

## SSG — Static Site Generation

Le compilateur pré-rend automatiquement l'état initial dans le HTML généré. Aucune configuration requise.

### Interpolations pré-remplies

Les `<span data-webcore-interpolation>` vides reçoivent leur valeur initiale au moment de la compilation :

```html
<!-- Source .webc -->
p "Compteur : {count}"    <!-- component state: count = 0 -->

<!-- HTML généré avec SSG -->
<p>Compteur : <span data-webcore-interpolation="count">0</span></p>
```

### Affichage initial `@if`/`@else`

Le bon branchement est affiché dès le premier paint, sans attendre JavaScript :

```html
<!-- State initial : count = 0 -->

<!-- Avec SSG : affichage correct immédiatement -->
<div data-webcore-if="count &gt; 0" style="display:none">...</div>
<div data-webcore-else="count &gt; 0" style="display:block">...</div>
```

### Compatibilité runtime

Le runtime JS (`bindIf`, `bind`) continue à opérer normalement après `DOMContentLoaded`.  
Il met à jour `el.style.display` et `el.textContent` de manière réactive.

### Expressions supportées pour le pré-rendu

| Type | Exemple | Résultat |
|---|---|---|
| Variable directe | `{count}` | valeur initiale de `count` |
| Littéral numérique | `{42}` | `42` |
| Addition/soustraction | `{count + 1}` | valeur + 1 |
| Variable de store | `{$store.hits}` | valeur du store |
| Condition `>`, `<`, `>=`, `<=`, `==`, `!=` | `@if count > 0` | `display:block/none` |

Les expressions complexes (appels de fonction, ternaires, etc.) sont laissées vides — le runtime les résout au chargement.

---

## WebAssembly (WASM)

WebCore détecte automatiquement un module Rust/WASM dans le sous-dossier `wasm/` et l'intègre à la sortie compilée.

### Détection et build

Si `wasm/Cargo.toml` existe dans le répertoire du projet, le compilateur :

1. Lit le nom du paquet (`[package] name`) depuis `wasm/Cargo.toml`
2. Exécute `wasm-pack build --target web --out-dir dist/wasm/`
3. Injecte un loader asynchrone dans le runtime JS

### Structure attendue

```
mon-projet/
├── wasm/
│   ├── Cargo.toml     # [package] name = "mon-module"
│   └── src/
│       └── lib.rs     # fonctions annotées #[wasm_bindgen]
├── pages/
└── webc.toml
```

`webc new <nom>` crée automatiquement ce scaffold avec un exemple `wasm-bindgen`.

### Loader JS injecté

```js
const WASM = {};
globalThis.wasm = WASM;
(async () => {
  try {
    const m = await import('./wasm/mon_module.js');
    await m.default();
    Object.assign(WASM, m);
    bind(); bindIf(); bindFor(); bindAttrs();
  } catch (e) {
    console.warn('[WebCore WASM]', e);
  }
})();
```

- `globalThis.wasm` est disponible **avant** le chargement (objet vide), puis rempli après.
- `Object.assign(WASM, m)` copie toutes les exports du module dans l'objet partagé.
- Un rebind complet est déclenché après le chargement pour mettre à jour les interpolations qui appellent des fonctions WASM.
- Les erreurs de chargement sont silencieuses (warning console) : la page reste fonctionnelle sans WASM.
- Le séquence de rebind après le chargement est contextuelle (tree-shaking) — seules les fonctions de bind présentes dans le document sont appelées.

### Utilisation dans un composant

```webc
component Signer {
  state { result: String = "" }
  view {
    button on:click={result = wasm.sign(result)} { "Signer" }
    p "Résultat : {result}"
  }
}
```

### Limites

- Un seul module WASM par projet (le premier `wasm/Cargo.toml` trouvé)
- `wasm-pack` doit être installé séparément (`cargo install wasm-pack`)
- Le loader est absent du bundle si `wasm/Cargo.toml` n'existe pas

---

## Bloc `http { }` — Requêtes HTTP déclaratives

Le bloc `http` dans un composant déclenche un `fetch()` JSON automatiquement dans `DOMContentLoaded`.

### Syntaxe

```webc
component NomComposant {
    state { items: List = null }
    http { get: "/api/endpoint"  into: items }
    view { ... }
}
```

- `get:` — URL cible (chaîne de caractères)
- `into:` — nom de la variable du state qui reçoit la réponse JSON

### Auto-injection de `loading` et `error`

Lorsqu'un composant contient un bloc `http {}`, le parser **injecte automatiquement** dans son `state` :

- `loading: Boolean = true` — mis à `false` après la réponse (succès ou erreur)
- `error: String = ""` — contient le message d'erreur si la requête échoue

Ces variables n'ont pas besoin d'être déclarées manuellement. Elles sont pleinement réactives.

### Exemple complet

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

### Code généré

```js
(async()=>{
  try {
    const __r = await fetch("/api/posts");
    if(!__r.ok) throw new Error(__r.statusText);
    const __d = await __r.json();
    S.set('posts', __d);
    S.set('loading', false);
    bind(); bindFor(); bindIf();
  } catch(__e) {
    S.set('error', __e.message);
    S.set('loading', false);
    bind(); bindIf();
  }
})();
```

---

## Bloc `head { }` — Personnalisation du `<head>`

Le bloc `head` dans une déclaration `page` permet de personnaliser le `<head>` HTML de cette page spécifiquement.

### Syntaxe

```webc
page "article" {
    head {
        title "Mon Article"
        meta description="Article de blog WebCore"
        meta og:title="Mon Article"
    }
    h1 "Hello"
}
```

- `title "..."` — génère `<title>Mon Article</title>` (override le titre global de `webc.toml`)
- `meta name="valeur"` — génère `<meta name="name" content="valeur">`
- `meta og:title="valeur"` — génère `<meta name="og:title" content="valeur">`

Les autres éléments de la page (ici `h1 "Hello"`) vont dans le `<body>` normalement.

---

## `$query.` — Paramètres query string

`$query.` donne accès aux paramètres de l'URL query string (`?key=value&...`).

### Syntaxe

```webc
p "Recherche : {$query.search}"
p "Page : {$query.page}"
p "Tri : {$query.sort}"
```

### Tree-shaking

Le compilateur n'émet `QUERY_PARAMS` que si au moins une référence `$query.` est présente dans le document :

```js
const QUERY_PARAMS = new Proxy({}, {
    get: (_, k) => new URLSearchParams(location.search).get(String(k)) ?? ""
});
```

Si aucune référence `$query.` n'est trouvée, ce code est absent du bundle — zéro overhead.

### Utilisation typique

```webc
component SearchResults {
    state { results: List = null }
    http { get: "/api/search?q={$query.q}"  into: results }
    view {
        p "Résultats pour : {$query.q}"
        @for item in results { li "{item.title}" }
    }
}
```

---

## `class:name` — Classes conditionnelles

`class:name={expr}` lie une classe CSS à une expression booléenne. La classe est ajoutée si l'expression est truthy, supprimée sinon.

### Syntaxe

```webc
div class:active={isOpen} { "Contenu" }
div class:active={isOpen} class:hidden={!visible} { "Contenu" }
button class:disabled={count == 0} on:click={count -= 1} { "−" }
```

### Compilation

`class:active={isOpen}` émet l'attribut HTML `data-webcore-class-active="isOpen"`.
`bindAttrs()` évalue l'expression et appelle `el.classList.toggle("active", result)`.

### Tree-shaking

La logique de class-toggle dans `bindAttrs()` n'est émise que si au moins un attribut `class:` est présent dans le document.

---

## `on:event|debounce` — Handlers debouncés

Le modificateur `|debounce` après le type d'événement enveloppe le handler dans un `setTimeout` de 300 ms. Le handler ne se déclenche qu'après 300 ms d'inactivité (si l'utilisateur cesse de taper/agir).

### Syntaxe

```webc
input on:input|debounce={search = event.target.value}
input on:keyup|debounce={query = event.target.value}
```

### Code généré

```js
el.addEventListener('input', (event) => {
    clearTimeout(el.__debounce);
    el.__debounce = setTimeout(() => {
        S.set('search', event.target.value); bind(); ...
    }, 300);
});
```

### Cas d'usage

- Champ de recherche : évite un appel API à chaque frappe
- Filtrage de liste : recalcule uniquement après une pause de l'utilisateur
- Compatible avec tout type d'événement : `on:input|debounce`, `on:keyup|debounce`, `on:change|debounce`, etc.

---

## `on:event|stop|prevent|self|once` — Modificateurs d'événements

Les modificateurs d'événements permettent de contrôler le comportement du listener sans JS brut. Ils s'ajoutent après le type d'événement, séparés par `|`.

### Modificateurs disponibles

| Modificateur | Effet |
|---|---|
| `\|stop` | Appelle `e.stopPropagation()` — empêche la propagation vers les parents |
| `\|prevent` | Appelle `e.preventDefault()` — empêche le comportement par défaut (ex. soumission de formulaire) |
| `\|once` | Le handler ne se déclenche qu'une seule fois (marqueur `data-webcore-onced`) |
| `\|self` | Le handler ne se déclenche que si `e.target === l'élément lui-même` (pas un enfant) |
| `\|debounce` | Voir [`on:event\|debounce`](#oneventdebounce--handlers-debouncés) |

### Syntaxe

```webc
// Stop propagation
button on:click|stop={count += 1} { "+" }

// Prevent default
form on:submit|prevent={handleSubmit} { ... }

// Once — ne se déclenche qu'une fois
button on:click|once={showWelcome = true} { "Afficher" }

// Self — uniquement sur l'élément lui-même, pas ses enfants
div on:click|self={collapsed = !collapsed} {
    span { "Clic sur ce span ne déclenche pas le parent" }
}

// Combinaison
form on:submit|stop|prevent={handleSubmit} { ... }
```

### Compilation

Les modificateurs sont encodés dans l'attribut `data-webcore-e` :

```html
<!-- on:click|stop|prevent -->
<button id="h1" data-webcore-e="click|stop|prevent">+</button>
```

Le listener délégué `D()` lit les modificateurs depuis `data-webcore-e` et les applique. La syntaxe `.webc` reste inchangée — seul le HTML généré reflète les modificateurs.

### Compatibilité avec `|debounce`

`|debounce` et les autres modificateurs sont **mutuellement exclusifs** — un handler debouncé utilise un mécanisme `setTimeout` distinct (voir [`on:event|debounce`](#oneventdebounce--handlers-debouncés)).

---

## Commande `webc watch`

`webc watch` surveille les fichiers sources et rebuilde automatiquement à chaque modification, **sans démarrer de serveur de développement**.

```bash
cd mon-app
webc watch
```

### Comportement

- Surveille récursivement `src/`, `locales/`, `public/`, `theme.toml` et `webc.toml`
- Debounce de 200 ms — un burst de modifications ne déclenche qu'un seul rebuild
- Affiche le résultat du build (arborescence `dist/` ou erreurs) après chaque rebuild
- Continue même si un build échoue — attend la prochaine modification

### Différences avec `webc dev`

| | `webc watch` | `webc dev` |
|---|---|---|
| Serveur HTTP | Non | Oui (port 3000) |
| Rechargement navigateur (HMR) | Non | Oui (WebSocket) |
| Idéal pour | CI/CD, builds continus, scripts | Développement interactif |

---

## Commande `webc check`

`webc check` valide le projet sans générer de fichiers de sortie.

```bash
cd mon-app
webc check
```

Contrôles effectués :

| Contrôle | Exemple d'erreur |
|---|---|
| Syntaxe `.webc` | Accolade manquante, expression vide `{}` |
| Routes → pages | `/about: AboutPage` déclarée mais aucune page `"about"` dans les fichiers `.webc` |
| Composants instanciés | `Counter {}` utilisé mais composant `Counter` introuvable |
| Types de props | Prop `count: Number` reçoit `label="hello"` (type incohérent) |

En cas d'erreur, `webc check` affiche le fichier, la ligne et un message explicite, puis quitte avec code 1.
Si tout est valide, il affiche `✓ projet valide` et quitte avec code 0.

---

## `ref:name` — Références DOM directes

L'attribut `ref:name=true` sur un élément enregistre une référence directe à ce nœud DOM dans l'objet `refs`, accessible après `DOMContentLoaded`.

### Syntaxe

```webc
input ref:myInput=true
button ref:submitBtn=true { "Envoyer" }
```

### Effet compilé

L'élément reçoit l'attribut HTML `data-webcore-ref="name"` :

```html
<input data-webcore-ref="myInput">
```

Dans le runtime JS généré, `const refs = {}` est déclaré à la portée du bloc, et dans `DOMContentLoaded` :

```js
const refs = {};
document.addEventListener('DOMContentLoaded', () => {
    refs['myInput'] = document.querySelector('[data-webcore-ref="myInput"]');
});
```

### Cas d'usage

```webc
component SearchBar {
    state { query: String = "" }
    on:mount {
        refs['searchInput'].focus();
    }
    view {
        input ref:searchInput=true
              on:input={query = event.target.value}
              placeholder="Rechercher..."
    }
}
```

- Accès direct à un élément sans `querySelector` dans le code utilisateur
- Utile pour la gestion du focus, les appels de méthodes DOM (`scrollIntoView`, `select`, etc.)
- Plusieurs références peuvent coexister sur des éléments différents

### Tree-shaking

Le flag `has_refs` est positionné lors du parsing. Si aucun `ref:` n'est présent dans le document, `const refs = {}` et le code d'enregistrement ne sont pas émis.

---

## `style:prop` — Styles inline dynamiques

`style:prop={expr}` lie une propriété CSS inline à une expression réactive. La valeur est appliquée via `el.style.setProperty(...)` dans `bindAttrs()`.

### Syntaxe

```webc
div style:color={myColor} { "Texte coloré" }
div style:background-color={bg} style:font-size={size} { "Contenu" }
```

### Effet compilé

`style:color={myColor}` émet l'attribut HTML `data-webcore-style-color="myColor"`.
Les tirets dans le nom de propriété sont préservés (`background-color` reste `background-color`).

```html
<div data-webcore-style-color="myColor">Texte coloré</div>
```

`bindAttrs()` appelle :

```js
el.style.setProperty('color', evalCond(myColor, ...));
el.style.setProperty('background-color', evalCond(bg, ...));
```

### Coexistence avec d'autres attributs

`style:`, `style="..."` statique et `class:` peuvent coexister sur le même élément :

```webc
div style="padding: 1rem"
    style:color={textColor}
    class:active={isOpen}
    { "Contenu" }
```

### Cas d'usage

```webc
component ColorPicker {
    state {
        hue:  Number = 200
        sat:  Number = 80
        lite: Number = 50
    }
    view {
        div style:background-color={"hsl(" + hue + "," + sat + "%," + lite + "%)"}
            { "Aperçu" }
        input type="range" bind:value={hue}
    }
}
```

### Tree-shaking

Le flag `has_style_binding` est positionné lors du parsing. La logique `style.setProperty` dans `bindAttrs()` n'est émise que si au moins un `style:` est présent dans le document.

---

## Contenu par défaut des slots

Les layouts peuvent définir un contenu de repli pour les slots nommés. Ce contenu est utilisé lorsque la page ne fournit pas de contenu pour ce slot.

### Syntaxe

```webc
layout DashLayout {
    header { slot header }
    aside  {
        slot sidebar {
            p "Navigation par défaut"
            link to="/" { "Accueil" }
        }
    }
    main { slot content }
}
```

### Comportement

- Si la page **remplit** le slot (`slot sidebar { ... }`) → le contenu de la page est utilisé
- Si la page **ne remplit pas** le slot → le contenu par défaut du layout est utilisé
- Le slot `content` (contenu principal) continue de fonctionner comme avant — il utilise le corps de la page

### Exemple complet

```webc
// Layout avec sidebar par défaut
layout AppLayout {
    aside {
        slot sidebar {
            p "Sidebar par défaut"
        }
    }
    main { slot content }
}

// Page A — fournit une sidebar
page "dashboard" {
    slot sidebar {
        nav { link to="/settings" { "Paramètres" } }
    }
    h1 "Tableau de bord"
}

// Page B — n'a pas de sidebar → contenu par défaut utilisé
page "about" {
    h1 "À propos"
}
```

### Historique

Avant la v1.4.0, les slots nommés non remplis par une page étaient silencieusement supprimés — le contenu défini dans le layout pour ce slot était ignoré.

---

## `webc:transition` — Animations CSS

L'attribut `webc:transition="name"` ajoute une animation CSS à un élément conditionnel. L'élément entre avec l'animation d'entrée et quitte avec l'animation de sortie lorsqu'un bloc `@if` change d'état.

### Syntaxe

```webc
div webc:transition="fade" {
    p "Contenu animé"
}

div webc:transition="slide" {
    p "Glisse vers le bas à l'entrée"
}
```

### Transitions intégrées

| Nom | Entrée | Sortie |
|---|---|---|
| `fade` | opacité `0 → 1` | opacité `1 → 0` |
| `slide` | `translateY(-10px) → 0` | `0 → translateY(-10px)` |

### Fonctionnement avec `@if`

```webc
@if isOpen {
    div webc:transition="fade" {
        p "Ce panneau s'affiche en fondu"
    }
}
```

À l'entrée, l'élément apparaît avec l'animation d'entrée.
À la sortie (quand `isOpen` devient `false`), l'animation de sortie est jouée, puis l'élément est retiré du DOM.

### Implémentation

L'attribut HTML `data-webcore-transition="name"` est émis sur l'élément :

```html
<div data-webcore-transition="fade">...</div>
```

Le runtime JS injecte le CSS des transitions et utilise `requestAnimationFrame` + `transitionend` pour synchroniser l'ajout et la suppression du DOM avec les animations CSS.

```js
// CSS injecté automatiquement (une seule fois)
const style = document.createElement('style');
style.textContent = `
    [data-webcore-transition="fade"] { transition: opacity 0.2s ease; }
    [data-webcore-transition="fade"].wc-enter { opacity: 0; }
    [data-webcore-transition="fade"].wc-leave { opacity: 0; }
`;
document.head.appendChild(style);
```

### Tree-shaking

Le flag `has_transition` est positionné lors du parsing. Si aucun `webc:transition` n'est présent, le CSS et la logique `requestAnimationFrame`/`transitionend` ne sont pas émis.

---

## `webc:img` — Images optimisées

La directive `webc:img` sur un élément `img` déclenche une transformation compile-time complète : injection des attributs de chargement différé, lecture des dimensions réelles depuis `public/` et validation de l'accessibilité.

### Syntaxe

```webc
img webc:img src="/hero.png" alt="Hero"
img webc:img src="/logo.svg" alt="Logo" class="logo"
```

### Sortie compilée

```html
<img src="/assets/hero.png" loading="lazy" decoding="async" width="1200" height="630" alt="Hero">
```

- `loading="lazy"` et `decoding="async"` sont **toujours** injectés sur tout `img` portant `webc:img`
- `width` et `height` sont lus depuis le fichier réel dans `public/` — le crate `imagesize` extrait les dimensions sans décoder l'image entière
- `src` pointe vers `dist/assets/` (le préfixe `/assets/` est appliqué automatiquement)
- L'attribut `webc:img` est **supprimé** de la sortie HTML — ce n'est pas un attribut HTML valide

### Avertissement `alt` manquant

Si l'attribut `alt` est absent, le compilateur émet :

```
warning[a11y]: <img> with webc:img is missing alt attribute
  --> src/pages/home.webc:12
```

La compilation continue normalement — c'est un avertissement, pas une erreur.

### Comparaison avec un `<img>` ordinaire

| | `img src="..."` | `img webc:img src="..."` |
|---|---|---|
| `loading="lazy"` | Manuel | Automatique |
| `decoding="async"` | Manuel | Automatique |
| `width` / `height` | Manuel (ou oublié) | Lu depuis `public/` |
| Prévention du CLS | Non garantie | Garantie |
| Avertissement `alt` | Non | Oui |

### Aucun overhead runtime

`webc:img` est une transformation **purement compile-time**. Aucun JS n'est émis dans le bundle — l'optimisation est intégralement réalisée par le compilateur Rust au moment du build.

---

## Fingerprinting des images

À chaque `webc build`, toutes les images du dossier `public/` reçoivent un hash de contenu intégré dans leur nom de fichier. Les références dans les HTML et CSS générés sont mises à jour en conséquence.

### Mécanisme

```
public/logo.png          →  dist/assets/logo.a3f9c1b2.png
public/hero.jpg          →  dist/assets/hero.d4e2f1a0.jpg
public/icons/arrow.svg   →  dist/assets/icons/arrow.7b3c9e4d.svg
```

### Extensions concernées

`.png`, `.jpg`, `.jpeg`, `.gif`, `.webp`, `.svg`, `.ico`, `.avif`

### Algorithme de hachage

FNV-1a 32 bits appliqué sur les **octets bruts** du fichier → 8 caractères hexadécimaux.
Le même algorithme déterministe est utilisé pour les IDs de scope CSS (`data-v`).

### Réécriture des références

Toutes les occurrences de `logo.png` dans les fichiers `.html` et `.css` générés sont remplacées par `logo.a3f9c1b2.png` avant l'écriture sur disque. Cela couvre :

- Les attributs `src` et `href` dans le HTML
- Les propriétés `url(...)` dans le CSS (images de fond, etc.)

### Cache-busting parfait

Le navigateur peut mettre les images en cache avec une durée de vie indéfinie (`Cache-Control: max-age=31536000, immutable`). Lorsque le contenu d'une image change, son nom de fichier change → le navigateur télécharge automatiquement la nouvelle version.

### Toujours actif

Le fingerprinting est activé **par défaut**, quelle que soit la valeur de `mode` dans `webc.toml`. Aucune configuration n'est nécessaire.

---

## Limites actuelles (v2.1.0)

| Limite | Contournement |
|---|---|
| SSG et `http {}` : les requêtes HTTP ne sont pas exécutées au pré-rendu | `loading` reste `true` au SSG — le runtime résout la requête au chargement |
| WASM : un seul module par projet | Exporter toutes les fonctions depuis un seul `lib.rs` |
| Événements inter-composants portée globale (`document`) — pas de portée par instance | Préfixer le nom d'événement pour éviter les collisions |
| Routes paramétrées : pas de routes imbriquées ni de regexp custom | Utiliser des routes à un seul segment dynamique par chemin |
| `webc:img` : formats AVIF et JXL non supportés pour la lecture de dimensions | Spécifier `width` et `height` manuellement pour ces formats |
| Imports de données : JSON et TOML plats uniquement (pas CSV, XML, requêtes réseau) | Convertir manuellement en JSON avant le build |

---

## Nouvelles fonctionnalités v1.1.0

### Routes paramétrées

Les routes déclarées dans `app { routes { } }` peuvent contenir des segments `:param` :

```webc
app MyApp {
    routes {
        "/":           HomePage
        "/post/:slug": PostPage
        "/user/:id":   ProfilePage
    }
}

component PostPage {
    view { h1 "Article : {$route.slug}" }
}
```

Le compilateur génère un tableau `ROUTES` avec patterns RegExp et une fonction
`matchRoute()`. Les paramètres sont accessibles dans les vues via `{$route.paramName}`.
`ROUTES` et `ROUTE_PARAMS` sont tree-shaqués si aucune route n'est paramétrée.

### `@for` avec key (DOM diffing)

```webc
@for post key=post.id in posts {
    article "{post.title}"
}
```

Sans `key`, la liste est entièrement re-rendue à chaque changement. Avec `key`,
`bindFor()` patche uniquement les nœuds modifiés/ajoutés/supprimés.

### i18n : paramètres et pluralisation

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

- `t("key", n: Number)` → cherche `key_one` (n=1) ou `key_other`, substitue `{{count}}`
- `t("key", val)` → substitue `{{0}}` dans la traduction

### Props composées

Les expressions composites dans les vues d'un composant sont maintenant substituées :

```webc
component Stepper {
    props { step }
    view {
        p "{step + 1}"       // ✓ compilé en "(2) + 1" si step="2"
        button class={step}  // ✓ class="2"
    }
}
```

---

## Nouveautés v1.1.1

### Handlers multi-instructions

```webc
button on:click={items = [...(items ?? []), mkItem(draft)]; draft = ""} { "Ajouter" }
```

Chaque instruction séparée par `;` est compilée indépendamment. Voir [Handlers multi-instructions](#handlers-multi-instructions).

### Items objets dans `@for`

Les items de liste peuvent être des objets. Les propriétés sont accessibles via la notation pointée dans les interpolations. Voir [`@for` avec des items objets](#for-avec-des-items-objets).

### Sélecteurs CSS multi-éléments

`input, textarea { }` dans les blocs `style { }`. Voir [Style](#style).

### Validation de formulaires — phase de capture

Le listener de soumission tourne désormais en phase de capture. Voir [Comportement runtime](#comportement-runtime).

### `on:mount` — imbrication profonde

Les corps `on:mount { }` supportent les accolades JS imbriquées à profondeur arbitraire. Les callbacks complexes ne provoquent plus d'erreur de parse. Voir [`on:mount`](#onmount).

---

## Nouveautés v1.2.0

### `@switch` / `@case` / `@default`

Directive multi-branches compilée en chaîne `@if`/`@else` au parsing. Voir [`@switch`](#switch--case--default).

### `bind:` — liaison bidirectionnelle

`bind:value={x}` et `bind:checked={x}` : raccourci pour l'attribut de valeur + le handler de mise à jour.
Voir [`bind:`](#bind--liaison-bidirectionnelle).

### `@for item, i in items` — index de boucle

La seconde variable optionnelle dans `@for` reçoit l'index (0-based) de l'élément courant. Voir [`@for` avec index](#for-avec-index).

### `webc check` — validation sans build

Nouvelle commande CLI qui parse et vérifie les références sans générer de fichiers. Voir [Commande `webc check`](#commande-webc-check).

### URLs propres

Les pages sont générées dans `slug/index.html` — les URLs n'ont plus d'extension `.html`.

### `dist/assets/` — séparation HTML / assets

JS, CSS et assets publics dans `dist/assets/` ; HTML à la racine de `dist/`. Les chemins d'assets sont absolus pour les pages imbriquées.

### CSS public minifié en mode prod

Les fichiers `.css` dans `public/` sont désormais traités par LightningCSS quand `mode = "prod"` est activé dans `webc.toml`.

---

## Nouveautés v1.3.0

### Bloc `http { }` — fetch déclaratif

`http { get: "/url"  into: var }` dans un composant : déclenche un `fetch()` JSON dans `DOMContentLoaded`.
`loading` et `error` sont auto-injectés dans le state. Voir [Bloc `http { }`](#bloc-http----requêtes-http-déclaratives).

### Bloc `head { }` — personnalisation du `<head>`

`head { title "..." meta name="..." }` dans une page : override le titre et ajoute des meta tags.
Voir [Bloc `head { }`](#bloc-head----personnalisation-du-head).

### `$query.` — paramètres query string

`{$query.search}`, `{$query.page}` etc. — accès aux paramètres d'URL ; tree-shaké si inutilisé.
Voir [`$query.`](#query--paramètres-query-string).

### `class:name={expr}` — classes CSS conditionnelles

`class:active={isOpen}` — active/désactive la classe selon l'expression ; géré par `bindAttrs()`.
Voir [`class:name`](#classname--classes-conditionnelles).

### `on:event|debounce` — handlers debouncés

`on:input|debounce={expr}` — le handler ne se déclenche qu'après 300 ms d'inactivité.
Voir [`on:event|debounce`](#oneventdebounce--handlers-debouncés).

### Correction : auto-injection `loading` / `error`

Les variables `loading` et `error` n'ont plus besoin d'être déclarées manuellement dans `state` lorsqu'un composant contient un bloc `http {}` — le parser les injecte automatiquement.

---

## Nouveautés v1.4.0

### `ref:name=true` — Références DOM directes

`input ref:myInput=true` enregistre l'élément dans `refs['myInput']` via `DOMContentLoaded`. Accès direct sans `querySelector` ; utile pour la gestion du focus et les manipulations DOM impératives. Tree-shaké via le flag `has_refs`.
Voir [`ref:name`](#refname--références-dom-directes).

### `style:prop={expr}` — Styles inline dynamiques

`div style:color={myColor}` émet `data-webcore-style-color="myColor"` ; `bindAttrs()` appelle `el.style.setProperty('color', evalCond(myColor, ...))`. Peut coexister avec `style="..."` statique et `class:` sur le même élément. Tree-shaké via le flag `has_style_binding`.
Voir [`style:prop`](#styleprop--styles-inline-dynamiques).

### Contenu par défaut des slots

`slot sidebar { p "Contenu par défaut" }` dans un layout — utilisé si la page ne remplit pas le slot. Les slots non remplis étaient précédemment supprimés silencieusement. Le slot `content` continue d'utiliser le corps de la page.
Voir [Contenu par défaut des slots](#contenu-par-défaut-des-slots).

### `webc:transition="name"` — Animations CSS sur les blocs conditionnels

`div webc:transition="fade" { ... }` ou `div webc:transition="slide" { ... }` — transitions intégrées sur les blocs `@if` : `fade` (opacité 0→1) et `slide` (translateY -10px→0). Le JS injecte le CSS et utilise `requestAnimationFrame` + `transitionend`. Tree-shaké via le flag `has_transition`.
Voir [`webc:transition`](#webctransition--animations-css).

---

## Nouveautés v1.5.0

### `webc:img` — Images optimisées (compile-time)

`img webc:img src="/hero.png" alt="Hero"` compile vers `<img src="/assets/hero.png" loading="lazy" decoding="async" width="1200" height="630" alt="Hero">`. Les attributs `loading="lazy"` et `decoding="async"` sont injectés automatiquement. Les dimensions `width`/`height` sont lues dans `public/` à la compilation via le crate `imagesize` (aucun décodage d'image complet). Si `alt` est absent, le compilateur émet `warning[a11y]: <img> with webc:img is missing alt attribute`. L'attribut `webc:img` est supprimé de la sortie HTML. Zéro JS émis — transformation purement compile-time.
Voir [`webc:img`](#webcimg--images-optimisées).

### Fingerprinting des images

À chaque `webc build`, toutes les images dans `public/` reçoivent un hash de contenu FNV-1a 32 bits intégré dans leur nom : `logo.png` → `logo.a3f9c1b2.png`. Extensions concernées : `.png`, `.jpg`, `.jpeg`, `.gif`, `.webp`, `.svg`, `.ico`, `.avif`. Toutes les références dans les `.html` et `.css` générés sont mises à jour automatiquement. Toujours actif — aucune configuration nécessaire. Avantage : cache-busting parfait, le navigateur peut mettre les images en cache indéfiniment.
Voir [Fingerprinting des images](#fingerprinting-des-images).

---

## Nouveautés v2.0.0

- **Signaux réactifs fins** — voir section [Signaux réactifs fins](#signaux-réactifs-fins)
- **HMR** — `webc serve` surveille et recharge automatiquement
- **Path traversal corrigé** — `webc serve` retourne 403 pour les URLs hors `dist/`
- **Détection de cycles** — `webc check` signale les références circulaires
- **Agrégation des erreurs** — toutes les erreurs de build sont reportées ensemble
- **CSS nesting** — voir section [CSS nesting](#css-nesting)
- **Rapport bundle** — voir section [Rapport d'analyse du bundle](#rapport-danalyse-du-bundle)

## Signaux réactifs fins

`$effect(fn)` remplace le pattern v1.x `VARS.forEach(v=>S.on(v,fn))`.

```webc
component Counter {
    state { count: Number = 0 }
    view {
        button on:click={count++} { "+" }
        p "{count}"
    }
}
```

À la compilation, le JS généré utilise `$effect` :
```js
$effect(() => {
    el.textContent = S.get('count');
});
```

Le tracking est automatique : l'effet est ré-exécuté uniquement quand `count` change.
Aucune liste manuelle de dépendances nécessaire.

## CSS nesting

Les règles imbriquées sont supportées dans les blocs `style {}` :

```webc
component Card {
    view { div class="card" { p "content" } }
    style {
        .card {
            padding: 1rem;
            &:hover { background: #f5f5f5; }
            & > p { color: #333; }
        }
    }
}
```

Le sélecteur `&` est remplacé par le sélecteur parent scopé à la compilation.
La sortie CSS générée est du CSS valide aplati.

## Rapport d'analyse du bundle

Après `webc build`, le compilateur affiche un tableau récapitulatif :

```
Bundle Analysis:
  ✓ state            312 b
  ✓ signals ($effect) 428 b
  ✓ dom init          89 b
  ✓ bindFor          512 b
  - bindIf           (tree-shaken)
  - http             (tree-shaken)
  ✓ router           634 b
Total JS: 1.98 KB
```

Les fonctionnalités non utilisées sont tree-shaquées automatiquement : `http`, `bindIf`, `bindFor`, etc. n'apparaissent dans le bundle que si le projet les utilise.

---

## Nouveautés v2.1.0

### `$watch` — observer réactif

Directive de composant qui exécute un bloc de code chaque fois qu'une variable d'état change.

```webc
component Analytics {
    state { count: Number = 0 }
    $watch count => {
        console.log("count changed to", count);
        fetch("/api/track", { method: "POST", body: JSON.stringify({ count }) });
    }
    view { button on:click={count += 1} { "+" } }
}
```

Génère dans `DOMContentLoaded` :
```js
S.on('count', count => {
    console.log("count changed to", count);
    fetch("/api/track", { method: "POST", body: JSON.stringify({ count }) });
});
```

---

### `@for N..M` — plage numérique

Syntaxe de plage sans donnée d'état :

```webc
@for i in 0..5 {
    li "Élément {i}"
}
```

Génère `data-webcore-for-range="0..5"` sur le `<template>`. Le runtime JS crée le tableau `["0","1","2","3","4"]` sans accès à l'état.

---

### `@for key={expr}` — clé complexe

En plus de `key=item.id`, la forme accoladée permet des expressions arbitraires :

```webc
@for item in items key={item.id + "-" + item.type} {
    li "{item.name}"
}
```

---

### Expressions SSG étendues

`eval_expr_with_locale` supporte maintenant :

| Expression | Résultat |
|---|---|
| `items.length` | nombre d'éléments du tableau ou longueur de la chaîne |
| `name.toUpperCase()` | chaîne en majuscules |
| `name.toLowerCase()` | chaîne en minuscules |
| `str.trim()` | chaîne sans espaces en début/fin |

---

### Validation des props à la compilation

Si un composant reçoit une prop non déclarée dans son bloc `props {}`, le compilateur émet :

```
warning[props]: component 'Badge' received unknown prop 'colour'
  hint: declared props are: label, color
```

La compilation continue — c'est un avertissement, pas une erreur.

---

### Imports de données build-time

```webc
import posts from "data/posts.json"
```

Le fichier JSON est lu, validé et injecté comme :
```js
S.setQ("posts", [{"id":1,"title":"Hello"},...]);
```

avant le bloc `DOMContentLoaded`. Les fichiers TOML sont convertis en JSON. Les chemins qui sortent du répertoire projet sont refusés (`canonicalize()` + `starts_with(project_root)`).

---

## Nouveautés v2.2.0

### `@keyframes` dans `style {}`

Les animations CSS peuvent être définies directement dans le composant :

```webc
component Spinner {
    view { div class="spin" {} }
    style {
        @keyframes spin {
            from { transform: rotate(0deg); }
            to   { transform: rotate(360deg); }
        }
        .spin {
            animation: spin 1s linear infinite;
            width: 2rem; height: 2rem;
            border: 3px solid #ccc;
            border-top-color: #333;
            border-radius: 50%;
        }
    }
}
```

Les `@keyframes` sont émis **globaux** (non scopés) car ils sont référencés par nom depuis `animation:`. Les règles CSS normales restent scopées via `data-v`.

---

### `<script defer>` et `<link rel="preload">`

Le shell HTML généré inclut désormais :

```html
<head>
    <link rel="preload" as="script" href="/assets/webcore.js?v=a1b2c3d4">
    <!-- ... -->
</head>
<body>
    <!-- ... -->
    <script defer src="/assets/webcore.js?v=a1b2c3d4"></script>
</body>
```

- `defer` : le script ne bloque pas le parsing HTML, s'exécute après le DOM
- `preload` : le navigateur télécharge le fichier JS dès la lecture du `<head>`, en parallèle du HTML

---

### Élision du scope CSS

Les composants sans bloc `style {}` n'émettent plus l'attribut `data-v="..."` :

```html
<!-- Composant sans style -->
<div>contenu</div>

<!-- Composant avec style (data-v conservé) -->
<div data-v="wc-a1b2">contenu</div>
```

---

### Minification HTML en mode `prod`

En mode `prod` (`mode = "prod"` dans `webc.toml`), le HTML généré est minifié :

- Commentaires `<!-- ... -->` supprimés
- Espaces entre balises (`>\s+<`) réduits à un seul espace

---

### Avertissement ReDoS

Si un attribut `validate:pattern` contient un quantificateur imbriqué susceptible de causer un backtracking catastrophique :

```webc
input validate:pattern="(a+)+"  // ← dangereux
```

Le compilateur émet :
```
warning[security]: validate:pattern may cause ReDoS — nested quantifier detected: (a+)+
```

---

## Nouveautés v2.3.0

### SRI — Subresource Integrity

En mode `prod`, le compilateur calcule automatiquement un hash SHA-256 de chaque fichier asset et l'intègre dans les balises :

```html
<link rel="stylesheet"
      href="/assets/theme.css?v=a1b2c3d4"
      integrity="sha256-AbCdEf...="
      crossorigin="anonymous">

<script defer
        src="/assets/webcore.js?v=a1b2c3d4"
        integrity="sha256-XyZqRs...="
        crossorigin="anonymous"></script>
```

Le navigateur vérifie que le fichier reçu correspond au hash déclaré avant de l'exécuter — protège contre la modification de fichiers sur CDN ou en transit.

---

### Élision JS zéro

Les pages purement statiques n'ont plus aucun overhead JS :

```webc
page "about" {
    h1 "À propos"
    p "Contenu statique sans état ni événements."
}
```

HTML généré :
```html
<head>
    <!-- Aucun <link rel="preload"> -->
    <!-- Aucun <script> -->
</head>
```

Une page est considérée statique si elle ne contient ni état réactif, ni directive `@if`/`@for`, ni interpolation dynamique, ni événement `on:`, ni composant réactif.

---

### Limite de profondeur d'imbrication

Le parser rejette tout document dont les éléments dépassent 128 niveaux d'imbrication :

```
error[parse]: nesting depth exceeded (limit: 128)
  at src/pages/home.webc
  hint: deeply nested structures are usually a sign of an error
```

Cette limite protège contre les "nesting bombs" qui provoquaient un stack overflow pendant la compilation.

---

### Escape JS des URLs de navigation

Les apostrophes et backslashes dans les chemins de navigation sont désormais échappés :

```html
<!-- Avant (vulnérable) -->
<a onclick="webcore_navigate('/path/it\'s-broken')">

<!-- Après (sûr) -->
<a onclick="webcore_navigate('/path/it\'s-broken')">
```

Les chemins contenant `'` ou `\` ne peuvent plus injecter de code JS arbitraire.

---

## Nouveautés v2.4.0

### Critical CSS inline (mode `prod`)

En mode `prod`, chaque page reçoit dans son `<head>` un `<style>` contenant uniquement
le CSS dont elle a besoin : les styles globaux (variables de thème + base) plus les
styles scopés des composants **réellement utilisés** sur la page (collecte récursive —
un composant utilisé par un autre composant est aussi inclus).

La feuille complète `theme.css` est chargée **en différé**, sans bloquer le rendu :

```html
<style>/* CSS critique de la page, minifié */</style>
<link rel="stylesheet" href="/assets/theme.css?v=hash"
      integrity="sha256-..." crossorigin="anonymous"
      media="print" data-webcore-defer>
<noscript><link rel="stylesheet" href="/assets/theme.css?v=hash"></noscript>
```

- `media="print"` : le navigateur télécharge la feuille sans bloquer le rendu
- `data-webcore-defer` : le swap `media` → `"all"` est effectué dans `DOMContentLoaded` (CSP-safe, remplace `onload="this.media='all'"` depuis v2.5.0)
- `<noscript>` : fallback bloquant classique si JS est désactivé

Résultat : zéro CSS render-blocking, First Contentful Paint immédiat.
En mode `dev`, le lien bloquant classique est conservé (plus simple pour le HMR).

---

### Collections SSG — `each`

Le chaînon entre les imports de données et les routes paramétrées : générer une
page statique **par élément** d'une collection.

```webc
// src/app.webc
import posts from "data/posts.json"

app Blog {
    layout: MainLayout
    routes {
        "/":           HomePage
        "/post/:slug": PostPage each posts
    }
}
```

```json
// data/posts.json
[
  {"slug": "hello-world",  "title": "Hello World"},
  {"slug": "second-post",  "title": "Deuxième article"}
]
```

À `webc build`, le compilateur génère :

```
dist/post/hello-world/index.html
dist/post/second-post/index.html
```

- Le champ correspondant au paramètre de route (`slug`) détermine le chemin de sortie
- `{$route.slug}` est **pré-rendu** dans le HTML de chaque page (SSG)
- Les données complètes restent disponibles côté client via l'import (`S.setQ`)
- Sécurité : une valeur contenant `/`, `\`, `..` ou vide est rejetée à la compilation
  (elle devient un nom de répertoire sous `dist/`)

Combiné à l'élision zero-JS et au critical CSS inline, un blog généré ainsi pèse
quelques Ko de HTML pur par article.

---

### Résolution des imports de données au build

Les déclarations `import name from "file.json"` sont désormais réellement résolues
par `webc build` (elles étaient parsées mais jamais lues) :

- Fichiers `.json` : validés puis injectés tels quels
- Fichiers `.toml` : convertis en JSON
- Les chemins sont canonicalisés et doivent rester dans le répertoire projet —
  `import x from "../../etc/passwd"` est rejeté

---

## Nouveautés v2.5.0

### CSP stricte — event delegation

Depuis v2.5.0, les handlers d'événements ne sont plus émis comme attributs HTML inline.

**Avant (v2.4.0) :**
```html
<button onclick="webcore_handle_click(...)">
<form onsubmit="webcore_handle_submit(...)">
```

**Après (v2.5.0) :**
```html
<button data-webcore-e="click">
<form data-webcore-e="submit">
```

Un unique `document.addEventListener` par type d'événement est enregistré dans le runtime JS via délégation. Cela rend le HTML entièrement compatible avec `script-src 'self'` — plus aucun JS inline.

> La **syntaxe `.webc` ne change pas** : `on:click`, `on:submit`, `on:change` etc. fonctionnent exactement pareil pour le développeur. Seul le HTML généré change.

### Navigation SPA `data-webcore-nav`

Les liens de navigation SPA n'utilisent plus de JS inline :

**Avant :**
```html
<a onclick="webcore_navigate('/path')">Accueil</a>
```

**Après :**
```html
<a href="/path" data-webcore-nav>Accueil</a>
```

Un listener délégué `document.addEventListener('click', ...)` capture les clics sur `a[data-webcore-nav]` et déclenche la navigation History API. La syntaxe `.webc` reste inchangée : `link to="/path" "Accueil"`.

### CSS différé `data-webcore-defer`

Le swap du CSS différé est désormais géré dans `DOMContentLoaded` au lieu d'un attribut `onload` inline :

**Avant (v2.4.0) :**
```html
<link rel="stylesheet" href="/assets/theme.css" media="print" onload="this.media='all'">
```

**Après (v2.5.0) :**
```html
<link rel="stylesheet" href="/assets/theme.css" media="print" data-webcore-defer>
```

Le runtime détecte `data-webcore-defer` au `DOMContentLoaded` et applique `media = 'all'`. Le `<noscript>` fallback est conservé.

### Option `csp = true` dans `webc.toml`

```toml
[app]
mode = "prod"
csp = true
```

Quand `csp = true` en mode `prod`, chaque page reçoit automatiquement dans son `<head>` :

```html
<meta http-equiv="Content-Security-Policy"
      content="default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self' data:">
```

> Note : `style-src 'unsafe-inline'` est requis pour le critical CSS inline (voir [Limites actuelles](#limites-actuelles-v260)).

---

## Nouveautés v2.5.1

### Corrections de sécurité et de comportement

**Escape des `</style>` dans le CSS critique inline**

Les séquences `</style>` présentes dans le CSS critique inliné sont désormais échappées en `<\/style>`. Cela empêche la sortie prématurée du bloc `<style>` et évite toute injection HTML.

**Zero-JS + critical CSS**

Une page sans JS réactif qui utilise le critical CSS inclut maintenant `webcore.js`. Ce script est requis pour le swap `data-webcore-defer` (le chargement différé de `theme.css`). Sans lui, le CSS différé ne s'appliquerait jamais.

**`.length` sur arrays contenant des virgules dans les chaînes**

```webc
// items = ["a,b", "c"]
{items.length}  // retourne 2 (correct) — était 3 (bug de parsing naïf)
```

Le parser de tableaux distingue désormais correctement les virgules à l'intérieur des chaînes entre guillemets des virgules séparatrices d'éléments.

**`.length` Unicode-correct**

```webc
{"café".length}  // retourne 4 (caractères) — était 5 (octets UTF-8)
```

Le calcul de longueur opère désormais sur les caractères Unicode (points de code) et non sur les octets.

**Minificateur HTML — encodage UTF-8**

Le minificateur HTML en mode `prod` utilisait `bytes[i] as char` pour itérer sur les caractères, ce qui tronquait incorrectement les séquences UTF-8 multi-octets (accents, emoji, caractères CJK). Les caractères sont désormais traités comme des points de code Unicode — les titres, meta tags et contenus texte en langues non-ASCII ne sont plus corrompus après minification.

---

## Nouveautés v2.5.2

### Messages d'erreur enrichis

Depuis v2.5.2, les erreurs de compilation affichent un format structuré avec contexte visuel.

**Format d'erreur :**

```
error[parse]: src/pages/home.webc:3:17
  |
3 |     div class={  }
  |                 ^
  = expected expression
  = hint: une interpolation vide `{}` n'est pas valide — utilisez `{varName}` ou `{expr}`
```

Chaque erreur de parse comporte :
- Un en-tête `error[parse]: fichier:ligne:col` (rouge gras si le terminal le supporte)
- Le numéro de ligne avec le gutter `|` (cyan)
- La ligne source fautive
- Un caret `^` positionné sous la colonne exacte (rouge)
- La clause `expected` extraite du message Pest
- Un `= hint:` contextuel si applicable

Le fichier source est propagé depuis tous les points de chargement (`app.webc`, `layouts/`, `components/`, `pages/`) — chaque erreur indique son fichier exact.

**Couleurs ANSI :**

Les couleurs sont automatiquement désactivées si `NO_COLOR=1` ou `TERM=dumb` est posé dans l'environnement. La sortie reste lisible en texte brut (CI, pipes, logs).

**Cinq patterns de hints contextuels :**

| Pattern détecté | Hint affiché |
|---|---|
| Interpolation vide `{}` | `une interpolation vide \{\} n'est pas valide — utilisez \{varName\} ou \{expr\}` |
| Accolade fermante manquante | `accolade fermante \`}\` manquante` |
| Guillemets attendus | `une chaîne de caractères est attendue ici` |
| Expression JS attendue | `une expression JS est attendue après \`=\`` |
| Nom sans espaces attendu | `un identifiant sans espaces est attendu ici` |

**Suppression du préfixe redondant :**

La sortie de `webc build` n'affiche plus le préfixe `"Build failed:"` avant la liste d'erreurs — `CompileErrors::Display` affiche directement les erreurs puis le compte final.

### Performances internes v2.5.2

Ces optimisations n'ajoutent aucune fonctionnalité visible mais améliorent les performances
du compilateur et du runtime généré.

**`bindFor` — mutation DOM atomique**

La boucle `@for` sans `key=` utilise désormais un `DocumentFragment` + `replaceChildren(frag)`
au lieu de `innerHTML=''` + N appels `appendChild`. Résultat : une seule mutation DOM atomique,
les reflows intermédiaires entre insertions sont éliminés — bénéfice mesurable sur les listes
de plus de ~20 éléments qui se rafraîchissent fréquemment.

**`evalCond` — O(1) et regexes pré-compilées**

Deux optimisations dans le JS généré :
- `const VARS_SET=new Set(VARS)` — lookup O(1) pour les expressions qui sont exactement un nom
  de variable (le chemin le plus fréquent).
- `const _VR=[...VARS].sort(...).map(v=>[RegExp,...])` — les regexes de substitution pour les
  expressions composites sont construites une fois au chargement de la page (et non à chaque appel
  `evalCond`).

**SSG — `OnceLock<Regex>` et scanners passe unique**

- Les 3 regexes de `apply_ssg_with_locales` sont compilées une seule fois par processus (`OnceLock`).
- `html_unescape` et `html_escape_text` utilisent des scanners passe unique avec sortie anticipée
  (évite l'allocation d'intermédiaires lorsqu'aucun caractère spécial n'est présent).

**Compilateur — `resolve_slots` et regexes variables**

- `contains_slot()` court-circuite `resolve_slots` pour les sous-arbres sans slot → évite la
  reconstruction récursive de l'AST dans la majorité des nœuds d'un layout.
- Les N regexes de substitution des variables d'état sont compilées une fois par document au lieu
  d'une recompilation par expression.

---

## Nouveautés v2.6.0

### Fragment shorthand `<>...</>`

Groupe d'éléments sans balise wrapper. Compilé en nœuds HTML inline — aucune `<div>` intermédiaire dans la sortie. Voir [`<>...</>`](#--fragment-shorthand).

### Modificateurs d'événements

`on:click|stop`, `on:click|prevent`, `on:click|once`, `on:click|self` — contrôle du comportement du listener sans JS brut. Combinables avec `|`. Encodés dans `data-webcore-e`. Voir [Modificateurs d'événements](#oneventstoppreventselfonce--modificateurs-dévénements).

### Valeurs de props par défaut

`props { label: String = "Défaut" }` — si la prop est omise à l'instanciation, la valeur par défaut est injectée statiquement. Voir [Props — Valeurs par défaut](#props--valeurs-par-défaut).

### Commande `webc watch`

Rebuild automatique à chaque modification de fichier source, sans serveur de développement. Debounce 200 ms via `notify`. Voir [Commande `webc watch`](#commande-webc-watch).

### Analyse de bundle améliorée

- Détection du core bytes corrigée (`class State{` au lieu de `class _S`)
- Ajout de `bindClassBindings` et `evalCond` dans le tableau d'analyse post-build

---

## Limites actuelles (v2.7.0)

| Limite | Détail |
|---|---|
| Expressions dans `on:mount` | Le corps `on:mount { }` est du JS brut — pas de vérification de types ni d'analyse statique |
| SSG et `http {}` | Les requêtes `http {}` ne sont pas exécutées côté serveur — `loading` reste `true` au pré-rendu |
| WASM et SRI | Les assets WASM ne reçoivent pas encore de hash SRI |
| `$watch` multiple | Un seul `$watch` par variable par composant |
| Imports de données | Seuls JSON et TOML sont supportés (pas de CSV, XML, ou requêtes réseau) |
| Collections SSG | Seuls les champs de l'élément accessibles via `$route.<param>` sont pré-rendus ; les autres champs (`title`, etc.) sont résolus côté client |
| Collections SSG | Un seul paramètre `:param` par route de collection |
| CSP et `style-src` | `style-src 'unsafe-inline'` est requis pour le critical CSS inline — une CSP stricte sans `'unsafe-inline'` n'est pas possible tant que le critical CSS reste dans un `<style>` inline |
