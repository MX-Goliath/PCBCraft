# PCB Properties — October 21, 2026

## Observations

Reference: running Altium Designer Enterprise 24.4.1, ReferenceBBoard.PcbDoc from
ReferenceB. Native window 1707×1027; PCBCraft 1707×1019 (different window-frame
height). Right panel about 316 px. The source board was unchanged.
Selection Filter > Texts was temporarily enabled to inspect Text, then restored.
Schematic data already marked `*` was not saved.

Observed: Board General/Parameters, Component, Track, Pad, Via, Text and Polygon
Pour. Board: context row, Search, General/Parameters/Health Check, three-row
Selection Filter, Snap Options, Board Information, bottom selection row.
Component: Location, Properties, Footprint, Swapping Options. Track: Net
Information, Location, Properties, Paste/Solder Mask Expansion. Pad/Via: Net
Information, Properties/Definition, Template/Library, coordinates with lock,
Pad/Via Stack and Hole Information. Text: Location, String/Frame, Text/Layer/Mirror,
Text Height, Font Type, Justification, Font, Stroke Width, Border Mode. Polygon:
Actions, Net Information, Properties and Outline Vertices. Arc/Fill/Region and
mixed selection lack separate complete native comparison.

## Implementation

- A contextual panel replaced the generic Apply/Revert editor and implementation
  sections Polygons/Nets/Board. Shared field sizes, expandable groups, search,
  tabs, fixed selection status and scrolling; width stays 316 px when selecting
  objects and displaying long imported strings.
- Board: working supported-object filters, Grids, All Layers / Current Layer /
  Off modes, distance and separate implemented-snap switches. Shift+E cycles
  modes; snapping is calculated in eda-core without screen coordinates.
  Supports track/arc endpoints, arc/pad/via centers, region/polygon/rotated-fill
  vertices, board outline and component origins. View settings are separate
  from model and history.
- Components: position, rotation, lock, designator/comment and string visibility;
  Top/Bottom switching uses the existing Flip command. Imported description,
  source ID, library and footprint are read-only. Height is read from
  OVERALLHEIGHT in available body projections.
- Track/Arc/Pad/Via/Text/Fill/Region: available dimensions, coordinates, layer/net,
  text, rotation, lock and geometry. Polygon: the application's pour-engine
  parameters, repour/shelve/restore and straight vertices. Changes use validated
  commands and History; importer/renderer do not mutate the document from the panel.
- Coordinate fields are relative to the source board's ORIGINX/ORIGINY; the model
  remains absolute integer nm. mm and mils are supported. Invalid input does not
  change the model; stale drafts do not overwrite Undo/Redo. Enter/blur commits
  a field; Escape cancels a numeric/single-line field.
- Comment import fixed: a literal Texts6 comment is no longer replaced by empty
  SOURCECOMMENT. The component and its owned string are synchronized; original
  metadata, hidden state and source archive are preserved.
- An unfinished changed field counts toward dirty state and close warnings.
  Save finishes Properties before writing. Selection changes commit a draft
  once, including transitions to mixed selection.
- Board Parameters displays computed imported-model information;
  explicit PARAMETER.* values are read-only and searchable.

## Verification and boundaries

Checks: fmt --check, workspace all-targets Clippy with -D warnings,
169 workspace tests, Windows debug build, Linux GNU all-targets cargo check.
New regressions: every context with long fields in a 316 px dock; Enter/Escape
in mm/mils through real egui input events; atomic/stale drafts; dirty/Close/Save;
selection changes and Undo/Redo; literal component comments; snap switches,
layers/visibility, arc endpoints, rotated fills and board vertices.

The real Windows window was inspected for Board, Component/XP1, Track, Pad and Via;
via Diameter 0.7 → 0.8 mm, blur, Undo and Redo. The test edit was undone; no native
file was written. The user's original PCBCraft window was preserved.
Linux GUI and Linux artifacts were not run/built.

