# PCB 3D Layout Mode — 2026-10-22

Press **3** in a PCB document to enter 3D Layout Mode, **2** to return to 2D.
Both modes also appear under View. Numeric input in an active text field is not
intercepted. Each document keeps separate 2D and 3D cameras. Navigation and mode
changes do not modify the document or Undo history. An unfinished PCB gesture,
route or properties edit must be completed/cancelled before entering 3D.

- Shift + right-button drag: orbit. Shift + middle drag also works.
- Right/middle drag: pan. Wheel, Ctrl+wheel and pinch: zoom at pointer.
- Fit resets pan/zoom; Top and Bottom set the viewing direction. Existing View
  fit commands work in 3D; Selected Objects also centres/fits Cross Probe targets.
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
through the substrate. Surface copper is clipped by the same through cuts so
tracks and poured regions cannot cover physical holes. Top/bottom lands, tracks, arcs, fills and regions are
projected onto the corresponding surface; the board has thickness and edge walls.

Native component-body projections are extruded between standoff and overall
height, on the appropriate side of the board. When a projection is missing,
a rotated pad envelope supplies an explicitly simplified placeholder, with native
component height when available (1 mm fallback). Body outlines, pads and component
origins remain in document coordinates. These are **simplified bodies**, not
manufacturer STEP meshes. Imported source archives remain untouched.

A software depth buffer resolves both visibility and object picking; objects
behind the board cannot be selected through it. Scene geometry is cached by
monotonic document-history revision, and raster/image-ID buffers by camera,
viewport and selection. Textures are updated in place. Idle frames do not redraw
or allocate textures; returning to 2D retains one bounded scene cache per document.
Raster resolution is bounded to 1200×1000 pixels. Geometry helpers are in eda-core;
the renderer does not edit document data.

## Validation and remaining differences

Tests cover source immutability, top/bottom heights, fallback identity, overlapping
through-drills vs blind vias, depth picking, selection shading, reusable scene and
texture caches, fast orbit/pan release with press modifiers, mode shortcuts,
selection/rotation/Undo, preservation of the 2D camera and refusal of pending edits.

Workspace validation: 228 tests passed; formatting and strict Clippy passed.
The release AppImage was exercised in a real Linux window on the local TI
PMP23595 reference board: 3D entry, physical-outline Fit, component selection
and Properties, orbit, zoom and bottom view. TI source files are not included
in the repository or AppImage.

This is not full Altium 24.4.1 visual parity. STEP/Parasolid tessellation, realistic
model materials/textures, planar silkscreen text, accurate solder-mask apertures,
3D collision checks, rigid-flex bending and native 3D-body placement editing are
not implemented. Projection-only bodies cannot reproduce shape details such as
connector cavities or leads. Existing native-save restrictions for moving imported
3D bodies still apply; navigation alone does not affect native saving.
