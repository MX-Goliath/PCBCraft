# PCB editor performance — 22.10.2026

## PCB 2D zoom stalls, 25.10.2026

Changing zoom invalidated the complete screen-space artwork on every frame.
The same read-only ReferenceM, 41,475 items and 34 layers, reproduced a 49.56 ms
mean CPU paint+tessellation time across 120 zoom frames, p95 54.55 ms and
maximum 128.62 ms. 119 of 120 frames exceeded 33 ms. The baseline is the parent
commit with the same new zoom sequence added to `pcb_frame_profile`.

Pan and zoom now transform resident vertex buffers with a camera uniform.
After 120 ms without another scale change, one background worker rebuilds exact
screen-space detail, including antialiasing, minimum stroke widths, curve
subdivisions and label thresholds. The previous vector artwork follows the
current camera immediately while that job runs. Grid, origin, tools and hatching
remain live in their original draw order; picking and snapping use the current
camera and document geometry. During a gesture, cached antialias widths and label
visibility scale with the previous artwork until refinement completes.

The worker owns a render snapshot and an independent egui context with matching
fonts, DPI and options. On the app's wgpu integration it also prepares GPU
buffers, atlas and pipeline off the UI thread. Results are published only for
the current content/display key and exact requested scale. New requests coalesce;
a content change retains any older running job until it exits, discards its
result, and prevents parallel refinement jobs for that view. The CPU fallback
remaps worker texture IDs into its UI context. A failed worker retains the last
scaled artwork until content/display changes reset refinement.

Release, Linux x86_64, RX 7900 XTX / RADV Vulkan, 1000×800, initial 15 px/mm:

- CPU-only GPU-callback submission probe: zoom mean 7.62 ms, p95 14.48 ms,
  maximum 18.56 ms; zero of 120 zoom frames exceeded 33 ms.
- Completed offscreen GPU zoom frames: median 2.46 ms, p95 5.59 ms, maximum
  9.09 ms. The following idle/refinement phase measured median 2.93 ms,
  p95 4.42 ms and maximum 6.29 ms, including publication of the rebuilt scene.
- The native eframe window exercised animated zoom, two settled scales and a
  framebuffer screenshot. After the first second, CPU median was 2.20 ms,
  p95 13.38 ms and maximum 51.60 ms; six of 1,530 samples exceeded 33 ms.
  These runs overlapped compilation and other probes. The native capture was
  visually inspected after refinement. These are CPU costs/offscreen GPU
  completion times, not presentation FPS or a claim that every native frame is
  below 33 ms.

Startup and content/display changes still build synchronously. In these runs,
arrival of background connection results triggered a separate cold rebuild,
including a 295 ms frame in the CPU long-pan phase. Snapshot creation adds cold
frame cost and retained memory. Arbitrary-board/extreme-zoom memory bounds,
Windows GPU runtime and Altium pixel parity remain unverified. The source PCB
SHA256 remains `redacted-source-sha256`.

Regressions check continuous zoom without rebuilds, exact refined-vs-fresh
geometry including lettering and thin tracks, stale scale/content rejection,
bounded workers across content changes, disconnected workers, and GPU pixels
and buffer reuse at fractional scales, pan, DPI 1/1.5/2 and gamma/sRGB targets.
Workspace tests, strict all-target Clippy and formatting pass. The rebuilt
AppImage opens this PCB in the real desktop session and its SHA256 sidecar
matches. The already running older AppImage needs to be restarted.

Reproduce with the existing read-only probes:

```sh
rtk bash scripts/dev.sh run --locked --release -p eda-app --example pcb_frame_profile -- INPUT.PcbDoc --gpu
rtk bash scripts/dev.sh run --locked --release -p eda-app --example pcb_3d_frame_profile -- INPUT.PcbDoc --2d
rtk bash scripts/dev.sh run --locked --release -p eda-app --example pcb_3d_frame_profile -- INPUT.PcbDoc --2d --window --capture /tmp/pcbcraft-zoom.png
```

## Oblique 3D transparency artifacts, 24.10.2026

The user screenshot showed stripes and large triangular color patches across
ReferenceM under an oblique camera. Ordering transparent triangles by the depth of
their centres cannot order overlapping, differently triangulated planes at each
pixel. The earlier radix-sort optimization retained that approximation. A new
GPU regression with two parallel translucent sheets and opposite diagonals
reproduced a 28/255 channel error under orthographic projection before the fix.
The CPU reference rasterizer uses the same centroid ordering, so matching it on
simple single-translucent-layer tests did not establish correct multilayer
transparency.

The GPU renderer now peels translucent surfaces by pixel depth. Each pass rejects
opaque occluders and previously peeled depths, writes the nearest remaining
surface, then accumulates its premultiplied color front to back. Two depth targets
alternate and reuse one layer-color target; accumulation uses RGBA16Float. Two
float ULPs separate passes to prevent the same surface from blending again through
depth interpolation roundoff. Opaque geometry still renders once with hardware
depth. Transparent indices upload on styling/selection changes, and camera
movement no longer sorts or re-uploads those indices on the CPU.

The pass count is bounded to eight transparent depth layers per pixel. This fixes
the reproduced PCB masks/substrate ordering; arbitrary stacks with more than
eight translucent surfaces are not fully represented. At 1800×1400 the extra
targets require about 48 MiB of GPU storage. Memory is bounded by viewport size,
not the number of transparent triangles. Antialiasing and native material/pixel
parity remain separate limitations.

The regression checks correct compositing through triangle seams in orthographic
and perspective projections, against the known physical sheet order rather than
the faulty centroid-sort reference. Existing depth, opacity, selection, picking
and buffer-reuse regressions continue to pass. The read-only native eframe probe
captures a fixed oblique pose for reproducible visual inspection; ReferenceM no
longer shows the large stripe/triangle patches in that capture.

Release 1000×800 completed-GPU probe, RX 7900 XTX / RADV Vulkan, same 2,183,154
triangles, alongside native capture: orbit median 3.66/p95 4.20/max 6.12 ms;
pan median 1.63/p95 3.10 ms; zoom median 5.07/p95 6.02 ms. These are completed
offscreen frames, not measured presentation FPS. Peak child RSS was about 656 MiB;
initial scene plus first GPU frame was about 3.8 seconds. Workspace tests,
strict all-target Clippy, formatting and rebuilt AppImage verification complete
the check; the source PCB/project are not edited.

