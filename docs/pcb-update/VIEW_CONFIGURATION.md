# PCB View Configuration and connections — 22.10.2026

## Live reference and scope

Altium Designer Enterprise 24.4.1 was observed live on Windows on 22.10.2026,
using the existing `pcb-padstack-reference.PcbDoc` research copy. No source
project was edited or saved. Both applications were maximized on the same
monitor: native captures were 1707×1027 and PCBCraft captures 1707×1019 because
of their different window frames. The panel width was 330 px in both.
This is a live layout comparison, not a pixel-difference acceptance test.

Observed states: Layers & Colors with all four groups, paired mechanical roles,
System Colors, 2D and 3D View Options, Additional Options, the expanded Object
Visibility dialog, and Section View. `Used On` was clicked: it directly enables
used layers rather than opening a menu. The advanced dialog was cancelled.
The official documentation URL remains
https://www.altium.com/documentation/altium-designer/pcb-view-configuration-panel ;
it could not be fetched in this session and is not treated as read evidence.

## Implemented layout and behavior

- View Configuration occupies the **left** dock; the right Properties panel
  remains independent. L, the menus, layer bar and Panels entry points remain.
  Search, 22 px tabs, 20 px layer/object rows, title controls and dock footer
  follow the observed layout. Close/Projects restores the project tree.
- Layers & Colors groups are Signal And Plane Layers (S), Component Layer Pairs
  (C), Mechanical Layers (M) and Other Layers (O). The catalog combines model
  layers, enabled V9 cached mechanical layers and MECHPAIR records. Empty
  configured layers remain available. Typed pairs are ordered 3D Body, Assembly,
  Component Center, Component Outline, Courtyard, Designator, then untyped pairs,
  Overlay, Paste and Solder. Mechanical numbers are sorted numerically.
- Layer/group/all visibility, per-side colors, Used On and active-layer selection
  work. View From Bottom / Ctrl+F mirrors the 2D camera, including picking, zoom,
  grid and panning. Layer-stack edits refresh the catalog by document revision.
- View Options contains General Settings, Object Visibility, Mask and Dim
  Settings and Additional Options. Visibility and opacity are independent:
  hiding a kind preserves its opacity. Mixed opacity is aggregated across
  configured layers. Draft rendering outlines supported copper primitives.
- Object Visibility is a centered modal matrix with the five native layer
  groups, used-layer filtering, Ctrl cell selection, a percentage slider/value
  and Apply/Cancel. Changes are staged until Apply. Fully transparent objects
  are excluded from picking. A single-frame press/move/release is retained.
- Supported system colors affect connections, selection/highlight, pad/via holes,
  relative origin, board line/area, and 2D/3D workspace gradients. Native palette
  defaults observed in the reference replace several generic layer colors.
- Pad numbers/nets, via nets, repeated readable net names along tracks, grid,
  connection drawing and single-layer connection filtering are functional.
  Status Info currently controls object/net hover information.
- 3D View Options has projection, body visibility, thickness scaling, Realistic /
  By Layer, and the eight material color/opacity rows. Saved native
  `3DCONFIGURATION` COLORREF colors, opacity, background, body visibility and
  thickness scale are read on open. The software viewport supports perspective,
  material blending, paste aperture faces, and layer/highlight dimming. Scaling
  board thickness preserves the height of bottom-side component bodies.
  2D and 3D single-layer state are independent. Entering 3D through the panel
  uses the same pending-edit guard as the existing 3D command.

All preferences remain per open document and view-local. They do not modify
board geometry, history or dirty state and are not written into native/JSON
files or persisted between sessions. Native imported configuration data stays
in the source archive.

## Explicit remaining differences

Full visual or functional parity is **not** claimed. Unsupported actions remain
visibly disabled, rather than silently doing nothing:

- Configuration/layer-set profile creation, save/delete and native profile
  import/export; docking/autohide and the other left-dock tabs.
- Section View plane editing/cutaway rendering. Its observed controls are shown
  disabled in 3D; there is no section renderer behind them yet.
- Rooms, dimensions and coordinates in Object Visibility; text draft rendering;
  Test Points, native Via Span markers, Net Color Override and Special Strings.
- Reference/snap/DRC/comparison system-color categories without a corresponding
  renderer. Their rows remain visible but disabled.

The color picker is still egui's opaque RGB picker, not Altium's palette dialog.
The advanced matrix currently supports Ctrl leaf-cell selection; native range /
parent-cell editing and movable/resizable OS-dialog behavior are not reproduced.
Small font/icon, focus, ellipsis and platform-frame differences remain. Linux
uses the same geometry and bundled Selawik fonts, with an optional Segoe override;
its native font metrics and runtime appearance were not verified this session.
The existing 3D scene still has limitations in solder-mask/copper composition,
internal copper, text, STEP models and native lighting. Material controls do not
establish full-board 3D rendering parity. Native keepout opacity indexing and
exact status-info/track-label behavior need further reference verification.

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

Current revision: 267 workspace tests, fmt --check, strict workspace/all-target
Clippy, Windows debug build and Linux GNU all-target cross-check. No Linux
binary/AppImage was built and no Linux GUI session was run.

New regressions cover configured empty mechanical layers and role/pair ordering,
mirrored camera roundtrip/zoom, independent repeated-label controls, opacity
aggregation and visibility preservation, fast slider drags, modal Apply/Cancel,
composed alpha, native 3D colors/opacity, perspective and thickness scaling,
transparent picking/blending, 3D masks, and repeated track-label orientation.
Real Windows GUI checks used the research copy; view changes left the document
clean. Remaining differences above stay tracked as partial in features.json.

Prior connection validation remains applicable: physical copper intersections,
via spans, plated barrels, holes, stale/shelved and inherited-net pours, minimal
spanning trees and bounded background cache reuse/stale-result rejection.
The earlier TI PMP23595 run (local only) contained 8452 items and 76 nets, with
zero missing island links after inherited polygon nets; its debug connectivity
calculation took roughly three seconds in the background. That result is not
an independent Altium DRC/topology comparison.
