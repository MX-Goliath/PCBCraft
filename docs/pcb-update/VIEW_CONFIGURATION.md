# PCB View Configuration and connections — 22.10.2026

## References and evidence

Target: Altium Designer Enterprise 24.4.1. The local reference is the recorded
live comparison in [VISUAL_PARITY.md](VISUAL_PARITY.md): Top/Bottom RGB values,
Object Visibility transparency, native `2DCONFIGURATION` and pad/via labels.
Native display settings in imported documents are also used as data references.

The official reference URL is
https://www.altium.com/documentation/altium-designer/pcb-view-configuration-panel .
Fetching it on 22.10.2026 failed at the environment proxy (CONNECT 403).
It was **not** read successfully. No new Altium screenshot/live session was
available in this environment. The tab/group arrangement beyond the recorded
observations is an implementation target, not a verified pixel comparison.
Full visual or functional parity is not claimed.

## Controls

- `L`, View > Layers and Colors, Design > Layers and Colors, the layer bar,
  or Panels > View Configuration opens the right dock. Close restores its
  previous Properties/Components/Messages context.
- **Layers & Colors**: Signal Layers, Internal Planes, Mechanical Layers,
  Other Layers; per-layer visibility, active layer, editable RGBA color,
  group All On/All Off. Signal layers follow physical stack order.
- **System Colors**: Connections visibility/color, visible grid, workspace,
  board area, grid and pad/via hole colors.
- **View Options**: Object Visibility transparency for Arcs, Pads, Vias,
  Tracks, Texts, Fills, Polygons, Regions and Component Bodies, applied to the
  active layer or all layers. Fully transparent items are omitted from picking.
  Pad numbers and nets can be displayed together; via nets, polygon copper,
  grid and connection lines can be switched independently.
- Single Layer Off/Dim/Hide and dim strength; existing Shift+S remains operative.
- In 3D, View Options exposes component body visibility and Top/Bottom/Fit.
  The other controls configure the 2D view, not the 3D renderer materials.

Settings are per open document and do not change model/history/dirty state.
They survive tab switches but are not saved to native/JSON documents or across
sessions. Native profile Load/Save is visibly disabled. 3D materials/opacity,
via span markers, draft object rendering modes, selection masking, net color
overrides, arbitrary docking/autohide and exact native profile parity remain
unfinished. Existing native visibility/opacity defaults are read on open.

## Connection calculation

`eda-core::pcb_connectivity` intersects actual copper geometry per net and layer
before connecting disconnected islands by a deterministic minimum spanning tree.
It uses track widths, arcs, padstacks, via spans, fill/region outlines and holes,
plated barrels, and the existing poured copper. Native fragments without their
own NET inherit it from the owning polygon. Shelved/stale pour results,
keepouts, body projections, duplicate legacy regions, cutouts and copper text
are excluded. Unassigned pads are not implicitly assigned a net.

All disconnected same-net copper islands participate, including unconnected
vias and track fragments. Anchor candidates
are pad/via centers, track endpoints and sampled region/arc outlines. This is
not Altium's proprietary routing optimization, topology rules or DRC. Arc/circle
intersections use the existing polygonal approximation. Negative plane handling,
net ties, unsupported import geometry and exact native endpoint choice are not
verified. A zero-length interlayer guide cannot be seen as a line.

A per-view background job builds connectivity without blocking input. At most
one job is in flight; edits coalesce, obsolete results are rejected, the previous
lines remain until the current result arrives. The panel says Updating
connections during calculation. The cache retains only the current line list,
one preview snapshot and one running snapshot; no history of computed scenes.
Visibility, color changes and camera motion do not trigger connectivity rebuilds.
Edits, undo/redo and changed move previews do. This asynchronous refresh can lag
movement on large boards; it is not instantaneous Altium dynamic optimization.

## Validation

Core regressions cover connected/broken tracks, different nets/layers, via span,
nonplated vs plated pads, holes in copper, stale/shelved/inherited-net pours and
minimal deterministic trees. Cache tests cover reuse and rejection of stale work.
`pcb_connections` is a read-only example for inspecting local boards.

TI PMP23595 (local only, no TI data added to Git): 8452 items, 76 nets,
0 missing island links after accounting for inherited polygon nets. The initial
calculation takes approximately 3 seconds in the debug build; it runs in the
background in the GUI. This count is not an independent Altium DRC comparison.

Completed checks: 255 workspace tests, `cargo fmt --all -- --check`, strict
workspace/all-target Clippy and debug app build. Linux GUI at 1440×900:
fixture shows two connection guides; Layers & Colors / View Options, connection
visibility and label switches; TI panel shows six physically ordered signal
layers and zero connection guides; 3D View Options is available. The source
boards remain unmodified. Screenshots/logs are local under `/tmp/view-*`.
