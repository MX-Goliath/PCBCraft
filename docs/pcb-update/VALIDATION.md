# PCB v2 integration validation — October 21, 2026

Integrated the user-supplied `PCBCraft-PCB-integration-v2-2026-10-21.zip`
onto base `bd761734cb5b2fae5461bb5bbec800efd5814bf9`.
Every `SHA256SUMS.txt` entry matched; original Git blob hashes of modified files
matched the package manifest. Transferred the overlay, Properties hook, synthetic
fixtures, documentation, verification script and code-provenance notices.

## Integration fixes

- Fixed borrow-after-move in `pcb_tools::edit_component`: select the mutable
  reference before moving the new component value.
- Updated the test application constructor for the new `pcb_clipboard`.
- Applied rustfmt and resolved Clippy findings without disabling checks.
- Added `geo 0.31.0` and its dependencies to Cargo.lock; existing dependency
  versions were not updated. `verify_pcb_update.py` uses `cargo fetch --locked`
  rather than regenerating the lockfile each run.
- The `pcb_inspect` report reflects the experimental writer and counts editable
  polygon definitions separately from original records.

## Automated checks performed

Linux x86_64, Debian 13, Rust 1.90.0, four parallel jobs:

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo build --locked --workspace --all-targets
python3 scripts/verify_pcb_update.py --report-dir /tmp/pcb-validation-unique
```

All checks passed. Ran 109 Rust tests: app 24, core 34, geom 1, IO 44, render 6;
no failures, skips or ignored tests. Empty doc-test groups are excluded.
All 22 Python builder tests from the original archive also passed; these check
packaging rather than Rust functionality.

Checks include geometry and transactions, routing preview/backtrack/via/splits,
polygons/cutouts/thermal/priority/islands, JSON migration and old-schematic regressions.
IO tests cover synthetic binary/ASCII files, unknown-stream preservation,
refusal of unsupported dependencies, backups and atomic replacement.

`pcb_roundtrip --exercise` on `fixtures/pcb-update/editor-v2.PcbDoc` changed a
component, Track, Pad and Via, added a track and changed polygon clearance with
repour. Saved 16704 bytes; the application's reader restored 9 primitives,
1 component, 2 nets and 1 polygon. The input fixture was unchanged.

## Linux GUI

A real wgpu/OpenGL window ran in Xvfb 1440×900 with software Mesa.
MIT-SHM was disabled for container X11; checks used a separate display and a
fixture copy outside the repository. Keyboard focus was set with `xdotool windowfocus`.

Checked board, pads, via, text, track and cutout display; R1 selection and
Component Properties; Comment `1k` → `2k2`, Apply, repour; Ctrl+S to `.PcbDoc`.
The first `.bak` matched the original fixture byte-for-byte.
Checked Ctrl+W, routing from J1.1 (25,10 mm) to (30,15 mm), Esc, Undo (new track
disappears), Redo and Save. Reimport preserved Comment `2k2` and two tracks;
they were displayed again after restart.

## Result boundaries

Verification used PCBCraft's own reader and synthetic files.
Altium Designer, real user boards and Windows were not checked in this environment.
Native-file compatibility with Altium and exact visual equivalence are unconfirmed.
This package does not implement DRC, obstacle avoidance, push-and-shove, ECO or
manufacturing outputs. The supported writer subset and blocking limitations are
in [IMPLEMENTATION.md](IMPLEMENTATION.md). The full PCB stage in PLAN.md is incomplete.

## Windows: opening ReferenceB with copper-free NPTH — October 21, 2026

Reproduced `Invalid PCB Pad geometry` when opening `ReferenceBBoard.PcbDoc` from
`ReferenceB.PrjPcb`. Pads6 record 121 (zero-based) contains Multi-Layer Pad `0`,
PLATED=false, zero top/middle/bottom land dimensions and positive drill 177165
native units (449999 nm, nominally 0.45 mm). The error was a model constraint,
not project-path resolution or dimension reading.

The model accepts this bounded case: an unplated through-hole without copper.
Zero-sized SMD pads, pads without holes, negative dimensions and zero-sized plated
pads are still rejected. Copper dimensions stay zero; the object is neither
removed nor replaced with a via. Bounds and picking include drill diameter.
The renderer creates no degenerate copper polygon and displays the hole outline,
which remains visible when switching layers. Polygon pours avoid the hole on
both layers without using it as an electrical pad even when the net matches.

Verified:

- 148 workspace tests, fmt --check, Clippy --all-targets -D warnings,
  Windows debug build and Linux GNU all-targets cargo check.
- Independently generated binary CFB with this pad: import, JSON Save/load,
  moving, native serialize/reimport and preservation of the opaque record tail
  and original source archive. The real board was not saved.
- Validation/bounds/picking regressions and pour hole avoidance on Top/Bottom.
- Real-board CLI: 15389 primitives, 69 components, 20 nets, 3 polygons;
  no Pad omitted (123 records).
- Real Windows window: project opening, double-clicking PcbDoc in Projects,
  board and tab appearing without import error. The old window with unsaved
  edits remained open; original PcbDoc unchanged.

This checks opening and the specified hole type. Accuracy of all layers, text,
padstack/slots, DRC and compatibility of the edited real board with Altium were
not confirmed here. Linux GUI and new Linux artifacts were not run.

## Windows: PCB visual comparison — October 21, 2026

Details and unfinished work: [VISUAL_PARITY.md](VISUAL_PARITY.md).
Full Altium parity is not claimed. This iteration checked 159 workspace tests,
fmt --check, strict Clippy, Windows debug build and the real board window;
Linux GNU all-targets check without Linux GUI/artifact release.
The model now contains 15513 records, including 124 2D body-projection records.
Both representations of every body/Region are retained for native IO without
being overlaid twice on screen.

Separate regressions check native layer overrides/fallback/tails, unchanged
3D-body-stream preservation, refusal to write a changed projection without 3D
transforms, hidden component strings, native radius percentages, font flags,
glyph geometry at three DPIs, hatch clipping and clockwise arc seams without
huge GPU triangles. The real board was not overwritten in native format;
edited real PcbDoc compatibility with Altium remains unconfirmed.

## Windows: PCB Properties — October 21, 2026

[PROPERTIES.md](PROPERTIES.md) separates native observations, implementation,
checks and remaining limitations. Contextual forms, origin-relative fields,
Comment, supported snap switches and History integration. 169 workspace tests,
fmt --check, strict all-targets Clippy, Windows build, Linux GNU all-targets check.
Real window: Board/XP1/Track/Via, diameter 0.7 → 0.8 mm and Undo/Redo/Undo.
Source board was not written. Linux GUI was not run.
Full Properties/3D padstack/rules/all-state parity is not claimed.

## Windows padstack / 3D / mask continuation — 2026-10-21

- 188 workspace tests: 38 app, 70 core, 1 geom, 56 IO, 23 render. Expanded table
  and 316 px layout, mask-only pour preservation, unchanged-mode geometry,
  slots/offsets, per-layer selection, Flip and bounded rule priorities/scopes,
  native stack/Rules6 preservation and refusal of unsupported lossy writes.
  Meshes include finite triangles, real cutouts, blind-via span and fast drag.
- Commands: `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`,
  Windows `cargo build -p eda-app`, Linux
  `cargo check --workspace --all-targets --target x86_64-unknown-linux-gnu`.
  The repository dev.ps1 supplies the local GNU toolchain/linker.
- Native research modified/saved only `.tools/pcb-padstack-reference.PcbDoc`;
  its untented snapshot is `.tools/pcb-padstack-untented.PcbDoc`. User source
  `ReferenceBBoard.PcbDoc` was never written. Both original user application windows
  and the old schematic research state were preserved.
- `.tools/pcb-padstack-roundtrip.PcbDoc`: generated by PCBCraft from the real
  source with Pad 2 Top Tented and an existing SolderMaskExpansion rule changed
  to 0.15 mm. AD 24.4.1 opened it without an error dialog. Visual inspection of
  Pad 2 confirmed top tenting on, bottom off; rule fields were reimport-checked,
  not exhaustively inspected in Altium. This is a bounded interoperability check.
- A copper geometry edit + repour on the same real board was correctly refused
  by native save: UniqueIDPrimitiveInformation remapping is not implemented.
  No destination bytes were returned or source changed. This limitation remains.
- Real Windows PCBCraft table/preview modes, rotation, Top Tented update and Undo/Redo/Undo were inspected; no edited QA board
  was saved to the user path. Linux compilation only; no Linux GUI or artifact.

The earlier Properties/display validation sections describe earlier milestones.
Current gaps and superseding padstack implementation are in PROPERTIES.md.
