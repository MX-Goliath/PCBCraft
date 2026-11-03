# Schematic interaction performance

The November 2026 investigation found document-wide identity searches inside
pointer-move previews, all-pairs wire overlap discovery, and exhaustive wire
searches for every automatic junction. An active tool also requested continuous
frames and rebuilt its preview while the pointer was stationary.

Implemented changes:

- Transaction-local identity maps for preview differences, patch application,
  owner validation, hierarchy/directive reconciliation and component parameters.
- Indexed selection roots and grouped extraction preserve selection/child
  order, hidden parameters, text roles and only referenced embedded images
  without scanning the full sheet for every selected component.
- Integer R-tree bounds for wire edges. Exact document-coordinate predicates
  still decide overlap and electrical connectivity. Overlap cleanup traverses
  only the connected overlap closure of changed wires. Unrelated imported
  overlaps, styles, hidden/locked objects and bare crossings retain their rules.
- Connected Drag indexes moving points, electrical anchors and branch wires.
  Wire joining rejects unrelated endpoints before copying paths or comparing
  cloned styles; stem identities use a transaction-local occupied-ID index.
- A single-entry preview cache keyed by document, monotonic history revision and
  edit. Undo/Redo invalidate it. An unchanged preview no longer requests frames;
  eventless follow-up frames do not recreate placement UUIDs indefinitely.
- The same validated core commands still generate previews and commit edits.
  Validation, atomic failure and history remain enabled.

`crates/eda-app/examples/schematic_edit_profile.rs` is a read-only CPU probe:

```powershell
cargo run -p eda-app --example schematic_edit_profile -- PATH_TO_SCHDOC
```

The workspace defaults to one Cargo build job in `.cargo/config.toml` after a
parallel all-target rebuild exhausted memory on the 16 GB Windows machine.
This limits concurrent compiler/linker peaks; it does not change editor runtime
parallelism. `cargo clean --profile dev` reclaimed 29.1 GiB of generated debug
artifacts before rebuilding. Release artifacts and source projects were retained.

It measures clone/validation, Move/Drag, all-component group Move/Drag, wire insertion,
Duplicate/Delete/Rotate, text/visibility, wire vertex drag, history and CPU frame
construction/tessellation. A timing is the mean of five sequential operations;
frame measurements follow a warm-up. Commit timing includes a fresh document
clone and history, and Undo/Redo timing covers a complete pair. No file is saved.
The group geometry diagnostic separates connected-transform calculation from
preview normalization/validation. The frame probe uses fallback egui fonts, a
1280 x 900 canvas and the fitted sheet.
It excludes native window event delivery, dock panels, GPU execution and display
latency. It is a diagnostic, not a complete end-to-end latency benchmark.

Regression coverage includes an exhaustive junction reference (negative and
diagonal geometry, zero/reversed edges, hidden/manual dots and colour ordering),
transitive overlap cleanup with remote objects preserved and Undo/Redo, history
cache invalidation, group extraction against independent single-object results,
and real egui input/frame scheduling through preview commit. Consumed keyboard
events still update a stationary preview (including Space rotation).

The corpus confirms lower CPU edit costs, but large connected group operations
still exceed a 60 Hz frame budget. This work does not establish constant latency
for arbitrary documents or every combination of native object types.

Final CPU samples are in `schematic-performance-2026-11-03.json`; workspace
results are recorded in PLAN.md. Windows
Computer Use returned `Computer Use native pipe is unavailable: failed to connect
native pipe` on both discovery attempts. Current real-window interaction and
Linux runtime performance therefore remain unverified.
