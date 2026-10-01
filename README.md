<div align="center">

# Bevy Editor Prototype

</div>

<div align="center">

English | [简体中文](readme/zh-CN.md) | [日本語](readme/ja-JP.md) | [Français](readme/fr-FR.md)

</div>

An experimental desktop world editor built with Bevy **0.20.0-rc.2**, BSN, and native UI controls, without egui or WebView.

This prototype explores interaction, layout, styling, and game engine editor ideas with Bevy UI. It is not intended to be maintained as a usable editor.

<p align="center">
  <img src="readme/screenshot_0.png" alt="Koiro desktop editor">
</p>

## Quick Start

### Install

Run from source with Rust **1.96+**; see [Build from Source](#build-from-source) to clone and build. From the project root:

```sh
cargo run
```

### Update

From your clone, with local changes committed or stashed:

```sh
git pull --ff-only
cargo run
```

## Features

- Dockable panes, layout presets, and multiple windows sharing one world.
- World hierarchy, editable names and transforms, and move/rotate/scale gizmos.
- Asset browser with built-in models, image previews, and a live camera preview.

## Controls and Limitations

In the viewport, right-drag to orbit, middle-drag to pan, and scroll to zoom. Click an object to select it; drag gizmo handles to edit its transform. Inspector fields commit on Enter or blur; Escape cancels edits.

**Ctrl+S saves only the layout** to `editor-layout.json`. Scene edits last only for the current session. Scene saving, undo/redo, play mode, and generic component editing are not implemented.

## Build from Source

Requires Git and Rust **1.96+** with Cargo. Bevy is pinned to **0.20.0-rc.2**.

```sh
git clone https://github.com/gloridifice/bevy-koiro-editor.git
cd bevy-koiro-editor
cargo build --release --locked
```

Run `cargo run --release` from the project root so the editor can find `assets/`.

## License

No project-level license is declared. Bundled Lucide icons retain their [upstream license](src/lucide_icons/LICENSE).

## Development

- [Architecture](doco/architecture.md)
- [Editor contracts and limitations](doco/specs/editor.md)
- [Maintenance and validation guide](AGENTS.md)
- [BSN UI example](examples/bsn_ui.rs)