## ReferenceM 3D orbit latency, 24.10.2026

The centroid/radix transparency order described below was replaced by the
per-pixel depth peeling follow-up above.

The 3D viewport still used a CPU rasterizer after the 2D fixes. A read-only
release reproduction on ReferenceM measured 782 ms to refresh the image after one
orbit change. Moving that work to a background thread kept the UI responsive,
but left the picture approximately a second behind the camera.

The app's wgpu integration now uses resident 3D vertex buffers, a 128-byte camera
uniform, hardware depth and a cached offscreen color/depth target. Material and
selection changes update a shared style table; pan/zoom do not rebuild geometry
or upload vertices. Opaque triangles use the depth buffer without CPU sorting.
Only translucent triangles need back-to-front ordering when orientation or
thickness changes. A stable four-pass radix sort replaces comparison sorting;
cached triangle centres retain the piecewise board-thickness transform. The
renderer retains premultiplied gamma colors, per-face lighting, native material
opacity/colors, layer dimming, highlighting and parent-component selection.
Pointer picking traverses a revision-bound triangle BVH and tests the current
camera ray, including perspective, bottom view and thickness scaling. It does
not wait for a software image or use a depth image from an old camera.

The same source produces 2,183,154 triangles and 17,510 shared styles, with
1,852,098 opaque and 298,824 translucent triangles in its saved configuration.
On the local AMD Radeon RX 7900 XTX / RADV Vulkan adapter, the new release
1000×800 offscreen probe submits work and waits for actual GPU completion:

| Case, 120 completed frames | Median | p95 | Maximum |
|---|---:|---:|---:|
| Cached camera | 0.76 ms | 0.90 ms | 2.02 ms |
| Continuous orbit | 6.12 ms | 8.18 ms | 9.67 ms |
| Continuous pan | 0.55 ms | 1.27 ms | 1.53 ms |
| Continuous zoom | 1.44 ms | 2.98 ms | 4.27 ms |

The orbit CPU median was 5.34 ms, including transparency ordering. Before radix
sorting it was 13.11 ms. Scene preparation plus the first GPU frame still took
about 3.7 seconds; the scene remains built in a bounded background worker. The
final build with device-capability checks repeated orbit at median 6.59/p95
8.52 ms, pan 0.56/zoom 1.55 ms. A
native eframe window probe also ran automatic orbit/pan/zoom without GPU errors;
sampled RSS was about 693 MiB. Initial GPU preparation was about 39 ms. Offscreen
completed-frame times and native CPU timings are not presentation FPS, and
performance on other GPUs/Windows is not established.

The native frame was captured directly from the application's own framebuffer
and inspected. The native probe animates its own read-only camera; manual
desktop input testing was unavailable. Workspace tests, strict all-target
Clippy and formatting pass, and the Linux AppImage is rebuilt.

Reproduce without changing the board:

```sh
rtk bash scripts/dev.sh run --locked --release -p eda-app --example pcb_3d_frame_profile -- INPUT.PcbDoc
rtk bash scripts/dev.sh run --locked --release -p eda-app --example pcb_3d_frame_profile -- INPUT.PcbDoc --window --capture /tmp/pcbcraft-3d.png
```

GPU tests compare interior pixels against the previous CPU rasterizer for
orthographic/perspective, top/bottom, thickness, hidden/translucent/opaque
materials and selection, within 3/255 per channel. They check vertex-buffer
reuse; the full canvas test checks scene/renderer reuse and absence of software
raster jobs during navigation. BVH picking is compared with raster depth through
flips, oblique views, perspective, pan/zoom, thickness and material visibility.
Radix ordering is checked against stable numeric sorting with equal depths.
The asynchronous software renderer remains available for integrations without
the wgpu callback, devices without vertex storage buffers, and scenes exceeding
the device's buffer limits. Its orbit latency is unchanged. Arbitrary-scene memory
budgets, first-scene latency, antialiasing/specular effects, exact native pixel
parity and support for the previously rejected STEP models remain open.

## Intermittent pan stalls on ReferenceM, 24.10.2026

The initial viewport cache below removed the continuous low FPS but rebuilt the
entire scene synchronously whenever a pan crossed its 256-point margin. A new
360-frame sweep spanning 2,400 logical points reproduced 16 frames over 33 ms,
with a maximum CPU paint+tessellation time of 158 ms. A ten-frame short-pan mean
did not expose those stalls. The frame probe now reports p95, maximum and the
number of frames over 33 ms, and accepts `--zoom PX_PER_MM`.

The cache now captures all visible-layer artwork, including objects initially
outside the viewport, at a fixed document anchor. A camera translation or canvas
resize reuses the same meshes and GPU buffers regardless of distance. The fixed
anchor also keeps egui's pixel rounding independent of the camera position when
the scene was built. The grid and origin marker stay live in their original draw
order, alongside tools and screen-anchored hatching. Grid clipping intersects the
board outline once per row instead of testing every edge for every grid point.
Offscreen hatch regions are rejected before generating their screen pattern.

A second unnecessary invalidation used the identity of egui's shared font atlas,
which egui replaces when it fills up. Cached PCB glyphs already own a private
atlas snapshot. The key now tracks font definitions rather than shared atlas
allocation, while retaining DPI invalidation. A regression explicitly replaces
the UI atlas, checks that the artwork is reused, then changes font definitions
and checks that the artwork rebuilds.

On the same read-only 41,475-item PCB, release GPU-callback CPU submission at
15 px/mm produced zero frames over 33 ms in the 360-frame sweep. One isolated
development run had a maximum of 2.34 ms; a later run alongside compilation and
native GUI QA had p95 5.20 ms and maximum 8.05 ms. At 150 and 1,500 px/mm the same
screen-distance sweep also had zero frames over 33 ms. These probes exclude GPU
execution and presentation. Peak headless RSS in those successive child runs was
about 322 / 482 / 877 MiB respectively: whole-scene memory and cold build cost
increase with zoom and displayed detail. This change does not establish bounded
memory for arbitrary boards or extreme zoom.

The actual 1440×900 XWayland window was exercised with 360 cursor movements,
360 short-pan movements and 720 long-pan movements made as repeated middle-button
drags. No artwork rebuild was logged during either pan phase. Warm short-pan CPU
frame batch medians were 2.4–2.7 ms, maxima below 4 ms; long-pan batch medians were
about 1–2.7 ms, with a maximum of 7.75 ms. Sampled GUI RSS was about 672 MiB.
Startup and explicit Fit Board rebuilds are excluded from those warm figures;
the startup batch included a 414 ms frame. These are CPU costs, not measured FPS.
The source project was not edited or saved.

