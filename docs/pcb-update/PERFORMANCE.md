# PCB editor performance — 22.10.2026

## ReferenceM cursor and pan frame cost, 24.10.2026

The opening fix below did not fix continuous 2D frame rendering. The unchanged
41,475-item PCB reproduced about 59 ms for cursor frames and 49 ms for pans in
the CPU canvas probe. Each frame rebuilt roughly 92,000 shapes / 1.37 million
vertices. Separately, one full comparison with the saved document cost about
11 ms and was repeated by the title bar, tabs and project tree within one frame.

The editor now reuses that comparison for an unchanged History revision. Save
explicitly refreshes the comparison cache; pending gestures/properties are still
checked live. Undo/Redo and derived pour revisions still compare actual contents,
including returning to a saved state. An unchanged native window title no longer
issues another viewport command on every update.

The 2D renderer now keeps one tessellated scene per view, with a 256-point border
for short pans. Ordinary egui integrations reuse its meshes. The app's wgpu
integration keeps vertex/index buffers resident and changes a 32-byte camera
uniform instead of moving/re-uploading the whole board on every pan. The artwork
uses its own font-atlas snapshot so later UI glyph allocation cannot change its
UVs. Hatching stays screen-anchored and interleaved in the original draw order;
selection rectangles, tool outlines and handles remain live. Colors, visibility,
selection, layers, preview/revision, DPI, mirroring and zoom expire the cache.

A revision-bound spatial tree narrows pointer picking and snapping to nearby
objects. It retains exact hit/snap tests and document-order tie resolution. Its
envelopes include all copper pad-stack lands, offsets and primitive centres.
Snapping during a preview still uses the original board, including insertion or
deletion previews whose item indices differ.

Read-only release probe, same 1000×800 / 15 px/mm scene: GPU-callback submission
measured 2.8 ms for cursor frames and 2.5 ms for pans in the final run. The initial
scene build was about 352 ms under concurrent CPU load. `--gpu` measures CPU
submission only; it does not execute GPU commands and does not measure FPS.
Without it, the CPU mesh fallback measured roughly 12 ms for cursor frames and
24 ms for pans during development.

The real 1440×900 app window was exercised under KDE/Wayland through XWayland:
View/Fit Board, 360 cursor movements and 360 middle-button pan movements, without
editing or saving the reference. Once warm, eframe's CPU frame-time medians were
about 6–11 ms for cursor batches and 5–7 ms for pan batches. Cursor batch p95 was
12–19 ms; pan p95 was 8–11 ms. These are CPU frame costs, not measured presentation
FPS. Sampled GUI RSS was about 663 MiB. Timings vary with CPU contention, viewport,
zoom and source contents. `PCBCRAFT_FRAME_PROFILE=1` logs batches of 120 native
CPU frame samples for reproduction.

Regression coverage includes cache reuse/expiry, live tools, preview insertions,
spatial-vs-full snapping including internal lands, saved-state Undo/Redo, and
real offscreen GPU image comparison. The latter compares resident buffers with
egui's ordinary mesh renderer at DPI 1/1.5/2, fractional pan/viewport offsets,
clipping, transparency and gamma/sRGB targets, with a maximum 1/255 pixel-channel
difference. It also exercises the complete canvas/cache/GPU path with interleaved
hatching, and checks buffer reuse during pan. Workspace tests, strict all-target
Clippy and formatting pass; the Linux AppImage is rebuilt.

Cache construction still occurs on zoom, display/content changes and when a pan
leaves the cached border. Those frames can be slower. Object-editing previews,
very large boards, driver/presentation latency, Windows runtime performance and
full Altium pixel/FPS parity are not established by these measurements.

## PCB opening hang on ReferenceM, 24.10.2026

The Linux release AppImage hung while opening `ReferenceMBoard.PcbDoc` from
the user's ReferenceMHardware project, before rendering any 2D frame. Import eagerly
tessellates embedded STEP models even when 3D Layout Mode is disabled.
GDB located the allocation failure in Truck's recursive
`curve::sub_parameter_division`, called by `robust_triangulation` during import.
Several converted surfaces evaluate to NaN. The subdivision tolerance test never
succeeds for NaN, and its depth of 100 permits exponential point allocation.
The existing triangle-count limit runs after tessellation and cannot stop this.
An address-space-limited read-only reproduction reached 1,137 MiB peak RSS in
4.87 seconds and aborted on another allocation, without reaching the first frame.
This is a controlled reproduction, not the maximum memory use of an unrestricted run.

The importer now checks surface coordinates at domain endpoints/midpoints before
entering the trimming tessellator. Boundary curves use finite-coordinate checks,
iterative subdivision with depth 20, at most 16,384 evaluations per edge and
250,000 per model. A rejected model produces an import warning and retains the
existing simplified body-projection fallback; the original archive remains intact.
On this source, non-finite surfaces are reported for `SMB_nominal.stp`,
`128-0711-201.stp`, `PJ-002AHAD_PRT.stp`, `NDK_NX3225SA.stp` and
`Molex_1050170001.stp`. This does not establish that the source models are corrupt;
they cannot currently be represented safely by this STEP-to-kernel conversion.

Once that hang was removed, the same PCB exposed a separate import error:
`POURINDEX=-1`. This unassigned-order sentinel now uses stable source order,
like a missing pour index, and retains the native property. Other negative,
fractional and overflowing indices are still rejected.

Read-only Linux x86_64 release probe on the unchanged 41,475-item, 34-layer PCB:
load 892 ms; cold 2D paint+tessellation 206 ms; ten-frame cursor mean 58 ms;
pan mean 49 ms; complete probe 2.20 s and peak RSS 270 MiB. The probe now prints
loading time and 3D import warnings as well as frame timings. It excludes GUI
panels, windowing and GPU allocations. The rebuilt AppImage opened and rendered
the same PCB in the real KDE/Wayland session; observed GUI RSS settled near
564 MiB, with a sampled high-water mark of 635 MiB. These are different probes,
not interchangeable memory figures or a 60 FPS claim.

Regressions cover non-finite surfaces with finite STEP fields, NaN/Inf curves,
non-converging curves, evaluation budgets, line/closed-arc approximation,
unassigned pour order and source-property/archive preservation. Workspace tests,
strict all-target Clippy and formatting pass. The AppImage and SHA256 were rebuilt
and checked. Source PCB SHA256 remains
`redacted-source-sha256`.
This fixes the reproduced import hang; general STEP support and full-frame
performance on large PCBs remain limited.

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
