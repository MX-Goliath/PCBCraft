# Basic schematic editing

October 21, 2026 iteration, partial implementation of §3 and §9–10 of the original
`ALTIUM_RUST_AGENT_PLAN.md`. Works with new schematics and imported SchDoc scenes.
This is a geometry and properties editor; a connectivity compiler and ERC remain
separate stages.

Original example to open: `fixtures/schematic/editing-demo.pcbcraft.json`.

## Controls

Create a schematic: Ctrl+Shift+N. Open SchDoc/PrjPcb: Ctrl+O.
Tools are available in Place and Active Bar. Part… currently has two original
symbols: Resistor and Capacitor. Other components can be copied from imported
scenes or placed from local SchLib/IntLib through Components.

| Action | Controls |
|---|---|
| Select | Click; Shift adds objects. The first Wire click selects the whole wire; the next selects a segment between bends/electrical nodes |
| Place | Select a tool, click the sheet; repeats until Esc |
| Wire | Ctrl+W or W; clicks confirm vertices; hitting an existing wire/electrical contact completes the connection and allows starting another; Enter/right-click finishes at the last confirmed vertex |
| Backtrack while wiring | Backspace/Delete removes the last confirmed vertex without deleting selected objects |
| Wire angle | Space switches horizontal/vertical segment order |
| Label / text / junction | L / T / J |
| GND / graphic line | Place → Power Port / Drawing Tools → Line |
| Component Drag | Drag or M, D: connected wires stretch; mixed groups of components, wires and selected segments are supported |
| Move | M, M: move while breaking connections |
| Wire shape | Drag a segment/vertex; or M, D, select a wire point and click the new position. A segment moves perpendicular to its axis; wire ends stay fixed; T-branches follow the segment |
| Rotate | Space: selected object or preview rotates 90° counterclockwise |
| Copy / cut / paste | Ctrl+C / Ctrl+X / Ctrl+V; paste requires a click |
| Duplicate | Ctrl+D, then click |
| Delete | Delete; a component is deleted with all its visible child primitives |
| Undo / redo | Ctrl+Z / Ctrl+Y |
| Cancel current tool | Esc; unfinished wires/previews are not committed |
| Properties | Designator, Comment, X/Y; object text; wire/line width. Apply on Enter or field blur |
| Save | Ctrl+S / Ctrl+Shift+S to `.pcbcraft.json`; close tab with Ctrl+F4 |

Properties has been extended with Document, Component General/Pins, Parameter,
Designator, Wire, Net Label, Power Port, Text/Text Frame and graphics contexts.
Sheet settings, colors, individual fonts, parameter visibility/name display,
parameter addition/removal and a wire/line vertex table are now available.
The model table reads instance metadata. Local SchLib/IntLib are supported through
Components; details: interface/components-libraries.md. Selecting multiple objects
with Shift displays disabled Mixed. Mixed groups support Move/Drag, rotation,
Copy/Cut/Paste, Duplicate and Delete; bulk Properties field editing remains disabled.
Full Properties parity remains open: docs/interface/schematic-properties.md and
features.json.

Numeric Properties fields use the selected mm/mil units. Placement snapping uses
the document snap grid, electrical pin endpoints, GND, junctions and axis-aligned
wire segments. The visible grid is independent view state.
New R/C components receive the first unused designator; component copies receive
new UUIDs and an unused designator.
The clipboard stores PCBCraft's own text format with geometry, fonts and embedded
images; it is not an Altium exchange format. Ctrl+V accepts payloads up to 16 MiB.
The menu Paste command uses the last internal fragment of the current session.

## Data contract

Typed `Edit` and `Request` live in `eda-core::schematic_edit`.
A request checks the document UUID and expected monotonic history revision.
The command is first applied to a model copy, checked by `Document::validate` and
recorded as one transaction. Failure of any Batch item leaves model and history
unchanged. The renderer only reads the model and returns events in document
coordinates; the GUI owns the temporary preview and tool selection.

Imported child coordinates are already absolute. Transform changes them once,
together with the component origin; UUIDs are preserved. Copying replaces UUIDs
and owner references and merges font tables.
New objects have `source_index = u32::MAX`; `text_role` preserves the
Designator/Comment/NetLabel role independently of original records.

Original records and the SchDoc archive remain unchanged. `source_modified`
marks that the editable scene has diverged from the archive; Undo restores the
flag along with the scene. JSON v1 gained optional `source_modified` and
`text_role` fields with defaults; old JSON remains readable. The archive is not
a saved native-format result of editing.

Scene coordinates remain i64 nm under ADR 0002. This iteration does not satisfy
the full plan's native 10 pm roundtrip requirement. History still stores snapshots
in memory; a durable journal, crash recovery, operation ID/idempotence and a remote
command API are not yet implemented.

## Connections during editing

`Edit::DragWire` and `wire_drag` calculate wire shape in eda-core.
Handle selection uses document coordinates with a radius supplied by the renderer.
Preview and commit preserve UUIDs, styling and endings. A vertex changes position
locally; other vertices stay fixed, with orthogonal bends added if necessary.
Segment edits support horizontal/vertical segments.
Separate Drag and Move semantics are preserved: Move still breaks connections.