Full visual or functional parity is **not confirmed**.
Remaining: exact vertical spacing/fonts/icons, all states and DPI, templates and
padstack libraries, text frames, barcode/justification/borders and exact stroke
fonts, component area/opacity and string/primitive locks, Workspace/DbLib
parameters, contextual filter popup, full rule engine/classes/propagation delay/
Health Check, mixed-selection editing, curved polygon vertices and general DRC/
rule engine. Such features are disabled or shown as unavailable values.
Native Snap Options had Pad Edges enabled; support was added October 22, 2026
(below). Grids currently links grid visibility and snapping; native view/filter
persistence needs separate comparison. Selection Filter defaults to supported
types; native SELECTIONFILTER_* is not yet decoded.

Board Information uses the actual outline rather than doc.size with an off-board
drill table. Computed area 1099.093 sq.mm differs from observed 1099.162;
Components Area is not replaced with an estimate. Parameters describe the
available imported scene, not Altium's compiled project. Pour uses PCBCraft's
existing engine; copper changes may rebuild polygons and do not confirm native
pour parity. This early check excluded native interoperability; later mask
verification and remaining save limitations are described below.

## Padstack, 3D and masks — October 21, 2026 continuation

### Observation

A separate `pcb-padstack-reference.PcbDoc` copy was studied in AD 24.4.1.
The user's source board was not edited. Observed: Simple, Top-Middle-Bottom and
Full Stack, expanded Copper/Hole/Top Solder, Round/Square/Slot, Copper Offset,
Manual/Rule Expansion, From Hole Edge, Tented and linked sides.
Pad/Via masks are in the shared stack table. Native Full Stack on the current
two-layer board shows two copper rows; T/M/B shows three. Top and Bottom tenting
are independent; only Top Tented was saved on the research copy.

### Implementation

- `eda-core`: typed padstack with per-layer geometry, Simple/T/M/B/Full, four pad
  shapes and corner radius, local copper offset, Round/Square/Slot holes and hole
  rotation. Geometry is shared by picking, bounds, 2D, pour and preview.
  Flip swaps stack and mask sides.
- `eda-render`: original 3D geometry with a portable software depth buffer and
  egui texture: copper surfaces with real cutouts, hole walls, plated barrel,
  top/bottom mask and paste. Drag rotates, wheel zooms, double-click resets view.
  Rotation does not change model/history. Depth is checked per pixel; the image
  is cached until geometry/view changes, with higher-resolution edge antialiasing.
  Height comes from available dielectric fields, falling back to 1.6 mm;
  internal layers are currently spaced evenly through the height.
- Properties: shared expandable Copper/Hole/Paste/Solder table, mode switches,
  per-layer dimensions/shapes, slot/offset/plating, separate mask sides, linked
  expansions and independent tenting. Expanded rows retain the 316 px dock;
  numbers display three decimal places, preserving internal nm until edited.
- Masks: Manual overrides and Rule Expansion; enabled-rule priorities,
  All/IsPad/IsVia/IsTrack/IsArc/IsSMDPad, InNet/InComponent/HasFootprint,
  NOT/AND/OR and parentheses. Unknown conditions in a higher-priority rule
  explicitly error rather than selecting a lower All rule. Negative expansion,
  percentage, enabled, tenting, From Hole Edge with Slot shape are supported.
  Mask geometry is also used on visible Paste/Solder layers in the main PCB canvas.
- A button beside a resolved rule opens an existing mask-expansion rule editor:
  scope, enabled, priority, expansion, tenting. This bounded editor does not yet
  reproduce the full native PCB Rules and Constraints Editor window.
  Changes use validation and shared History.
- PCB6: read/write legacy Top/Mid/Bottom, 32 via diameters, pad extra block,
  slot/offset/individual corner percentages and modern Full Stack entries.
  The Board6 layer chain is read before assembling final layers. Fields, unknown
  tails, wildcard rows and rule prefixes are preserved. Tented flags were checked
  against native Save. JSON retains the entire new typed stack and independent masks.
- Mask and mode changes that leave actual copper unchanged do not trigger repour.
  Copper/hole geometry changes still invalidate pour.

### Verification and remaining boundaries

