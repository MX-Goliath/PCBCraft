# PCB 3D Layout Mode — updated 2026-10-25

Press **3** in a PCB document to enter 3D Layout Mode, **2** to return to 2D.
Both modes also appear under View. Numeric input in an active text field is not
intercepted. Switching modes shares the board point at the viewport centre and
the scale in logical pixels per millimetre. The chosen 3D viewing direction is
retained. Navigation and mode changes do not modify the document or Undo history. An unfinished PCB gesture,
route or properties edit must be completed/cancelled before entering 3D.

- Shift + right-button drag: orbit. Shift + middle drag also works.
- Right/middle drag: pan. Wheel, Ctrl+wheel and pinch: zoom at pointer.
- Fit resets pan/zoom; Top and Bottom set the viewing direction. Existing View
  fit commands work in 3D; Selected Objects also centres/fits Cross Probe targets.
- Ctrl+F flips the 3D camera around the board. Returning to 2D uses the resulting
  visible board area and scale.
  View > 3D View Control provides the observed native sequence of side views,
  rotate/pan commands, Isometric View (0), Flipped Isometric View (Ctrl+0), and
  90 Degree Rotation (9). Fold/Unfold remains disabled. Keypad-specific mappings
  are not claimed: egui does not expose a distinct key for every numpad digit.
- Click a visible component/body/pad to select the component; Shift-click toggles
  selection. Double-click opens Properties. Hover shows reference/comment/footprint.
- Component Properties remain editable; changes rebuild the 3D geometry through
  the normal validated document/history path. Space/Shift+Space rotate selection;
  a single 3D component rotates around its component origin. Undo/Redo work.
- Ctrl+click and Tools > Cross Probe use depth-picked object identities, including
  pads and tracks. Components can be hidden with the Components checkbox.
- Placement, routing and clipboard-reference tools return to 2D. A left drag in
  the 3D viewport does not move PCB objects; move through Properties or in 2D.

## Shared 2D/3D camera, 2026-10-25

Panning and zooming in either mode carry over to the other. In an oblique 3D
view, the anchor is the centre ray's intersection with the board plane at Z=0.
The scale is measured along the board axis without foreshortening at that point;
perspective and tilt still change the apparent shape and extent. At an edge-on
angle, or when the centre ray misses the board plane, the stored board anchor
is retained instead of exporting an unstable horizon point.

The incoming view is applied after asynchronous scene preparation, using the
actual 3D viewport size. Returning before preparation finishes keeps the exact
incoming 2D camera. Dock/toolbar resizing preserves scale. Repeated switches
without navigation preserve the exact 2D camera and 3D pose, avoiding cumulative
floating-point drift. Explicit Fit and directional preset commands still fit
the board. A deferred Fit is cleared when leaving the mode that requested it.

Regression coverage includes high zoom, top/bottom and oblique views in both
projection modes, navigation in either mode, 100 repeated transitions, viewport
and scene-radius changes, edge-on fallback and immediate return during scene
preparation. Controller tests exercise asynchronous rendering and document/Undo
immutability. Exact Altium camera/projection parity remains unverified.

Linux AppImage validation used a separate U10 fixture copied from ReferenceM:
2D navigation, 3D orbit/pan/zoom and five repeated mode switches. Captured 2D
round trips and oblique 3D round trips were pixel-identical. Workspace validation
passed 429 tests, formatting and strict all-target Clippy on Rust 1.90.0. The
rebuilt AppImage passed its SHA256 check; the source PcbDoc hash was unchanged.

## Geometry and rendering

The board uses its routed outline and imported stack dielectric thickness (1.6 mm
fallback when unavailable). Through drills and board cutouts are unioned before
subtraction, preserving overlapping holes/edge slots. Blind vias are not drilled
through the substrate. A native copper polygon cutout (`KIND=1`) is **not** a
physical hole: only explicit `ISBOARDCUTOUT=TRUE` regions remove substrate.
Surface copper is clipped to the board and by the same through cuts so
tracks and poured regions cannot cover physical holes. Top/bottom lands, tracks, arcs, fills and regions are
projected onto the corresponding surface; the board has thickness and edge walls.