GPU image comparisons now include large pans that reveal initially offscreen
artwork, a live grid and an origin initially outside the viewport. A fresh scene
at each destination is compared with the translated resident scene, with at most
1/255 difference per pixel channel. Cache tests cover long translation without
rebuilds and grid scanline agreement with the model's even/odd rule at vertices,
edges and concave rows. Zoom, display/content changes and editing previews still
require synchronous scene construction; Windows runtime and full Altium
visual/performance parity remain unverified.

Packaging also exposed `Text file busy` when overwriting the currently running
AppImage. The builder now packages a temporary file on the destination filesystem
and atomically replaces the output only after success. The running instance keeps
its old executable until restarted. The rebuilt image passes a Wayland launch
smoke check and its SHA256 sidecar matches.

## ReferenceM cursor and pan frame cost, 24.10.2026

This first fix used a viewport border cache; the follow-up above replaces it.

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

### Windows complex-board opening, 26.10.2026

The reported failing PCB is on another device and is not available here. A
separate read-only reproduction on local ReferenceCRevision2 (27.6 MB) stalled
before the first 2D frame. Import profiling located it in Truck's robust
triangulation of shell 2353 in BOURNS_INDM135125X65N_SRP1265A.STEP. The initial
probe consumed over 56 CPU seconds without returning. Finite source-edge checks
did not cover the new PCurves generated internally to close trimming loops.

The surface adapter now shares 250,000 evaluations and a three-second deadline
across one model's shells, including generated trimming PCurves. Surface grids
use bounded subdivision and at most 16,384 grid positions; the same 0.02 mm
sampling tolerance is retained. Exceeding the budget unwinds into an explicit
model warning, preserves archived STEP bytes and uses the existing body
projection. This is a bounded approximation, not a claim of complete BRep
conversion or equivalence to Altium's tessellator. A 15-second library budget
stops starting additional models; it is checked between models and is not a
hard process timeout. Failed models are cached as well as successful models.
Parser/topology operations remain subject to the existing input limits and
are not isolated in a killable process.

Interactive File/Open, recent entries, project-tree and startup document
requests run on one worker. A bounded queue coalesces paths. Progress shows the
file, current import phase and elapsed time; Cancel clears queued requests,
sets a cooperative cancellation flag and prevents publication. The worker slot
remains occupied until the cancelled job actually exits, preventing repeated
cancels from starting unlimited CPU jobs. Synchronous internal dependency
lookups for ECO/cross-probing retain their existing behavior.

The ReferenceC probe now returns an independent region-geometry compatibility error
instead of stalling in STEP. Other local large samples reveal separate padstack,
parameter-owner and plane-alias compatibility errors. These are not silently
accepted or counted as successful opening. The user's remote PCB still needs
verification. The portable package includes Diagnose.cmd: close other PCBCraft
instances, launch it and open the file, or drag a PCB onto it. Import phases and
STEP model timings are written locally to Data/diagnostic.log.

Regression coverage invokes Truck's actual PCurve subdivision with an exhausted
shared surface budget, checks planar and curved grid approximation, non-finite
surfaces/deadlines, cancelled imports and the single-worker queue. Stage results
and real-window verification are recorded in PLAN.md.

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


## Movement and 3D rebuilds, 2026-10-25

Measured against commit `dfc36a7`, using Rust 1.90.0 release builds on the same
Ryzen 9 5900X / RX 7900 XTX Linux machine. Sources were opened read-only;
ReferenceA and ReferenceM SHA256 values remained unchanged. Raw probe output,
conditions and source hashes are in `performance-2026-10-25.json`.

### Changes

- A movement session validates its source once, then transforms and validates
  only its selected entities. It reuses its draft allocation while immutable
  worker snapshots use copy-on-write. Release still passes the complete board
  through History validation. Invalid moves, locked objects, cancellation,
  rotation, Undo and Redo retain their transaction semantics.
- 2D artwork keeps stationary GPU buffers during move, rotation, connected drag
  and handle editing. Live items, masks, drills and labels retain their original
  paint order. Changing selection/display or editing polygon definitions still
  rebuilds the appropriate artwork. Connections render separately; background
  connection calculations share immutable previews instead of cloning each one.
- Movement refits the spatial tree and expires only changed outlines. Snapping
  queries a separate index of the original board during previews.
- Connected dragging checks its selected primitives instead of rescanning all
  board objects for every candidate endpoint. Neighbour replacements share one
  draft and receive one final validation instead of cloning/validating per track.
- 3D placement parameters, component-side lookup and trigonometry are prepared
  once per model instance. Substrate construction is shared between mask films.
  Rebuilds reuse substrate/mask faces when their dependencies match, and reuse
  unaffected item faces when neither their source nor nearby physical cuts
  changed. Rules, layer stack, component metadata, polygons and outline changes
  invalidate the relevant results. Cut queries use an R-tree; fully internal
  artwork on convex boards avoids unnecessary clipping. BVH construction
  computes triangle bounds and partition keys once.

### Measurements

The 2D probe uses 1000×800, 15 px/mm and 60 movement samples. These values are
CPU canvas/tessellation times with GPU callbacks, not presentation FPS. Session
construction is outside the repeated samples. The maximum includes the first
artwork rebuild when the gesture starts.

| Board | Objects | Move frame median before / after | Frame p95 before / after | Updated frame maximum |
|---|---:|---:|---:|---:|
| ReferenceA | 1,032 | 15.89 / 0.41 ms | 23.09 / 0.79 ms | 14.29 ms |
| ReferenceM | 41,475 | 284.94 / 4.57 ms | 318.61 / 8.09 ms | 319.31 ms |

The separate model-update median changed from 2.81 ms to below 0.01 ms on ReferenceA,
and from 53.20 ms to below 0.01 ms on ReferenceM. Publishing the updated preview
changed from 1.46 to 0.02 ms and from 13.96 to 1.44 ms respectively. These are
separate medians, not a measured combined frame/input latency.

The 3D probe waits for scene preparation and an actual completed offscreen GPU
frame. Edits are cumulative translations of an unlocked track, via, then
component by 1 mm in X and Y; the original file is never saved.

