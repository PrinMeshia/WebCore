![logo_webcore](https://github.com/PrinMeshia/Webcore/blob/main/Webcore.png)
# WebCore

**Un langage unifié pour le développement web**

WebCore est un langage de programmation qui unifie HTML, CSS et JavaScript dans une seule syntaxe déclarative. L'objectif : simplifier le développement web en éliminant la fragmentation entre les technologies.

## 🚀 État actuel

**Version :** MVP fonctionnel  
**Status :** En développement actif  
**Compilateur :** Rust + Pest parser

### ✅ Ce qui fonctionne

- **Parser Rust** : Grammaire Pest pour analyser les fichiers `.webc`
- **AST structuré** : Représentation interne des composants, états, vues et styles
- **Codegen** : Génération de HTML/CSS/JS à partir de l'AST
- **CLI basique** : `webc build` et `webc dev` fonctionnels
- **Pipeline de build** : Transformation et optimisation des assets

### 📝 Syntaxe actuelle

```webc
component Hello {
    state name: String = "World"

    view {
        <h1>Hello {name}!</h1>
        <button>Click me</button>
    }

    style {
        h1 { color: blue; }
        button { padding: 0.5rem; }
    }
}
```

## 🛠 Installation et utilisation

### Prérequis
- Rust 1.70+ avec Cargo

### Build
```bash
git clone https://github.com/PrinMeshia/Webcore.git
cd Webcore
cargo build --release --bin webc
```

### Utilisation
```bash
# Compiler un fichier .webc
./target/release/webc build --input examples/basic.webc --out dist

# Serveur de développement
./target/release/webc dev --input examples/basic.webc --out dist --port 3000
```

## 🏗 Architecture technique

```
.webc files → Pest Parser → AST → Codegen → HTML/CSS/JS → Transformers → dist/
```

### Composants clés

- **`grammar.pest`** : Grammaire du langage WebCore
- **`src/parser.rs`** : Parser qui transforme le texte en AST
- **`src/ast.rs`** : Structures de données pour représenter le code
- **`src/codegen.rs`** : Génération de code HTML/CSS/JS
- **`src/transformers.rs`** : Post-traitement (SWC/LightningCSS - stubs)

### Pipeline de compilation

1. **Parsing** : Analyse syntaxique avec Pest
2. **AST** : Construction de l'arbre syntaxique
3. **Codegen** : Génération de modules ES2025 + HTML
4. **Transformers** : Optimisation JS/CSS (prévu)
5. **Output** : Fichiers prêts pour le navigateur

## 📁 Structure du projet

```
Webcore/
├── src/
│   ├── bin/webc.rs          # CLI principal
│   ├── parser.rs            # Parser Pest
│   ├── ast.rs               # Structures AST
│   ├── codegen.rs           # Génération de code
│   ├── transformers.rs      # Post-traitement
│   └── config.rs            # Configuration TOML
├── examples/
│   ├── basic.webc           # Exemple simple
│   └── state_only.webc      # Exemple minimal
├── grammar.pest             # Grammaire du langage
└── webc.toml               # Configuration par défaut
```

## 🎯 Prochaines étapes

### Court terme (1-2 semaines)
- [ ] Parser des vues HTML avec interpolation `{variable}`
- [ ] Parser CSS complet avec sélecteurs et propriétés
- [ ] Support des expressions binaires dans les états
- [ ] Tests unitaires pour le parser

### Moyen terme (1-2 mois)
- [ ] Intégration SWC pour l'optimisation JavaScript
- [ ] Intégration LightningCSS pour l'optimisation CSS
- [ ] Hot reload en mode développement
- [ ] Support des composants multiples

### Long terme (3-6 mois)
- [ ] Système de routing natif
- [ ] Gestion d'état globale
- [ ] Support WebAssembly
- [ ] Extension VSCode avec syntax highlighting

## 🔧 Développement

### Ajouter une nouvelle fonctionnalité

1. **Modifier la grammaire** : `grammar.pest`
2. **Étendre l'AST** : `src/ast.rs`
3. **Mettre à jour le parser** : `src/parser.rs`
4. **Adapter le codegen** : `src/codegen.rs`
5. **Tester** : Créer un exemple dans `examples/`

### Exemple de contribution

```bash
# Créer un exemple de test
echo 'component Test { state count: Number = 0 }' > examples/test.webc

# Tester la compilation
cargo run --bin webc -- build --input examples/test.webc --out test_dist

# Vérifier le résultat
ls test_dist/
```

## 🤝 Contribution

Les contributions sont les bienvenues ! Voici comment contribuer :

1. **Fork** le projet
2. **Créer une branche** : `git checkout -b feature/nouvelle-fonctionnalite`
3. **Commiter** : `git commit -m 'Ajouter nouvelle fonctionnalité'`
4. **Pusher** : `git push origin feature/nouvelle-fonctionnalite`
5. **Ouvrir une Pull Request**

### Guidelines

- Code en Rust avec des commentaires clairs
- Tests pour les nouvelles fonctionnalités
- Exemples dans `examples/` pour les nouvelles syntaxes
- Documentation des changements dans les commits

## 📄 Licence

todo

## 🙏 Remerciements

- [Pest](https://pest.rs/) pour le parser
- [Clap](https://clap.rs/) pour le CLI
- La communauté Rust pour les outils et l'écosystème