Embedded `/Models` zlib STEP archives are read by the independent Rust Truck
kernel in eda-io. B-rep faces are tessellated at 0.02 mm tolerance, retaining AP214
face/shell colours. SI length units are converted to millimetres; unsupported
assembly transforms and conversion-based/mixed units produce explicit warnings
and use the simplified projection fallback. STEP geometry is stored once per
model in shared Arc data in eda-core; no GUI/GPU dependency is added to the model.
Old JSON files recover missing meshes from their retained original archive.

STEP processing also occurs on ordinary 2D PCB import. Non-finite converted
surface coordinates are checked before trimming; boundary-curve subdivision is
bounded by depth and per-edge/model evaluation budgets. Rejected models emit
warnings and use the simplified projection fallback. The original embedded STEP
bytes remain in the source archive. The ReferenceMHardware opening-hang reproduction,
Linux measurements and remaining limits are in [PERFORMANCE.md](PERFORMANCE.md).

Native MODEL.3D Euler rotations precede the bottom-side Y/Z reflection and
MODEL.2D board translation. Component rotation must not be applied twice.
Validated move/rotate/flip and Undo update placements while sharing mesh data.
Bodies with unavailable STEP geometry use their explicitly simplified native
projection; native components without bodies do not acquire fictitious boxes.
New, non-native components can still use a pad-envelope placeholder.

Solder mask is a film above copper, with unioned pad/via and explicit mask
apertures removed. Top/bottom tenting and paste rules are respected. Copper,
mask, paste and silkscreen occupy distinct depths; copper polygons cannot cover
drills. Silkscreen and copper text use planar Liberation Sans outlines, including
glyph counters, mounting-side mirroring and rotation. This is an open font
approximation of native PCB lettering, not Altium's proprietary stroke font.

A software depth buffer resolves both visibility and object picking; objects
behind the board cannot be selected through it. Scene geometry is cached by
monotonic document-history revision, and raster/image-ID buffers by camera,
viewport and selection. Textures are updated in place. Idle frames do not redraw
or allocate textures; returning to 2D retains one bounded scene cache per document.
Raster resolution follows DPI and is bounded to 1800×1400 pixels. Shared triangle
edges use half-open coverage to avoid double blending. Perspective depth is
interpolated reciprocally; lighting uses camera-space normals before perspective.
Geometry helpers are in eda-core;
the renderer does not edit document data.

## Validation and remaining differences

Tests cover source immutability, top/bottom heights, fallback identity, overlapping
through-drills vs blind vias, depth picking, selection shading, reusable scene and
texture caches, fast orbit/pan release with press modifiers, mode shortcuts,
selection/rotation/Undo, shared camera transfers and refusal of pending edits.

Initial 2026-10-22 validation: 228 tests passed; formatting and strict Clippy passed.
The release AppImage was exercised in a real Linux window on the local TI
PMP23595 reference board: 3D entry, physical-outline Fit, component selection
and Properties, orbit, zoom and bottom view. TI source files are not included
in the repository or AppImage.

### ReferenceB adaptive reference investigation — 2026-10-24

Observed live in AD Enterprise 24.4.1: the user's `ReferenceBBoard.PcbDoc`, top,
bottom, body visibility, materials and the native 3D View Control menu/isometric
view. Native stored configuration initially hides bodies and paste. Research
used camera/material display controls only and never saved the original PCB.

Reproduced defects: exposed gold instead of masked pours; copper polygon voids
drilling through the laminate; duplicate legacy regions; unrendered STEP data;
missing text; bottom camera reversing Y instead of X; asymmetric bottom STEP
connector displaced by 15.65 mm because its mounting reflection was absent.
The corrected bottom connector occupies the same side, origin and orientation
as the native projection, with individual leads and connector cavities visible.