| Scene preparation plus first GPU frame | ReferenceA before / after | ReferenceM before / after |
|---|---:|---:|
| First entry | 743.30 / 120.77 ms | 3819.00 / 4174.46 ms |
| After track movement | 738.04 / 51.49 ms | 3753.22 / 725.78 ms |
| After via movement | 753.30 / 117.76 ms | 3716.51 / 2642.68 ms |
| After component movement | 727.36 / 75.78 ms | 3716.45 / 1721.64 ms |

First entry on the large board did not improve; its roughly 2.2 million triangles
still require a complete build and upload. A moved drill invalidates physical
substrate and mask surfaces, so that rebuild remains expensive.

### Verification and reproduction

398 workspace tests, strict all-target Clippy and fmt --check pass. Tests compare
prepared edits with ordinary fully validated commands, preserve retained worker
snapshots after subsequent moves, reject invalid/locked edits, compare refitted
spatial queries with fresh trees, and compare incremental 3D faces with complete
builds after track/via/component/mask/layer/outline edits, deletion and restoration.
GPU readback compares live movement against a full rebuild with at most two
8-bit channel levels allowed for shader dithering/quantization.

Real eframe windows were run and their captures inspected. The fixed ReferenceA 3D pose
is pixel-identical before/after. During continuous native movement, ReferenceA CPU frames
had median 0.66 / p95 0.84 / maximum 1.64 ms; ReferenceM had 6.20 / 20.10 / 23.23 ms.
These samples exclude the first second. The large-board probe also recorded
initial frames of 309 and 72 ms. Local captures are retained under
`target/pcb-performance-2026-10-25/`. The rebuilt Linux AppImage was launched with
the editor fixture and painted its canvas successfully.

```bash
rtk bash scripts/dev.sh run --locked --release -p eda-app --example pcb_frame_profile -- INPUT.PcbDoc --gpu --drag
rtk bash scripts/dev.sh run --locked --release -p eda-app --example pcb_3d_frame_profile -- INPUT.PcbDoc --edits
rtk bash scripts/dev.sh run --locked --release -p eda-app --example pcb_3d_frame_profile -- INPUT.PcbDoc --2d --drag --window --capture /tmp/pcb-drag.png
```

Remaining costs include initial selection/gesture artwork, copy-on-write when a
worker still retains a snapshot, polygon editing/repour, initial large 3D scenes,
and globally changed substrate/mask surfaces. Constant 60 FPS across all boards
and operations is not established. Windows runtime performance was not measured
in this iteration. Existing visual/STEP/import limitations remain in features.json.

## Routing obstacle queries on Windows, 2026-10-25

The route solver previously scanned every obstacle for every candidate segment,
including the forty binary-search steps in Stop mode. It now uses an R-tree of
model-space bounds and a conservative clearance bound, then applies the existing
pair-rule and geometry checks to candidates in original board order. Room/matrix
clearances, live fallback clearance changes, globally blocking rule errors and the
1 nm distance tolerance remain covered. Floating envelope arithmetic avoids i64
overflow when calculating areas of board-sized boxes; valid nanometres remain
exactly representable. No GUI/GPU dependency was added to the core.

The reproducible `pcb_route_query_profile` example uses 40,000 remote tracks and
one local obstacle stored last, with 120 cursor positions for each conflict mode.
Measured on Windows, Ryzen 7 8845H, optimized dev profile, Rust 1.90.0:

| Solver CPU time | Before median / p95 | After median / p95 |
|---|---:|---:|
| Stop | 4.0011 / 7.1388 ms | 0.0084 / 0.0100 ms |
| Walkaround | 1.9503 / 2.6895 ms | 0.0056 / 0.0062 ms |

The summed route-leg counts remained 239 and 539 respectively. Preparation
increased from 12.234 to 27.768 ms because of index construction. These are
isolated model calculations on one synthetic layout, not frame rate or total
input latency. Larger overlapping regions can still dominate exact distance
checks. Full-board preview copies/validation and artwork rebuilds remain open.

Read-only native ReferenceB probe (15,513 items): startup previously failed on
`CompParameterValue`; after parameter import/query support, preparation took
21.77 ms, three solves 4.76/3.84/3.96 ms, and preview/place/finish/Undo 79.96 ms.
This is not a successful-route timing comparison with the previous version.
The native Windows GUI also started NetC13_2, placed a leg, finished and returned
to a clean document with one Undo. Source SHA256 stayed unchanged.

Raw measurements, conditions and limitations:
`routing-performance-2026-10-25.json`. Reproduce the synthetic workload with:

```powershell
.\scripts\dev.ps1 -Task run -ExtraArgs @('--example','pcb_route_query_profile')
```

## Reusable route previews on Windows, 2026-10-25

Moving the routing cursor used to clone and validate the whole board, publish
another complete preview, then rebuild all artwork. RouteSession now owns a
private validated prefix and updates an immutable, copy-on-write preview buffer
with stable IDs for the active legs. Repeated identical requests do no work.
Only new geometry/references are checked while hovering; placing, changing
layers, backtracking and committing still validate the complete transaction.
Retained snapshots remain unchanged when another cursor position is published.

The renderer retains the prefix spatial tree and unaffected pad/drill/mask
contours. Source-track splits are refitted; variable tails are queried separately
and painted in their original layer/type order. Cached GPU artwork survives
zero/one/two active legs, placements, vias and backtracking. Entering a route can
also reuse the displayed source geometry, including after a cleared preview;
other previews and document revisions require rebuilding it.

Connectivity workers now receive an owned geometry-only snapshot rather than
retain the editor's Arc<Board>. This avoids forcing a deep copy on every cursor
update while a worker runs. Filtering, effective polygon nets, pad/via stacks and
geometry order are preserved; native metadata and component parameters are not
copied into that worker input.

Live artwork uses the same document anchor as cached tessellation. This fixes a
one-pixel stroke shift caused by rounding at opposite sides of zero. Screen-phase
region hatching retains its existing coordinate behavior. GPU readback compares
optimized and complete redraws in both linear and sRGB targets, with masks,
highlighting, single-layer modes, draft, mirroring, splits, vias and backtracking.
Maximum channel difference is one level without dithering and three with the
production dithering on overlapping meshes. Stationary scene buffers remain
shared during the tested tail changes.