Automatic T-junction and pin-connection points are computed from geometry,
including preview. They do not become manual Junctions or clutter Undo.
A simple crossing of two wire interiors is not a connection.
Manual/imported Junctions are retained. The point cache belongs to View and is
released on document close; changes, Undo/Redo and preview update it.
This is local topology, not a project-level named/hierarchical net compiler.

October 21, 2026 observation, Altium 24.4.1: on a separate ReferenceB copy,
Edit → Move → Drag moves a wire middle, stretches branches and leaves components
stationary. The change was undone; the original project was not edited.
Additional source: [Creating Circuit Connectivity](https://www.altium.com/documentation/altium-designer/schematic/creating-circuit-connectivity).

Windows GUI: component M,D with wires, segment M,D with a T-branch, Undo/Redo and
Save/reopen were checked on the application's own schematic. Automated tests cover
vertex/segment, normal and coalesced mouse gestures, cancellation outside the
canvas/on focus loss, tab switching, stale revision, Backspace and automatic wire completion.

## Groups, individual segments and overlaps — October 21, 2026

Observation: on a separate ReferenceB copy in Altium 24.4.1, the first wire click
shows green markers; a repeat click selects a segment with red markers.
The user's original project was not changed. Segment deletion and Optimize Wires
& Buses were also compared with official descriptions:
[Placement and Editing Techniques](https://www.altium.com/documentation/altium-designer/schematic/placement-editing-techniques),
[Schematic Preferences](https://www.altium.com/documentation/altium-designer/preferences/schematic).
Native Delete results and every preference combination were not observed in this iteration.

Implementation:

- `Selection { ids, segments }`, `DeleteSelection` and `TransformSelection` live in
  eda-core. Mixed groups change in one validated history transaction.
  A component and its own jointly selected label are transformed once.
- Shift adds objects; a repeated Shift+click on a selected Wire switches to segment
  selection. Further Shift+clicks add/remove segments. Boundaries are bends,
  neighboring wire endpoints, pins, labels and junctions; an ordinary interior
  crossing without a node does not split the wire.
- Delete/Cut retain other polyline parts and their styling. Disappearing automatic
  T-points are recomputed. Selected-segment geometry and nodes are rechecked;
  stale selection is rejected atomically.
- Move transfers selected geometry; Drag stretches external connections.
  Groups of whole wires and selected segments together with components support
  common rotation and Copy/Cut/Paste/Duplicate. Pasted groups receive new UUIDs
  and distinct unused designators, e.g. R1/R2 → R3/R4.
- Overlapping collinear portions of compatible wires are absorbed after editing,
  insertion or wiring; reversed direction and self-overlap are handled.
  Electrical T-nodes survive even when an original Wire endpoint is absorbed.
  Adjacent portions may remain separate objects at node boundaries.
  Unchanged geometry outside the overlap area is not normalized.
- Move/Drag, paste and wiring previews use the same command, validation and
  optimization result as commit. Absorbed/split originals are hidden.
  A quick repeat click on the schematic no longer triggers Fit;
  View → Fit Document remains available.

Verification: 145 workspace tests, fmt --check and Clippy -D warnings; includes
real egui pointer events for Shift/repeat clicks and two-segment deletion, mixed
groups, distinct designators, no-op, stale revision/topology, Undo/Redo, exact
geometry coverage and 864 direction/overlap combinations.
Windows GUI on the application's own `.tools/group-wire-qa.pcbcraft.json`:
green/red markers, mouse segment Drag, Duplicate with overlap and T-branch
preservation, Undo/Redo, JSON Save/reopen. Groups were additionally checked through
automated canvas events; the native Shift modifier was not used in this GUI probe.
Linux GNU all-targets cargo check passes; Linux GUI and new Linux artifacts were
not run/built in this iteration. Full Altium parity is not claimed.

## Remaining limitations

- No rectangle selection or bulk Properties field editing.
- Optimization merges only style-compatible wires involved in overlap of geometry
  affected by the edit. Differently styled, locked/hidden wires and wires with
  styled ends are not absorbed. No whole-sheet cleanup command exists.
- No diagonal segment drag, all Altium angle modes, complete locked-selection
  semantics or four-way node transformations. New endpoint coincidence is handled
  by local geometry; a project compiler, net-change warnings and full connectivity
  checking are still absent.
- Placement of built-in R/C and local SchLib/IntLib symbols through Components,
  including dragging from the list/preview. Power Port is GND.
  No library editor, full annotation, hierarchy, ERC, ECO or native writer.
- Rotation of Port, Image, TextFrame and unknown primitives is rejected;
  moving is supported. Mirroring and pin editing are not yet available.
- Place/Drawing Tools order was compared with open Altium 24.4.1; Part… opens
  Components, and Junction was added as a separate entry.
  Icons/Active Bar, Edit and Properties do not yet fully match the reference in
  appearance and behavior.
- Windows GUI is checked locally; Linux uses the same core/GUI without Win32 and
  is checked locally with cargo check x86_64-unknown-linux-gnu. The new Linux GUI
  iteration was not verified; Linux/AppImage artifacts were not built at the user's request.