Read-only `pcb_3d_audit` on the source: 16 unique meshes / 10,293 triangles,
62 placements, zero missing models, maximum XY bounds difference from saved
native projections 0.000122 mm. Loading plus tessellation measured about 0.7 s
on this Windows host. Bounds agreement verifies placement/envelope, not every
surface point or pixel. The fixture STEP in this repository is independently
constructed; the user's models and project are not committed.
The final read-only CPU frame probe (1000×800 logical viewport, native default
body visibility) prepared the 3D scene/image in about 594 ms and refreshed an
orbit frame in 274 ms; UI frames while waiting stayed below 0.8 ms. These numbers
exclude GPU upload and the surrounding app panels. Curved artwork is one bounded
annular band with round end caps, rather than many overlapping segment meshes.

The source SHA-256 before investigation is
`redacted-source-sha256`.
It was unchanged after the live comparison. Workspace validation: 361 tests,
strict all-target Clippy and formatting passed; Windows dev build and Linux GNU
all-target compilation passed. Live Windows top/bottom/flipped-isometric views,
STEP depth picking and locked-object rejection were exercised after changes.
An unlocked bottom-side component was rotated with Space in the real 3D window
and restored with Undo. Automatic repour reported the already unsupported native
`InPad(...)` query; no pour or source file was saved. Undo restored the clean
document state. This independent repour limitation is not a 3D geometry fix.

Tests cover true holes versus polygon voids, mask union/tenting, STEP face
colours/units/failure fallback/sharing, top/bottom placement and editing history,
glyph counters/mirroring, bottom camera and translucent triangle seams.

Full pixel parity remains **unverified**: native lighting/specular reflection,
antialiasing, font metrics, exact isometric camera/Fit parameters and viewport
toolbar/dock layout differ. The Windows captures have the same desktop/DPI but
different dock widths and native/client chrome bounds; they are a live visual
comparison, not an equal-viewport image-difference acceptance test. Parasolid,
model textures, assembly transforms, section views, collisions and rigid-flex
are unsupported. Linux GUI rendering is not exercised in this Windows session.
The software renderer remains asynchronous and is not a 60 FPS parity claim.
Native save restrictions for edited imported 3D placements still apply;
navigation alone does not affect native saving.


### 3D entry loading feedback and GPU preparation - November 9, 2026

The user reports a blank white viewport before a slowly loading board. Source
inspection found a white-text-on-native-background loading message (invisible
when the stored workspace background is white), and synchronous shader pipeline,
vertex-buffer and render-target construction in the first GPU paint callback.
Scene replacement also reconstructed all pipelines instead of retaining them.
These are source findings; the reported complete delay has not been profiled.

The viewport now paints a themed loading card with spinner before starting the
board snapshot/geometry job. The announcement is held through every layout pass
of its first frame. Geometry preparation reports its actual stage and elapsed
seconds. CPU failure, GPU preparation and software raster fallback all use a
contrasting card, independent of the configured board background.

Registering the native device also enables one background GPU preparation job
per 3D renderer. Pipeline compilation can overlap CPU geometry generation. A
completed stale preparation is not displayed against a different scene/format/
size; its compatible pipelines can be reused for the newest request. Geometry
replacement updates vertex/material/index buffers and bindings while retaining
compiled pipelines and compatible render targets. Unsupported integrations keep
the existing callback/software paths. Failed preparation is tracked per key so
an unchanged failure does not continuously restart shader compilation.

A component-index map replaces repeated component-table searches while building
artwork/model ownership. Board geometry, mask/drill generation, material rules
and transparency/depth rendering remain enabled. Initial boolean geometry work,
board snapshot copying, GPU driver serialization and the first actual draw may
still be expensive. No universal cold-load or frame-time guarantee is claimed.

Added regression sources cover white-background first-frame feedback, bounded
GPU preparation requests, pipeline reuse and rendering changed geometry/materials.
The frame probe now waits for GPU resource preparation as well as CPU geometry
before calling a frame ready. Formatting/static checks only: the user explicitly
deferred rebuilding, so these tests and updated native-window/GPU latency checks
remain uncompiled/unrun. No original design was edited or saved.