The read-only ReferenceB workload contains 15,513 items, 69 components and 60 cursor
positions on ExampleGround. At 1000 x 800 logical pixels and 15 px/mm, the original
combined CPU median was 57.5230 ms (p95 64.6967). The first optimized run measured
5.7226 ms (p95 9.0993), including target lookup, model, publication, canvas and
tessellation. With alternating straight/bent tails it measured 6.3729 / 9.2021 ms.
These are headless CPU submissions: GPU callbacks are not executed, and presented
FPS or total input latency is not measured. Cold preview construction remained
90.7778 ms in that run; one of 60 samples exceeded 33 ms. Subsequent measurements
after source-geometry reuse and the full-redraw control are retained separately
in `route-preview-performance-2026-10-25.json`.

After source-geometry reuse, the final regular workload measured median 7.1928 /
p95 9.6066 / maximum 72.3750 ms; changing tails measured 7.6839 / 10.3159 /
62.3378 ms. The sequential full-redraw control in the same final executable
measured 83.2955 / 103.6451 / 112.6602 ms. The differing steady medians between
runs show timing variability; they are not evidence of a fixed hardware speedup.
The new regression also verifies retained source contours after a nonzero prior
preview generation, split-track refitting and rejection of unrelated previews
and revisions. Cold optimized frames still exceed 33 ms once in each run.

Validation: 449 workspace tests, fmt --check, strict all-target Clippy, Windows
build and Linux GNU all-target compilation. Real Windows ReferenceB window: routing
NetC13_2, placement, Backspace (8.173 -> 7.845 mm), placement again, Finish,
Undo/Redo/Undo to a clean document. No source save; SHA256 stayed unchanged.
The first test used the preceding preview build; the final geometry reuse build
also passed Fit Board, NetC13_2 preview/placement, Finish and Undo to a clean
document. Both source hashes match. Linux execution and
new AppImage packaging were not performed.

First-frame artwork, original-board snapping preparation, full validation on
placement/Finish, exact distances to large polygons, changed display/layer
states and retained external snapshots can still stall. Full editor performance
and exact Altium behavior/layout remain open.

```powershell
.\scripts\dev.ps1 -Task run -ExtraArgs @('--example','pcb_route_preview_profile','--','INPUT.PcbDoc')
# Add --varied for changing leg counts, or --full-preview for the full-redraw control.
```

## Shared snapping candidates during previews, 2026-10-25

The original-board snapping cache previously rebuilt bounds, component maps,
the outline and a second spatial tree when the first preview was displayed.
It now shares only the source frame's immutable spatial tree. Source-track
splitting and transformed geometry refit through copy-on-write, so the snap
candidates retain the original document coordinates and indices. The source
revision controls lifetime; a preview arriving before a source frame prepares a
spatial-only fallback. No additional Board snapshot is retained.

Off mode skips item preparation/query and still calls the core grid snap.
Ctrl+Shift continues to return the raw document point. Real canvas event tests
compare all modes, grid on/off and bypass against complete model snapping on
both cold and warm preview entry, including points near the original track end
and the new preview end. Multilayer pad extents, source split refits, retained
snapshots, fallback and revision/Undo invalidation are also covered.

The profiler now records the first sample explicitly. This change removes the
duplicate construction, verified by tree identity, but the whole-frame timings
do **not** establish further latency improvement. The current regular ReferenceB run
measured median/p95/first 6.7688/9.2336/65.6447 ms; the immediate baseline was
4.8551/6.4533/51.1992 ms. The changing-tail run measured 7.7865/10.9781/42.9343 ms.
CPU frequency/background scheduling were uncontrolled, and GUI presence differed.
All runs retain one combined sample above 33 ms; stationary artwork and model
preparation remain cold-frame costs. Timing excludes GPU execution/presentation.
Raw summaries and conditions: `route-snapping-performance-2026-10-25.json`.

451 workspace tests, formatting, strict all-target Clippy, Windows build and
Linux GNU all-target compilation passed. In the native Windows ReferenceB window,
Snap Options changed Off -> Current Layer, NetC13_2 preview/placement and Finish
worked, and Undo restored a clean document. Off was restored afterward. Source
SHA256 remained unchanged and no PcbDoc was saved. No new Altium comparison,
Linux runtime verification or AppImage was performed.

## Reusing stationary artwork groups on route entry, 2026-10-25

Artwork previously had one rebuild boundary: making polygon copper stale on
route entry retessellated masks, pads, labels and all other stationary layers.
The cache now follows the existing paint order with separate board, layer/type
artwork, mask, detail and polygon-outline groups. Source captures include the
route-tail slots, so unchanged groups can be used by the first preview as well
as subsequent ones. Stale copper and source-track splits invalidate their own
groups. Display, font, DPI, history and incompatible-preview changes still
invalidate sharing; different tessellation scales cannot share these groups.

Shared groups retain their exact meshes, private atlas handles and GPU scenes.
A reused scene can have its first mesh omitted, so every submitted callback can
initialize/update that scene's camera; identical uniforms are not rewritten.
CPU zoom refinement remaps each shared mesh once to the UI atlas. Group parts
reference resources directly instead of retaining ancestor Cache objects.
A weak-reference regression verifies expired source GPU resources are released
after style invalidation. Retained generations still consume RAM/VRAM; peak
memory on arbitrary boards has not been established.

On the full ReferenceB workload, 151 of 155 groups survived the stale-copper transition.
The final two headless runs rebuilt artwork in 1.99 and 2.62 ms; their original
source artwork builds took 30.44 and 38.47 ms. Combined regular CPU samples had
median/p95/first 7.2775/10.5462/37.1903 ms; changing tails measured
8.1933/10.3123/33.3280 ms. Initial group-cache runs were faster, with first
samples 21.6751/22.0493 ms; all results are retained. The preceding shared-snap
baseline first samples were 65.6447/42.9343 ms. CPU frequency/background activity
were uncontrolled, and these are submissions without GPU execution, not FPS or
end-to-end latency. One final sample in each run still exceeded 33 ms.

GPU readback compares group reuse with a complete redraw in linear/sRGB targets,
with source pad glyphs, holes and masks, splits, fresh/stale copper, vias,
backtracking, draft, highlighting, single-layer modes and mirroring. It also moves
the camera while mesh zero from a retained scene is absent. Channel tolerances
remain <=1 without dithering and <=3 with production dithering on overlaps.
452 workspace tests, fmt --check, strict all-target Clippy, Windows build and
Linux GNU all-target compilation passed.

