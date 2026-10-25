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
