# PCB: Altium 24.4.1 comparison — October 21, 2026

## ReferenceA — October 23, 2026

The current debugging reference is the user's `ReferenceA.PrjPcb` and
`ReferenceA.PcbDoc`, open in installed AD Enterprise 24.4.1.
Sources are not overwritten; research JSON is in ignored `target`.
The board has 56 components, 29 nets, 982 imported objects and three polygon
definitions. The object count includes archived paired regions.

Observations and fixes:

- Import previously stopped at `V7_LAYER=CONNECT`. This is system layer 75, used
  by saved shelved-polygon regions. System layer names 75–82 were added without
  treating them as copper. A binary regression checks both region families and
  byte-preserving unchanged saves. IDs were compared with [Altium TLayer](https://www.altium.com/documentation/altium-dxp-developer/pcb-api-types-reference#tlayer).
- The source has `CFG2D.SINGLELAYERMODESTATE=3`; live Altium View Options selects
  Single Layer Mode **Off** and shows both copper layers. The former `.min(2)`
  incorrectly hid inactive layers. Value 3 now gives normal view.
  Complete semantics of other native single-layer modes remain unstudied.
- The file has `ELECTRICALGRIDRANGE=1.9685mil`,
  `ELECTRICALGRIDUSEALLLAYERS=TRUE`; Altium displays 0.05 mm / All Layers.
  PCBCraft previously retained 0.25 mm / Current Layer. Range, mode, arc-center/
  board-outline flags are imported; subsequent user changes are not overwritten each frame.
- A nearly point-sized board (and schematic) appeared at process startup;
  manual Fit Board restored it. Fitting an empty area is now deferred until
  panel layout produces a nonzero viewport.

Full parity is **not achieved**: comparison shows differences in Active Bar,
palette/compositing, adaptive labels, body hatching and connection lines.
Initial dock layout differs from the user's Altium layout; pixel comparison
requires matching workspace sizes and DPI. Router, DRC, pour or native Save
equivalence after arbitrary board editing is not claimed.

Verified: `cargo fmt --all -- --check`, strict workspace/all-targets Clippy,
286 workspace tests (64 app, 102 core, 1 geom, 59 IO, 60 render), Windows dev
build. After a new launch, the real window shows the board without manual fitting
and All Layers / 0.05 mm. Fit Board, via selection, moving and Undo back to clean
were checked. Moving invokes the application's pour, which visually differs from
native; Undo restores original results. This remains an open compatibility
defect, not confirmation of the pour algorithm.

Source SHA-256 before/after research matches:

- PrjPcb: `redacted-source-sha256`
- PcbDoc: `redacted-source-sha256`

The reference is the user's open `ReferenceBBoard.PcbDoc` from
`ReferenceB.PrjPcb`. 2D mode, Top Layer active. Neither source file nor board
in Altium was edited. Full visual parity is **not yet achieved**.

## Observations

- View Configuration confirmed Top Layer RGB 153/0/0 (`#990000`) and Bottom
  Layer RGB 11/83/148 (`#0B5394`). Colors belong to the Altium profile rather than
  PcbDoc geometry; these observed values are used as the palette.
- Object Visibility has 20% transparency for Pads, Polygons and Regions on
  signal layers; Tracks, Arcs and Texts have 0%. These settings exist in the
  document's `2DCONFIGURATION`. Pad numbers/nets and via nets/span are enabled.
- Components6 has NAMEON/COMMENTON=false for all 69 components. Their string
  records still exist in Texts6. Manually placed labels are not hidden by these flags.
- `Carbon on Top L` is on modern Mechanical 18. The one-byte fallback layer is
  not its ID. The drill table is red; top and bottom overlays are both visible over copper.
- Board outlines contain arcs whose starting vertex is at the arc's end angle,
  plus seams a few nanometers long. Counterclockwise-only traversal produced
  diagonals and path reversal at rounded corners.

## Implementation

| Area | Change | Verification boundary |
| --- | --- | --- |
| Layers | Modern Tracks/Arcs/Fills/Texts IDs, V7_LAYER for Regions and bodies; names from V9 cache; Mechanical 1–1024 | Real two-layer board; native Save transfer of arbitrary modern layers is limited |
| Display | Visibility, per-type transparency, snap grid from native 2D configuration | Original file settings; not the entire View Configuration profile system |
| Text | NAMEON/COMMENTON, separate font kind/bold/italic/inverted, mirroring, rotation, dimensions and saved metrics; fixed layout scale | No exact Altium Default/Sans Serif glyphs; embedded fonts/LettersGeometry and special styles are unimplemented |
| Outlines | Arc traversal direction from its vertex; safe seam drawing without infinite miters | Shared PCB/polygon geometry and synthetic GPU-mesh regression |
| Pad | Per-signal-layer corner radius; shared renderer/pour/picking calculation | Full padstack shapes, slots/offsets and separate corner masks remain limited |
| Cutouts and bodies | Cutout hatching; saved 2D projections from both ComponentBodies families | PcbDoc projection, not a STEP renderer or 3D-model editor |
| Duplicates | Legacy and ShapeBased remain in model/archive; alternate representation hidden only when record index, properties, owner and geometry match within ARCRESOLUTION | Checked 57 Region pairs and 62 Body pairs on the real board |
| Holes | Separate drilling pass over copper; active-layer and Multi-Layer pad labels | Via-span indicators and exact adaptive size/placement of all labels still differ |

Import retains **15513** records (formerly 15389: 124 projection records added,
representing 62 bodies in two formats), 69 components, 20 nets and 3 polygons.
Paired records neither double on-screen transparency nor disappear from the
source format. Native writer retains unchanged body streams; changing a projection
blocks native Save until full 3D transforms exist.

Percentage radius is calculated from half the pad's shorter side;
100% gives full rounding, matching the [official Pads & Vias documentation](https://www.altium.com/documentation/altium-designer/pcb/pads-vias).
Font kind/bold/italic/barcode field offsets were checked against the independent
[AltiumSharp PcbLibReader](https://github.com/issus/AltiumSharp/blob/master/src/OriginalCircuit.Altium/Serialization/Readers/PcbLibReader.cs).
No third-party parser/renderer, fonts or Altium engine are included in the application.

## Verification and remaining work

- 159 workspace tests; fmt --check; Clippy --all-targets -D warnings.
- Binary import/native-save regressions for modern-layer fallback, opaque tails
  and body streams; refusal to save an outdated 3D model.
- Glyph mesh geometry stays stable during zoom/rotation/mirroring at DPI 100/150/200%.
- Hidden strings/fit, corner radius/picking, hatching with holes, arc direction
  and absence of unbounded triangles at native seams checked.
- Windows debug build and real board opening; Linux GNU all-targets check.
  Linux GUI and artifacts were not run in this iteration.

Next required steps for full parity: exact PCB stroke fonts, native text styles/
embedded glyph geometry, via-span markers and adaptive labels, stroke-width and
hatching comparison at equal zoom, DPI and workspace size. PCB Properties layout,
floating toolbar and layer bar also still differ.
No parity percentage or pixel-perfect acceptance is claimed.