Real Windows ReferenceB window: Fit Board/refinement, start NetC13_2, preview motion and
stale dimming, place, Backspace, place, Finish and Undo to a clean document.
At entry, 155/155 groups were reused in 3.52 ms; the stale transition reused
151/155 in 5.39 ms. Source SHA256 stayed unchanged, with no PcbDoc save.
Finish/repour/Undo still changed document revisions and rebuilt all groups;
native artwork costs reached 67.69 ms. Native frame batches include startup and
action outliers and do not establish continuous routing FPS.

Source splitting can still rebuild a whole affected group. Cold model snapshots,
commit/validation/repour, global display changes, generic previews, large boards,
peak memory, Linux execution and exact Altium parity remain open. Conditions,
all raw summaries and the native log are in
`artwork-groups-performance-2026-10-25.json`.


### Polygon update, 2026-11-05

The earlier synchronous-explicit-repour limitation below the background-pour
checkpoint is historical. Explicit Repour now uses the cancellable background
path, including failed-revision retries and progress. Large-board booleans use
batched obstacles and conservative spatial pruning; derived results are validated
on the worker before application. See [POLYGON_POURS.md](POLYGON_POURS.md) and the
2026-11-05 PLAN.md entry for component-move regressions, measurements and limits.

## Routing and editing latency, 2026-11-05

The endpoint picker now prepares a conservative spatial index once per routing
session, then applies the existing exact distance, layer/span, pad precedence and
document-order tie rules to candidates. The source revision guards its lifetime.
Stop-mode bisection memoizes repeated integer-rounded segments; the same forty
steps and clearance tests remain in effect.

Keyboard Move and component/pad Drag reuse validated movement storage, including
previously attached track endpoints. Pointer component Drag uses the same path.
Repeated stationary requests reuse their preview. Pad/via/text/fill/line/circle,
rectangle, arc and region placement reuse an immutable source and a small tail;
stationary GPU artwork and spatial data are shared. Commit still validates the
complete transaction and records one history edit. Prepared geometry is compared
against complete transactions, retained snapshots and invalid/locked edits.

Connectivity retains one compact source geometry snapshot per document revision.
Preview jobs copy changed primitives, assemble unchanged references on the worker,
preserve document order and exclude stale generated copper. Superseded jobs are
cooperatively cancelled and cannot publish obsolete airwires. The worker does not
retain the editor's mutable preview allocation. Delta/full connectivity and GPU
preview/full-redraw regressions cover the new paths.

JSON input uses a buffered reader, complementing the buffered output path. The
read-only `pcb_route_preview_profile` reports loading, session preparation, endpoint
lookup, model preview and CPU canvas/tessellation separately. Its optional
`--linear-target` measures the old picker; `--ignore`/`--stop` choose solver modes,
and `--verify-connectivity` separately checks compact connectivity equivalence.
`pcb_interaction_profile` compares full/prepared Move, connected Drag and placement
without writing the input and checks exact final transaction geometry.

These changes do not establish universal frame rate or Altium performance parity.
Cold session preparation, commit/history validation, source revisions and initial
artwork still cost time. Obstacle-solving track Drag, polygon/outline edits and
Paste retain complete transaction paths. GPU execution/presentation, panel work,
input scheduling, arbitrary-board peak memory and Linux runtime latency are not
measured by these CPU probes. Polygon boolean work remains asynchronous and can
take seconds on complex boards; see POLYGON_POURS.md.


Final Windows observations (milliseconds; medians unless stated). Matched route
runs use `--varied`, the same five offsets repeated sixty times, and include one
cold preview. Input A has 41475 items / 74 pours; B has 31257 items / 2 pours.
The baseline overlapped compilation; final probes ran serially without a compiler.
CPU frequency and other system activity were uncontrolled.

| Route probe | ReferencePolygonA, before -> after | ReferencePolygonB, before -> after |
| --- | ---: | ---: |
| Endpoint lookup | 1.8581 -> 0.0204 | 13.5689 -> 0.0051 |
| Combined CPU canvas sample | 22.0497 -> 16.0570 | 15.8140 -> 2.6226 |
| Combined p95 | 25.5634 -> 17.9064 | 34.1364 -> 4.2145 |
| First combined sample | 131.3446 -> 123.6284 | 194.8257 -> 177.2776 |

Final route session preparation was 1021.1 / 391.4 ms, outside these samples.
A separate regular workload with tiny vertical offsets remains more expensive on
A: median/p95/first 22.1150/31.3522/159.2329 ms. B regular samples were
2.9204/4.3692/260.3610 ms. These are different route legs, not matched baseline
comparisons. Raw phase summaries, both workloads and intermediate observations
are preserved in `editing-performance-2026-11-05.json`.

| Model operation, full -> prepared | ReferencePolygonA | ReferencePolygonB |
| --- | ---: | ---: |
| Move | 127.0483 -> 0.0682 | 103.8880 -> 0.0254 |
| Component connected Drag | 194.7244 -> 0.0615 | 104.2085 -> 0.0253 |
| Via placement | 124.1951 -> 0.0004 | 102.2498 -> 0.0005 |

These twelve-sample transaction comparisons exclude drawing and initial
preparation. Preparation still took 85.9-122.4 ms. Both large inputs produced
exactly the same final Board as complete transactions. The 107 MB poured
research JSON loaded in 1404.3 ms with buffering, against 176074.4 ms before;
its final varied route CPU median/p95 was 19.5264/21.3136 ms.

Final formatting, strict workspace/all-target Clippy and all 765 workspace unit
tests passed (app 174, core 327, geometry 1, IO 149, renderer 114); documentation
tests passed. Windows exe and both diagnostic examples built successfully.
The existing STEP interpretation diagnostic on ReferencePolygonB remains.


Native Windows QA after unlocking: current exe, one GUI. On editor-v2, moving the
whole component updates the pour; route preview/placement/Finish and via placement
work, with one Undo each returning a clean document. On ReferencePolygonA, Edit
-> Drag moves a whole component through a live preview, stretches attached tracks,
commits, starts automatic 74-pour regeneration and completes. One Undo restores
original component/copper and a clean document. A separate Top-pad routing preview
moves with the pointer and is dismissed without committing copper. The research
tab was closed cleanly; the app-created default example was discarded. The sole
remaining GUI shows clean editor-v2. No native research file was saved. This is
functional verification, not measured native FPS or an Altium equivalence claim.

## Pointer responsiveness, 2026-11-05

