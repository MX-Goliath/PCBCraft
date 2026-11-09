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


## SchDoc opening after import - November 9, 2026

The user reports a loading indicator followed by an unresponsive pause before the
sheet appears. Static inspection found saved-snapshot construction after worker
completion, a second native-document snapshot, and synchronous opening of a
project's first schematic. A cold sheet also builds all object graphics in one
UI frame. These are candidate blocking paths, not measured attribution of the
complete reported delay.

The import worker now returns the normalized model and its saved snapshot.
Attaching them moves both values into the editor without cloning the document
again. Already resolved paths enter Recent without another filesystem lookup.
The first SchDoc in both new and previously loaded projects uses the same queued
import worker as an explicitly opened file. Reading/preparing phases remain
visible, with the existing one-worker cancellation and queue bounds.

Interactive opening opts into cooperative first-graphics preparation: up to 256
new objects or approximately 8 ms per paint pass, yielding between objects.
The sheet shows preparation progress and exposes no partial picking response
until its geometry is ready. Print/PDF bypass this policy and render complete
pages. Ordinary navigation keeps the existing rendering behavior. The bound
is cooperative: a single expensive primitive/image, mask analysis, junctions,
GPU/presentation or the final replay can still exceed that budget.

No rebuild or compiled tests were run, as requested by the user. Added regressions
cover snapshot ownership/clean state, queued project opening and complete rendering
after multiple preparation frames, including Print/PDF bypass. The updated
`schematic_frame_profile` source reports load time, snapshot construction, cold
frame count/CPU/max and warmed frame times for a later authorized build.

The existing, previously built diagnostic executables were run read-only on the
local reference scene (241 components, 2475 primitives, 5363 parameter objects).
They measured clone 3.820 ms and validation 0.813 ms. At sheet fit the warmed
pointer-frame paint mean was 1.004 ms and total mean including tessellation was
2.335 ms. These old binaries exclude this fix, native GUI/panels, GPU/presentation
and cold opening, and do not establish an improvement from the new changes.
Updated runtime and actual-window latency verification remain pending.

## Native pointer - November 9, 2026

The schematic crosshair previously hid the system cursor and drew black/white
lines at the pointer in each scene frame. It now requests the OS crosshair over
the focused canvas, including active tools. There is no software duplicate, so
ordinary pointer motion can proceed independently of painting/presentation.
Picking, snapping and placement previews retain their document-coordinate input.
The application also requests AutoNoVsync presentation with a one-frame latency
hint; support, driver and compositor behavior determine the actual presentation.

Cursor appearance/contrast/DPI now follow the OS theme rather than a forced
black/white halo. Source regressions were updated for light/dark documents,
100/150/200 percent DPI, tools and canvas/focus/modal boundaries, but were not
compiled or run because the no-rebuild instruction remains in force. Formatting
and static validation do not establish runtime visibility or latency. See
`pcb-update/PERFORMANCE.md` for presentation tradeoffs and remaining software
cursor styles; actual-window comparison and measured latency remain pending.
