# Project instructions

<!-- DOCO:START -->
<!-- doco:entry template=v1 -->
## Doco

For current project documentation and explicitly tracked changes, use the
`doco` skill. Read `.agents/skills/doco/SKILL.md` before creating, executing, completing,
or archiving a change. Perform only the requested phase.
A doco change package is optional for implementation and is not an
implementation-history mechanism: archiving retains only proposal.md. Routine
behavior fixes and implementation-detail changes may proceed without creating
a doco change unless the user explicitly requests tracking. Use Git, pull
requests, or release notes for implementation history. Regardless of tracking,
update current architecture and specs when their documented facts or contracts
change.
Start from `doco/architecture.md` and relevant current specs and decisions.
When implementing a selected tracked change, use its proposal and any present
design and task files. Treat completed changes, archived changes, and
`doco/tmp/` as non-current material; consult them only when explicitly needed.
<!-- DOCO:END -->

## Maintenance index

- [Current architecture and module boundaries](doco/architecture.md)
- [Current editor contracts and limitations](doco/specs/editor.md)
- Active change: [proposal](doco/changes/active/bevy-editor-shell/proposal.md), [research and prototype screenshots](doco/changes/active/bevy-editor-shell/work/research.md), [delivery checks and user acceptance](doco/changes/active/bevy-editor-shell/work/tasks.md). These describe tracked work, not a replacement for current documentation.
- [HTML reference prototype](prototype/index.html)
- [BSN UI example](examples/bsn_ui.rs): `cargo run --example bsn_ui`
- Native UI measurement: [snapshot example](examples/ui_audit.rs) and [analysis script](scripts/ui_audit.py)
- Icons: [rendering and cache](src/icons.rs), [embedded Lucide SVGs](src/lucide_icons), [upstream license](src/lucide_icons/LICENSE). SVGs are embedded at compile time and rasterized by `resvg` for the target window's DPI; the JSON metadata is not used at runtime.
- Public desktop screenshot: [readme/screenshot_0.png](readme/screenshot_0.png). Capture the actual editor window with `cutty`, inspect the PNG, and retain a full-resolution image.

## Validation

```sh
cargo test
cargo check --all-targets
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

For native UI measurements, install Pillow and run:

```sh
cargo run --example ui_audit
# Snapshot: <system-temp>/koiro-ui-audit.json
# Use a full-resolution PNG matching the snapshot's window, size, and DPI.
cutty --pid <PID>
python scripts/ui_audit.py <snapshot.json> <screenshot.png> --output <temp-output-prefix>
```

The audit reports node bounds, padding/gaps, clipping, collisions, contrast, desktop pointer target sizes, and pixel centroids, and writes an annotated image. Check overlays and scroll clipping against the screenshot; pixel symmetry is not a layout acceptance criterion. Automated checks do not replace user acceptance of GUI interactions and visuals.
