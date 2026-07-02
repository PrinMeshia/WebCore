# Publier une version

Le processus est entièrement automatisé : **c'est le workflow qui crée *et*
publie la release.**

> ⚠️ Ne **pas** créer la release à la main dans l'interface GitHub.
> Avec les *immutable releases* activées, une release publiée depuis l'UI est
> figée *avant* que le workflow ait pu y attacher les binaires — l'upload
> échoue alors. (De plus, drafter une release dans l'UI ne crée le tag qu'au
> moment du « Publish », donc le déclencheur de tag ne se déclenche jamais
> pendant le brouillon.) Laisser le workflow tout faire.

## Déclencher une release

Deux façons, au choix :

1. **Pousser un tag de version** (recommandé) — sans préfixe `v`, au format
   `N.N.N`, identique à la version du `Cargo.toml` :

   ```bash
   git checkout main && git pull
   git tag 2.10.1
   git push origin 2.10.1
   ```

2. **Depuis l'interface** — onglet *Actions* ▸ *Release* ▸ *Run workflow*, en
   saisissant un tag **déjà existant** (ex. `2.10.1`). Utile pour relancer une
   release après correction.

## Avant de tagger

Sur `develop`, puis fusionné dans `main` (CI verte) :

1. Mettre à jour la version dans `webcore-compiler/Cargo.toml` (et `Cargo.lock`
   via `cargo build`).
2. Compléter l'entrée du [CHANGELOG](../CHANGELOG.md).

## Ce que le workflow fait

[`release.yml`](../.github/workflows/release.yml) :

1. **verify** — refuse le tag s'il ne correspond pas à la version du crate
   (garde-fou contre `git tag 2.10.1` avec un `Cargo.toml` resté à `2.9.0`).
2. **build** — binaire `webc` pour Linux, macOS Intel, macOS Apple Silicon,
   Windows.
3. **vsix** — empaquette l'extension VS Code.
4. **release** — crée une release **draft** (mutable), y attache les 4 archives
   + le `.vsix`, puis la publie. Le passage par un draft est imposé par les
   *immutable releases* de GitHub.

La release publiée apparaît avec ses binaires et des notes générées
automatiquement. Aucune action manuelle.

## En cas d'échec

- **« Tag … does not match crate version … »** : le tag et `Cargo.toml`
  divergent. Supprimer le tag (`git tag -d 2.10.1 && git push --delete origin
  2.10.1`), corriger la version, re-tagger.
- **« A published release for … already exists »** : un run antérieur (ou une
  release créée dans l'UI) a laissé une release publiée. La supprimer dans
  l'onglet *Releases*, puis relancer via *Run workflow*. Un *draft* résiduel,
  lui, est réutilisé automatiquement.