188 workspace tests (38 app / 70 core / 1 geom / 56 IO / 23 render), fmt --check,
strict all-targets Clippy, Windows debug build and Linux GNU all-targets check.
Regressions cover slot/offset/cutout, per-layer picking/bounds, atomic edits,
Flip, mask-rule priority/unknown scopes, native roundtrip with unknown tails,
Rules6 prefix/opaque fields, refusal of data loss, finite 3D vertices, blind-via
span, fast drag and expanded-panel width across 60 frames.

Windows: selected Pad 2 inspected in the real PCBCraft window, Simple/T/M/B/Full
tables, 3D rotation and Top Tented with preview update, Undo/Redo/Undo to clean.
QA-window changes were not written to the user's file.
`pcb-padstack-roundtrip.PcbDoc`, written by PCBCraft after changing Top Tented
and solder-mask rule expansion, opened in AD 24.4.1; Pad 2 visually confirmed
Top Tented=true and Bottom=false. This does not confirm all rules/objects or DRC.

Full visual parity is absent: native materials/lighting/camera, dividers/icons/
spacing and all DPI still need comparison. Templates/PadViaLibrary, thermal relief
and Pad Features are unimplemented. Native Save currently rejects modern custom
mask shapes, independent native side expansions/percentages and Pad From Hole
Edge; JSON retains them. Ordinary symmetric expansions/modes, independent solder
tenting and Via From Hole Edge are written. Advanced stack/mask edits in ASCII
are rejected. Full drill tolerance/IPC-4761 and exact physical placement of
internal/plane layers remain incomplete.

On a real board, geometry edits followed by repour can change native primitive
counts. Save refused this because UniqueIDPrimitiveInformation references were
not remapped; source and destination files were unchanged. This guard remains.
Successfully opening a mask-only edited copy does not confirm native saving of
arbitrary changed copper. Linux GUI was not run; no Linux artifact was built.

## Pad Edges snapping — October 22, 2026

Following the recorded AD 24.4.1 observation, `Pad Edges` was enabled in the
existing Snap Options table without changing order/layout. The switch affects
only the view; document and Undo/Redo stay unchanged.

In `eda-core`, the cursor is projected to the nearest point on a Pad's outer
copper boundary. Rectangle/Octagonal/Round/RoundedRectangle, corner radii,
rotation, Copper Offset and layer-specific Simple/T/M/B/Full Stack dimensions
are handled. Round with unequal sides is a capsule. Circles are calculated
analytically, independent of tessellation or screen zoom.

Current Layer uses visible copper on the current layer; All Layers considers
visible copper layers. Zero-copper and drill-only pads create no fictitious edges.
Capture radius is measured from the original cursor; outside it, ordinary grid
snapping remains. Pad Centers is independent; Ctrl+Shift temporarily bypasses
snapping. The nearest supported candidate wins.

Checks: finite sides/corners, arcs from inside/outside in 120 directions,
capsules, rounded and octagonal pads, per-layer dimensions/offsets, rotation,
hidden layers, zero copper, Snap Distance, Off and model immutability.
A separate regression feeds real pointer/modifier events into the PCB canvas
and checks controller coordinates for enabled/disabled/bypass states.

A new live Altium comparison is unavailable in the Linux environment. Priorities
of coincident snaps, custom pad shapes, inner hole edges and full native All Layers
semantics remain unverified. Pad Vertices, Track/Arc Lines and Intersections
remain unavailable; this stage implements Pad Edges only.

Final checks: 193 workspace tests (38 app / 74 core / 1 geom / 56 IO / 24 render),
fmt --check, Clippy all-targets -D warnings and Linux debug build.
In Linux GUI on editor-v2.PcbDoc at one mouse position, Pad Edges enabled gives
X=9.750/Y=9.669 mm; disabled gives X=10.000/Y=10.000 mm (1 mm grid);
reenabling restores edge snap. The document stays clean.
TI PMP23595 imported: 8452 objects, 236 components, 76 nets, 33 polygons.
PMP22650 remains blocked by `Invalid padstack land size, shape or offset`;
fixing this case is outside the implemented snapping scope.
TI originals and verification screenshots remain outside Git.
