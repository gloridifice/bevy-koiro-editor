<div align="center">

# Bevy Editor Prototype

</div>

<div align="center">

[English](../README.md) | [简体中文](zh-CN.md) | [日本語](ja-JP.md) | Français

</div>

Un éditeur de monde expérimental pour ordinateur, construit avec Bevy **0.20.0-rc.2**, BSN et des contrôles d’interface natifs, sans egui ni WebView.

Ce prototype explore les interactions, la disposition, le style et des idées d’éditeur de moteur de jeu avec Bevy UI. Il n’est pas destiné à être maintenu comme un éditeur utilisable en pratique.

<p align="center">
  <img src="screenshot_0.png" alt="Éditeur de bureau Koiro">
</p>

## Démarrage rapide

### Installation

Exécutez le projet depuis les sources avec Rust **1.96+** ; consultez [Compiler depuis les sources](#compiler-depuis-les-sources) pour le clonage et la compilation. Depuis la racine du projet :

```sh
cargo run
```

### Mise à jour

Depuis votre clone, après avoir effectué un commit ou un stash de vos modifications locales :

```sh
git pull --ff-only
cargo run
```

## Fonctionnalités

- Panneaux ancrables, dispositions prédéfinies et plusieurs fenêtres partageant un même monde.
- Hiérarchie du monde, édition des noms et des transformations, et gizmos de déplacement, rotation et mise à l’échelle.
- Navigateur d’assets avec modèles intégrés, aperçus d’images et aperçu de caméra en temps réel.

## Commandes et limites

Dans la vue 3D, faites glisser avec le bouton droit pour tourner autour de la scène, avec le bouton du milieu pour déplacer la vue, et utilisez la molette pour zoomer. Cliquez sur un objet pour le sélectionner ; faites glisser les poignées du gizmo pour modifier sa transformation. Les champs de l’Inspector sont validés avec Enter ou à la perte de focus ; Escape annule la saisie.

**Ctrl+S enregistre uniquement la disposition** dans `editor-layout.json`. Les modifications de la scène ne durent que le temps de la session. La sauvegarde de scène, l’annulation et le rétablissement, le mode jeu et l’édition générique des composants ne sont pas implémentés.

## Compiler depuis les sources

Git et Rust **1.96+** avec Cargo sont nécessaires. Bevy est fixé à la version **0.20.0-rc.2**.

```sh
git clone https://github.com/gloridifice/bevy-koiro-editor.git
cd bevy-koiro-editor
cargo build --release --locked
```

Exécutez `cargo run --release` depuis la racine du projet pour que l’éditeur puisse trouver `assets/`.

## Licence

Aucune licence n’est déclarée pour le projet dans son ensemble. Les icônes Lucide incluses conservent leur [licence d’origine](../src/lucide_icons/LICENSE).

## Développement

- [Architecture](../doco/architecture.md)
- [Contrats et limites de l’éditeur](../doco/specs/editor.md)
- [Guide de maintenance et de validation](../AGENTS.md)
- [Exemple d’interface BSN](../examples/bsn_ui.rs)