The PCB hides the system pointer and paints the configured Small 90, Small 45
or Large 90 cursor at the raw mouse position. Presentation delay therefore
affects the visible cursor even when snapping and CPU geometry are fast.
The native app now requests a maximum presentation latency of one frame through
egui-wgpu, retaining its default AutoVsync mode. This is a driver-dependent
request, not a measured input-to-photon guarantee or a cursor-style change.

Polygon boundary picking now caches the same flattened outer contour used by
core selection and indexes its segments per scene revision. Exact segment
distance still decides hits; document order, visibility/lock filters, holes and
empty-interior selection behavior are unchanged. Footprint interior checks use
the existing item-ID index rather than scanning the entire item array for each
hit. Read-only component hover also preserves the source-cache provenance needed
to reuse geometry when routing starts.

The read-only CPU frame probe adds a 480-point cursor sweep over a fixed
1000x800 canvas at 15 px/mm, with GPU callbacks enabled but no GPU execution.
Separate completed offscreen Vulkan GPU measurements on ReferencePolygonA
preceding this change gave cached median/p95 6.62/12.91 ms. Those samples exclude
native panels, the compositor, presentation queues and mouse-device latency.
Native CPU timings likewise do not measure end-to-end cursor latency.
Matched final CPU sweep measurements, with both versions run serially after
compilation and with the native GUI closed:

| CPU paint + tessellation, ms | A before | A after | B before | B after |
| --- | ---: | ---: | ---: | ---: |
| Mean | 6.16 | 5.61 | 1.59 | 1.14 |
| p95 | 8.23 | 6.87 | 2.39 | 1.33 |
| Maximum | 17.62 | 8.35 | 3.87 | 1.99 |

Each case has 480 samples and zero samples above 33 ms. CPU frequency, thermals
and other system activity were uncontrolled. These results measure CPU picking
and drawing preparation, not the effect of the presentation-queue setting.
Raw summaries and conditions are in editing-performance-2026-11-05.json.

Final fmt, strict workspace/all-target Clippy and all 766 workspace unit tests
passed (app 174, core 327, geometry 1, IO 149, renderer 115); documentation tests
passed. The existing empty-polygon canvas test exposed addition without a
revision change; cache preparation now also detects polygon-count changes.
The Windows exe and both frame probes built. After the user unlocked Windows
again, native QA passed on ReferencePolygonA in the sole current GUI: Fit Board,
raw cursor positions over empty canvas, dense copper and a polygon edge; routing
from a Top pad, blocked-clearance and valid preview updates; Escape cancellation
and return to ordinary selection with a clean document. No research source was
saved. These snapshots establish functional input/painting, not input-to-photon
latency or perceptual equivalence to Altium.

Cold routing remains a visible limit. A mixed 120-frame native CPU batch spanning
route startup and preview had median/p95/max 9.42/25.83/1106.60 ms; it includes
command preparation, panels, painting and idle frames, and is not a steady
pointer-motion benchmark. Subsequent cancellation/redrawing also produced a
mixed-batch maximum of 499.66 ms. Presentation latency is excluded from these
eframe CPU timings. The current clean board remains open for user evaluation.

## Connected track drag, 2026-11-05 follow-up

The current native window recorded repeated connected-track clearance/connection
failures. A failed frame previously removed the last valid preview, and release
then discarded the gesture. Native CPU batches also contained multi-second
updates. The complete solver cloned the full board and reconstructed obstacles
for each selected track, neighbour, bridge and final clearance check.

Single assigned-net track body, endpoint and keyboard Drag now own one immutable
source and reusable working board per gesture. Obstacle geometry and its spatial
index are shared, keyed by layer/width; foreign copper remains stationary and
same-net copper is excluded by the existing solver. Room-dependent clearances
are still resolved per proposed leg. Changed source slots and appended legs are
restored between requests, with stable preview IDs and copy-on-write protection
for retained snapshots. Only edited geometry needs per-frame validation;
finish and History still validate the complete transaction.

Repeated constraint attempts also reuse UUID-to-source-slot indices for transforms
and replacements. The indexed replacement helper retains legacy stack
reconciliation, generated-copper/locked-owner restrictions and the locked-item
Unlock-only rule; mismatched or missing slots fail without mutation. Assigned-net
previews skip irrelevant No Net contact scans and collect final edited geometry
from source slots and the appended tail rather than scanning the entire board.

A blocked requested position is clipped towards the last valid connected
position using at most twelve refinements. Release commits the displayed valid
geometry; cancellation discards it. Changing conflict/clearance options starts
the constraint search from the original source rather than retaining a position
accepted under the previous options. Unassigned and multiple-net selections
retain the complete transaction path. This is a bounded connected rerouter,
not a claim of Altium's full angle-preserving slide, shove or gloss behavior.

Renderer bounds and spatial picking now refit source slots restored by a later
preview, including returning a dragged track to its original position. Regression
coverage compares prepared results with complete transactions, retained immutable
snapshots, fixed pads/branches, variable widths and Room rules; verifies blocked
preview release and Undo; and compares restored display bounds/queries with a
fresh cache. Read-only measurements and final native verification follow below.

Free-space routing has a separate hot-path fix: No Net endpoint contact used to
scan every source primitive on each pointer update. It now queries the session's
prepared source index and checks the small appended tail in document order.
Splitting only shortens source tracks; the original envelopes, expanded for
native quantization, remain conservative. Exact hit distance, pad precedence,
No Net contact permissions and tie ordering are retained. Exhaustive comparisons
cover split tracks, new route segments, vias, layer changes and backtracking.
The read-only route probe adds `--free-start` to exercise this path explicitly.

Serial optimized-debug measurements on ReferencePolygonA (41,475 items), with
compilation finished and the GUI closed, before the cold-start follow-up:

| Case | First prepared model, ms | Warm model median / p95 / max, ms |
| --- | ---: | ---: |
| Connected track, reachable request | 1086.77 | 9.18 / 11.03 / 11.06 |
| Connected track, constrained request | 1027.53 | 9.62 / 36.16 / 61.30 |

Each warm case contains 30 varied requests. The reachable complete transaction
took 6617.76 ms; the prepared final geometry matched it after normalizing newly
generated UUIDs. The constrained complete request failed, whereas the prepared
preview retained a connected legal position. Both prepared commits undid to the
exact source. History, drawing and presentation are excluded from these model
timings. Cold construction is included only in the first column.

