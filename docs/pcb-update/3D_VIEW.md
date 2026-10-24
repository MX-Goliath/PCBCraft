# PCB 3D Layout Mode — updated 2026-10-24

Press **3** in a PCB document to enter 3D Layout Mode, **2** to return to 2D.
Both modes also appear under View. Numeric input in an active text field is not
intercepted. Each document keeps separate 2D and 3D cameras. Navigation and mode
changes do not modify the document or Undo history. An unfinished PCB gesture,
route or properties edit must be completed/cancelled before entering 3D.

- Shift + right-button drag: orbit. Shift + middle drag also works.
- Right/middle drag: pan. Wheel, Ctrl+wheel and pinch: zoom at pointer.
- Fit resets pan/zoom; Top and Bottom set the viewing direction. Existing View
  fit commands work in 3D; Selected Objects also centres/fits Cross Probe targets.
- Ctrl+F flips the 3D camera around the board, preserving the 2D camera.
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
selection/rotation/Undo, preservation of the 2D camera and refusal of pending edits.

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
