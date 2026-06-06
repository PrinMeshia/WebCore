# Spécification du langage WebCore

> Version : 1.2.0 — Référence complète de la syntaxe `.webc`

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
23. [Commande `webc check`](#commande-webc-check)
24. [Limites actuelles (v1.2.0)](#limites-actuelles-v120)
25. [Nouveautés v1.1.1](#nouveautés-v111)
26. [Nouveautés v1.2.0](#nouveautés-v120)

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

## Limites actuelles (v1.2.0)

| Limite | Contournement |
|---|---|
| SSG limité aux expressions simples (variables, arithmetic ±, comparaisons numériques) | Le runtime résout les expressions complexes au chargement |
| WASM : un seul module par projet | Exporter toutes les fonctions depuis un seul `lib.rs` |
| Événements inter-composants portée globale (`document`) — pas de portée par instance | Préfixer le nom d'événement pour éviter les collisions |
| `@for key=` : clé doit être un chemin simple (`item`, `item.id`) — pas d'expressions JS arbitraires | Pré-calculer la clé dans l'état avant la boucle |
| Routes paramétrées : pas de routes imbriquées ni de regexp custom | Utiliser des routes à un seul segment dynamique par chemin |
| Objets littéraux interdits dans les expressions `on:click` (accolades ambiguës) | Définir un helper `window.mkObj = ...` dans `on:mount` |

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