New routing, 60 varied requests per case, includes CPU canvas/tessellation:

| Case | Begin, ms | Combined CPU median / p95 / max, ms |
| --- | ---: | ---: |
| No Net free-space start | 1081.608 | 8.2596 / 15.6975 / 117.6719 |
| Assigned-net start | 1209.307 | 20.6578 / 35.8100 / 135.8274 |

The first paint/model samples are retained. These probes execute no GPU
callbacks and do not measure native input-to-photon latency. Commit, Undo and
source-file reopening passed without saving either source.

The 15:38 Windows build passed fmt, strict workspace/all-target Clippy and all
773 unit tests plus documentation tests. In its sole native GUI, a connected
track was dragged and placed, a new track preview moved and was placed/finished,
and each operation was undone exactly once. The user's pre-existing unsaved
edits remained visible. The user confirmed improved movement and reported a
remaining initial pause; that cold-start issue is tracked separately below.

## Cold route preparation, 2026-11-05 follow-up

Phase profiling confirmed repeated rule work during obstacle construction.
Within one immutable board/probe pair, maximum clearance bounds now cache by
object kind and cutout status. Priority-sorted electrical rules lazily cache
the fixed probe's scope results, retaining symmetric evaluation, net gating,
unsupported-query diagnostics and net-tie behavior. Room contours normalize
once per construction snapshot; each obstacle still receives its own membership
test. Moving route legs continue to resolve their live scopes. These caches
do not outlive the board snapshot and do not suppress malformed Room errors.

Serial measurements after compilation, while the earlier GUI stayed open and
idle to preserve the user's unsaved edits:

| Case | First preparation, ms | Warm model median / p95 / max, ms |
| --- | ---: | ---: |
| Connected track, reachable request | 367.51 | 7.02 / 8.51 / 9.25 |
| Connected track, constrained request | 390.50 | 7.20 / 29.99 / 53.11 |

Both commits undid exactly; reachable geometry matched the complete transaction.
The complete reachable Walkaround solve took 1892.80 ms. New-route begin took
344.184 ms (assigned) and 345.854 ms (No Net). Combined CPU model/canvas median /
p95 / max were 23.3816 / 32.0509 / 117.2442 ms and 6.8051 / 13.3157 / 106.1936 ms,
respectively, retaining first frames. These are a separate checkpoint from the
earlier GUI-closed measurements; thermals, frequency and scheduling are uncontrolled.

With phase instrumentation enabled in a separate assigned-net run, 4036 obstacle
objects used seven gap-bound keys: bounds 0.454 ms, pair rules 128.065 ms, geometry
69.259 ms, complete environment 202.163 ms, complete begin 391.546 ms. Remaining
startup work includes board cloning/validation, exact pair queries and geometry,
and initial preview/index construction. The initial pause remains perceptible;
these measurements establish neither a universal frame rate nor native latency
or full Altium equivalence. Regression coverage includes symmetric/unsupported
scopes, priority/net gating, concave/invalid Rooms and mixed-object/cutout matrix
clearances compared with exhaustive spatial queries.

Final cold-start gates: fmt, strict workspace/all-target Clippy and all 775
workspace unit tests plus documentation tests PASS. Windows exe built; final
read-only probes ran serially after compilation. Native verification after the
authorized restart is recorded below; no research source was saved.


Native final verification completed after the user explicitly authorized
Discard Changes in the old test session. The sole 16:17 Windows build passed
assigned-net cold start, two moving previews, place/finish and Undo; connected
body drag/place/Undo; and an obstacle-constrained drag which retained a legal
position, followed by Undo. Final title is clean, ordinary selection is active,
Top is active, and All Objects / All Layers selection and snapping are restored.
The new GUI remains open; no research source was saved.

At 1920x1032, zoom 98.023726, Top-only view and phase profiling enabled, assigned
route begin took 365.845 ms (obstacle environment 169.774 ms). Initial preview
solve/copy took 0.087/53.463 ms; following preview updates took 3.806 and 0.003 ms.
A mixed CPU batch across this first start reached 434.70 ms; a constrained
connected-drag/release batch reached 578.88 ms. These include preparation,
History/commit, artwork, panels and idle frames and exclude GPU presentation;
they do not establish motion/input latency bounds. The initial pause and
commit/redraw stalls remain limits.

Additional attempted free-space starts selected copper on other document-visible
layers despite Top-only display, including with Current Layer snapping selected.
The route-start target search separately inspects all document-visible copper
layers. Those attempts were cancelled to a clean title. This is an observed
layer-selection limitation: native No Net movement is not claimed from these
attempts; explicit read-only No Net probes remain the corresponding verification.

## Pointer presentation - November 9, 2026

The user reports delayed pointer movement in both document editors. Static
inspection found that both hid the native cursor and painted the crosshair as
scene geometry. Even a correct, unsnapped latest pointer position therefore waits
for UI work, tessellation, GPU execution and presentation. The existing one-frame
latency hint alone did not separate cursor movement from that path. This finding
identifies a coupling; it is not a measurement of the reported input-to-photon lag.

The schematic cursor and PCB Small90 default now request the native OS crosshair,
without a software duplicate. The OS can move it between application frames;
snapping, picking and tool previews still use normal document input. Existing
focus/canvas/modal gates and PCB panning cursor behavior are retained. Small45 and
Large90 keep their configured scene geometry and still depend on frame delivery.

Native presentation now requests AutoNoVsync with maximum frame latency one.
wgpu 25 chooses Immediate, then Mailbox, then FIFO according to support. This may
reduce frame wait for previews and software cursor styles, but can permit tearing;
driver/compositor behavior and FIFO fallback can still add latency. Repainting
remains input driven: no unconditional animation loop was introduced.

The native crosshair follows the OS cursor theme, including size, DPI and contrast.
The schematic's previous forced black cross/white halo is no longer guaranteed.
Exact Altium cursor appearance and visibility against light/dark content require
actual-window checks; no current visual or measured responsiveness parity is claimed.

Added/updated source regressions cover absence of a frame-rendered duplicate,
raw movement within a grid cell, tool mode, focus/modal/panel boundaries and
retained Small45/Large90 geometry. The user requested no rebuild: only formatting,
whitespace and static checks were run. These tests have not been compiled/run and
no new GUI was launched. Native Windows/Linux input-to-photon measurements,
themed cursor contrast and the remaining expensive editing frames are pending.
