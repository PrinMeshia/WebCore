# WebCore Language Support

Extension VS Code pour le langage [WebCore](https://github.com/PrinMeshia/Webcore) (`.webc`).

## Fonctionnalités

- **Coloration syntaxique** complète (syntaxe 2.7.0) : blocs `app`/`layout`/`page`/`component`/`store`,
  directives `@if`/`@else`/`@for`/`@switch`/`@error`, fragments `<>...</>`,
  modificateurs d'événements (`on:click|stop`), props avec valeurs par défaut,
  `$watch`, imports de données, `@keyframes`, CSS nesting…
- **Snippets** : composants, pages, boucles, formulaires, i18n, et plus.
- **Formatage via `webc fmt`** : clic droit → « Mettre en forme le document »,
  ou activez `editor.formatOnSave` pour les fichiers `.webc`.

## Configuration

| Réglage | Défaut | Description |
|---|---|---|
| `webcore.formatterPath` | `webc` | Chemin du binaire `webc` |
| `webcore.formatIndent` | *(vide)* | Indentation forcée ; vide = respecte le `webc.toml` du projet |

## Installation

Téléchargez le `.vsix` attaché aux [releases](https://github.com/PrinMeshia/Webcore/releases)
puis : `code --install-extension webcore-language-<version>.vsix`
