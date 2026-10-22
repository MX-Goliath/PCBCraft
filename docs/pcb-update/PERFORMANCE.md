# PCB editor performance — 22.10.2026

## Observed problem

The local Windows dev build repeatedly rebuilt display geometry, tested every
visible object against the pointer, and triangulated the board outline. A native
15,513-object, 23-layer PcbDoc research copy reproduced the reported slowdown.
Opening 3D also built the complete scene and rasterized it on the UI thread.

## Changes

- Bound document-space geometry caches to the document revision and explicit
  preview generation: bounds, board outline/triangles, padstack outlines, drills,
  and resolved paste/solder masks. Zoom, pan and selection reuse this geometry.
- Reject pointer candidates by cached bounds before exact hit testing. Exact
  layer-aware tests, ordering, masks and model geometry remain the same.
- Share the preview through Arc; connection results use generation keys instead
  of whole-board equality. Identical mouse-drag frames reuse the preview, while
  rotation, release and revision changes still update/validate/commit it.
- Use hashed selection membership and indexed handle lookup for large groups.
- Prepare 3D scenes and images on bounded background workers, coalescing changes.
  A result for another document revision/body setting is discarded. During camera
  movement the previous image may remain visible; picking waits for the matching
  camera, dimensions, selection and display settings. Worker failure is shown in
  the viewport. No GPU or GUI dependencies were added to eda-core.
- Calculate triangle projections/materials once before sorting the 3D image.
- Enable opt-level 2 in normal dev builds, including dependencies. Debug symbols,
  assertions and incremental compilation remain enabled on Windows and Linux.

## Reproduction and measurements

Run the read-only probe with a native board path:

```powershell
.\scripts\dev.ps1 -Task run -Package eda-app -ExtraArgs @('--example','pcb_frame_profile','--','INPUT.PcbDoc','--3d')
```

The probe uses an egui viewport of 1000×800, 15 px/mm, the board-outline centre,
default display configuration and default-font aliases. It measures CPU painting
and egui tessellation; it excludes file loading, other panels, GPU submission and
presentation. These numbers are not measured window FPS or an Altium comparison.

| Case | Original unoptimized dev build | Updated optimized dev build |
|---|---:|---:|
| 2D cold paint + tessellation | 201 ms | about 55 ms |
| 2D cursor, ten-frame mean | 148 ms | 22.5 ms |
| 2D pan, ten-frame mean | 153 ms | 23.0 ms |
| First 3D frame on UI thread | 14,765 ms | about 10 ms to request work |
| 3D scene + first image | synchronous, included above | about 869 ms in background |
| UI frames while preparing 3D | blocked | maximum about 0.9 ms in this probe |
| 3D orbit image refresh | synchronous | about 554 ms in background; UI frames ≤0.5 ms |

The build-profile change contributes to the total improvement. Before enabling
compiler optimization, geometry caching and broad-phase picking alone reduced
warm unoptimized 2D paint+tessellation from approximately 150 ms to 90–100 ms.
Timings vary with zoom, visibility, board contents and CPU load.

## Delay when releasing a moved object

The follow-up report was reproduced separately from frame rendering: committing
every copper edit synchronously called `repour` for all three polygons. On the
same 15,513-object board this took 1.38–1.43 seconds, in addition to move
validation, document cloning and history accounting.

The editor now commits the move immediately, retains the explicit stale-copper
flag, and starts one background repour per document. Ready geometry is applied
through a typed, validated History operation only for its source document and
revision. It refreshes derived copper without adding a user Undo entry or
clearing the Redo chain. New edits/Undo supersede old jobs; an active gesture,
dirty Properties draft or model-editing dialog defers result application.
Failures keep the stale flag, report the error once and do not retry endlessly.

Release reuses the validated preview when its point/rotation/revision match.
PCB commits copy the document envelope without cloning the board they replace.
The read-only `pcb_edit_profile` example compares the synchronous and deferred
paths, checks identical generated geometry, and checks one Undo/Redo round trip:

```powershell
.\scripts\dev.ps1 -Task run -Package eda-app -ExtraArgs @('--example','pcb_edit_profile','--','INPUT.PcbDoc')
```

Measured with the same optimized dev profile:

| Work | CPU time |
|---|---:|
| Previous synchronous repour on release | 1,378 ms |
| New commit of the ready preview | 9.1 ms |
| Snapshot for the background worker | 10.8 ms |
| Background repour | 1,355 ms |
| Apply validated result | 7.2 ms |

These are isolated model/history timings, not end-to-end input latency or frame
times. Explicit Repour All Polygons and native serialization of stale copper
still perform their requested rebuild synchronously. JSON saves retain the stale
flag and rebuild on open; completing a background refresh after saving does not
make a clean document dirty again.

Follow-up verification: 280 workspace tests, fmt --check, strict Clippy, Windows
dev build, Linux all-target cross-check. Windows GUI: via release displays
`Repouring polygons…`, the new position/Properties are available immediately,
and a single Undo/Redo restores/reapplies the move. The research copy was not
saved. Linux runtime input latency is not measured.

## Verification and remaining limits

Regression coverage checks geometry equality/reuse/expiry, preview rotation and
release/Undo, stale worker rejection, bounded pending work, texture reuse and 3D
navigation. Windows GUI verification uses the research copy only: Fit Board,
select/move via, Undo, scrolling, Page Up, 2D/3D switching and 3D zoom. The source Altium
project is not saved or modified.

Validation passed: 273 workspace tests, `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, Windows dev build and
Linux GNU all-targets cross-check (no Linux linking or distributable).

The complete 3D image still uses a software rasterizer. Its refresh can lag during
continuous orbit; background rendering makes the window responsive, but does not
establish a 60 FPS 3D renderer. Polygon repour, validated model copies during
editing, larger boards and other zoom/visibility configurations can still be
expensive. Full Altium rendering/performance parity is not claimed. Linux is
cross-checked with all targets; native Linux performance/UI is not measured and
no Linux distributable is built in this stage.
