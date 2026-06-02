# Spécification du langage WebCore

> Version : 0.7.0 — Référence complète de la syntaxe `.webc`

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
   - [View](#view)
   - [Style](#style)
8. [Éléments HTML](#éléments-html)
9. [Attributs](#attributs)
10. [Événements](#événements)
11. [Interpolation](#interpolation)
12. [Directives de contrôle](#directives-de-contrôle)
13. [Routage](#routage)
14. [Slot](#slot)
15. [Thème (`theme.toml`)](#thème-themetoml)
16. [Sortie compilée](#sortie-compilée)
17. [Internationalisation (i18n)](#internationalisation-i18n)
18. [SSG — Static Site Generation](#ssg--static-site-generation)
19. [WebAssembly (WASM)](#webassembly-wasm)

---

## Structure d'un projet

```
mon-app/
├── webc.toml              # Configuration du projet
├── theme.toml             # Tokens de design (optionnel)
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
title = "Mon Application"   # Titre HTML des pages
lang  = "fr"                # Attribut lang de <html>
mode  = "dev"               # "dev" ou "prod" (active la minification en prod)
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

Un seul `slot content` par layout. Le nom du layout doit commencer par une majuscule.

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
    props { ... }    // optionnel
    state { ... }    // optionnel
    view  { ... }    // obligatoire
    style { ... }    // optionnel
}
```

Le nom doit commencer par une majuscule. Un composant s'utilise comme un tag HTML :

```webc
NomComposant {}
NomComposant prop1="valeur" {}
```

### Props

Les props permettent de passer des valeurs statiques à un composant à l'instanciation.

```webc
component Greeting {
    props {
        name: String
        title: String
    }

    view {
        div {
            h2 "{title}"
            p "Bonjour {name} !"
        }
    }
}
```

Utilisation :

```webc
Greeting name="Alice" title="Madame" {}
```

**Comportement actuel :** les props sont substituées statiquement à la compilation
(les `{propName}` dans la vue deviennent des nœuds texte avec la valeur passée).
Les props de type expression (`prop={stateVar}`) ne sont pas encore supportées.

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
Seuls les sélecteurs CSS simples sont supportés (pas de `@media` pour l'instant).

```webc
style {
    div    { display: flex; gap: 1rem; }
    button { padding: 0.5rem 1rem; border-radius: 4px; }
    p.large { font-size: 2rem; }
}
```

Les sélecteurs `*`, `:root`, `html`, `body` ne sont **pas** scopés (globaux).

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

L'expression peut être n'importe quelle opération sur le state :

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

// Navigation
button on:click={webcore_navigate(/about)} { "Aller à propos" }
```

### Fonctions utilitaires disponibles dans les expressions

| Fonction | Description |
|---|---|
| `max(a, b)` | Maximum de a et b |
| `min(a, b)` | Minimum de a et b |
| `abs(x)` | Valeur absolue |

---

## Interpolation

### Dans les chaînes

```webc
p "Bonjour {name} !"
p "Total : {count + tax}"
p "Max : {max(a, b)}"
p "Résultat : {a * b + c}"
```

### Expression arbitraire

L'expression entre `{` et `}` est évaluée au runtime. Elle peut référencer
n'importe quelle variable du state et appeler `max`, `min`, `abs`.

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
    // contenu répété pour chaque élément de `items`
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

---

## Slot

`slot content` est le point d'injection du contenu de la page dans un layout.

```webc
layout MainLayout {
    main { slot content }  // contenu de la page ici
}
```

Un seul slot nommé `content` est supporté par layout.

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
├── index.html     # Page "home" avec layout
├── about.html     # Page "about" avec layout
├── theme.css      # Variables CSS + styles scopés des composants
└── webcore.js     # Runtime réactif (état, routage, événements)
```

### Runtime JS (`webcore.js`)

Le runtime généré contient :
- `class State` — état réactif avec `S.get(k)`, `S.set(k, v)`, `S.on(k, cb)`
- `evalCond(expr)` — évaluation sécurisée des expressions en substituant les variables
- `bindIf()` — réactivité pour `@if`/`@else`
- `bindFor()` — réactivité pour `@for`
- `bindAttrs()` — réactivité pour les attributs dynamiques
- `bind()` — réactivité pour les interpolations `{expr}`
- `nav(path)` — navigation SPA via `fetch` + `history.pushState`
- `H` — objet contenant tous les handlers d'événements compilés

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
- Si la validation échoue, `e.preventDefault()` empêche la soumission
- Les blocs `@error` sont masqués (`display:none`) par défaut ; le runtime les affiche avec le message d'erreur correspondant
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

La fonction `t("clé")` retourne la traduction de la locale active. Elle s'utilise dans toute interpolation :

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
const t = k => LOCALES[LOCALE]?.[k] ?? k;
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

<!-- HTML généré sans SSG -->
<p>Compteur : <span data-webcore-interpolation="count"></span></p>

<!-- HTML généré avec SSG (v0.5.0+) -->
<p>Compteur : <span data-webcore-interpolation="count">0</span></p>
```

### Affichage initial `@if`/`@else`

Le bon branchement est affiché dès le premier paint, sans attendre JavaScript :

```html
<!-- State initial : count = 0 -->

<!-- Sans SSG : les deux divs sont visibles jusqu'au DOMContentLoaded -->
<div data-webcore-if="count &gt; 0">...</div>
<div data-webcore-else="count &gt; 0">...</div>

<!-- Avec SSG : affichage correct immédiatement -->
<div data-webcore-if="count &gt; 0" style="display:none">...</div>
<div data-webcore-else="count &gt; 0" style="display:block">...</div>
```

### Compatibilité runtime

Le runtime JS (`bindIf`, `bind`) continue à opérer normalement après `DOMContentLoaded`.  
Il met à jour `el.style.display` et `el.textContent` de manière réactive — les styles SSG sont simplement écrasés par les mêmes valeurs, sans conflit.

### Expressions supportées pour le pré-rendu

| Type | Exemple | Résultat |
|---|---|---|
| Variable directe | `{count}` | valeur initiale de `count` |
| Littéral numérique | `{42}` | `42` |
| Addition/soustraction | `{count + 1}` | valeur + 1 |
| Variable de store | `{$store.hits}` → `{hits}` | valeur du store |
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

### Utilisation dans un composant

```webc
component Counter {
  state { count = 0 }
  view {
    button "Signer" onclick={count = wasm.sign_message(count)}
    p "Résultat : {count}"
  }
}
```

### Limites

- Un seul module WASM par projet (le premier `wasm/Cargo.toml` trouvé)
- `wasm-pack` doit être installé séparément (`cargo install wasm-pack`)
- Le loader est absent du bundle si `wasm/Cargo.toml` n'existe pas

---

## Limites actuelles (v0.7.0)

- Les props ne supportent que des valeurs `String` statiques (pas d'expressions réactives)
- Un seul slot (`content`) par layout
- Pas de `@media` queries dans les blocs `style { ... }` (à utiliser dans `theme.toml`)
- SSG limité aux expressions simples (variables, arithmetic ±, comparaisons numériques)
- i18n limité aux clés de type `t("key")` (pas de pluralisation, pas de paramètres)
- WASM : un seul module par projet ; `wasm-pack` requis séparément
