# PCB Active Bar

Reference: Altium Designer Enterprise 24.4.1.13, inspected live on Windows on
2026-11-04. Status: partial. The floating strip, rather than File/Edit/View or
the file toolbar, is the requested scope. Full command and pixel equivalence
has not been established.

## Observations

All fourteen groups and their popups were opened through Computer Use. The
native 2D strip measures approximately 476 by 33 1/3 logical pixels; its buttons
are 33 1/3 pixels wide. At reference window width 1707 its top is y=82. Menu rows
are approximately 29 1/3 pixels, separators 1 1/3 pixels. There is a separator
after Align. Order, captions and initial group commands also agree with the
installed declarative PCB resource; no vendor execution code is used.

| Group | Popup width | Available commands / catalog |
|---|---:|---:|
| Selection Filter | 363 | Shared object selection switches |
| Objects for snapping | 425 | Shared grid and supported snap switches |
| Move | 188 | 7 / 11 |
| Select | 230 | 16 / 21 |
| Align | 255 | 11 / 20 |
| Components | 155 | 1 / 4 |
| Routing | 240 | 1 / 5 |
| Length Tuning | 237 | 0 / 2 |
| Pad/Via | 79 | 2 / 2 |
| Polygons | 166 | 2 / 3 |
| Keepout | 125 | 4 / 4 |
| Dimensions | 203 | 0 / 10 |
| Strings | 115 | 1 / 2 |
| Drawing Tools | 141 | 8 / 8 |

The twelve command groups contain 92 entries, of which 61 are connected to
available workflows. This count describes command availability, not complete
Altium semantics of the existing routing, component, polygon or text engines.
Selection-dependent commands also require appropriate current selection.
Native Routing shows five entries; the additional hidden declarative autoroute
entry is deliberately excluded.

Native dialogs were opened and cancelled: Get X / Y Offsets [mm] is 282 by 228,
Rotation Angle (Degrees) is 258 by 111, and Align Objects is 310 by 240. Offset
has sign buttons, swap, reset and interactive measurement. Align has independent
horizontal and vertical radio groups. The selection was not edited or saved in
the original user project; native probes used an ignored research copy.

## Implementation

Short click invokes the last selected group command. Right-click or a one-second
hold opens its menu; release after the hold cannot also invoke a command. Last
commands are stored in application settings and shared across PCB tabs. Escape
and outside click dismiss menus. Up/Down skip disabled entries and Enter invokes
the focused command. Popup input cannot leak to board placement or global edits.
Menus clamp to the viewport and scroll when necessary.

Selection Filter updates the same fields as Properties. Rooms and Other remain
disabled. Snapping has the native group order and Custom / All - On / All - Off
cycle, retaining the supported custom pattern. Track Lines, Intersections, Pad
Vertices, Texts and 3D Body Snap Points, as well as Guides/Axes, are disabled.
Ctrl+E opens this palette; its exact native cursor-relative positioning is not
verified.

New document-coordinate operations live in eda-core: geometry-based rectangular,
touching-line and lasso selection; atomic object/component alignment and equal
edge-gap distribution; rectangular track outlines; and independent arc placement
geometry. Region selection respects visible/selectable candidates and Shift
addition. Text selection bounds remain conservative. Line placement is distinct
from interactive routing and does not assign the contact net automatically.

Drawing supports Line, Arc (Center), Arc (Edge), Arc (Any Angle), Full Circle,
Fill, Solid Region and Rectangle. Center arcs use four anchors: center, radius,
start, end. Edge arcs use two endpoints for a 90 degree arc; Any Angle uses
start, center, end. Space reverses arc direction. Rectangle inserts four real
tracks as one history operation. Keepout Track/Arc/Region/Fill carry actual
keepout restrictions and no electrical net. Polygon Pour Cutout is a separate
cutout region. All mutations validate before publication and use document Undo.

Move/Drag use the existing bounded model operations. Component movement picks
the owner; Flip currently requires one unlocked component. Offset validates
units and finite document limits and can measure two canvas points. Rotation
accepts an angle before asking for a canvas pivot, rather than inventing a
selection-center pivot. Align combines both radio choices in one history entry.
Dialogs and point-picking retain a revision snapshot and reject intervening edits.

Center alignment asks for a selected reference primitive separately for each
axis, in horizontal/vertical order. A confirmed axis is retained when the next
reference is cancelled, and one Undo restores the full operation. Hovering an alignment reference does not move objects; an axis changes only
after its confirming click. Other alignments keep locked objects fixed
and move the remaining objects. Increase/decrease spacing moves consecutive
reference points by one additional grid step per object, leaving the first
leftmost/bottommost object fixed. The current implementation has one isotropic
document grid; independent Global Board Snap Grid Step X/Y are not modeled.
Move All Components Origin To Grid excludes components with off-grid pad centers
and locked components; the native HTML report is not generated.

Physical Connection, Physical Connection Single Layer, Component Connections
and Component Nets now have separate seed modes and read-only core operations.
Component pads terminate physical routes; vias bridge their actual plated span.
Free pads remain part of the physical route. The single-layer variant omits
vias and component pads but retains free pads. Component nets includes all
electrical members of the component-pad nets, including disconnected copper. The new picking commands
remain active until cancellation. Shorted-net traversal, Shift accumulation,
visibility/filter preferences, blank-canvas component lookup and ambiguous
overlapping seeds need further native comparison.

## Additional native observations, 2026-11-04

Independently authored three-fill and two-net routing fixtures were opened in
AD24 through Computer Use. Single-axis center alignment asked for a primitive.
The combined Center/Center dialog asked twice; cancelling after the first
reference retained that axis, and one Undo restored both completed axes.
The dialog reopened with No Change selected for both axes. Increasing horizontal
spacing at the native 5 mil grid shifted the middle and last fills by one and
two grid steps, leaving the first fill stationary.

Physical Connection on the routing fixture selected three route segments, a via
and the intermediate free pad. The PCB List panel confirmed all five types and
layers. It kept prompting Choose object. The component-pad variants are
implemented from official documentation and core/controller checks; their full
native result sets have not yet been inspected. Single Layer selected the top
track alone, then replaced that selection with the two bottom tracks and
intervening free pad even though Top remained active. PCB List confirmed those
three selected bottom-layer objects. It therefore follows the
clicked track's layer; a via seed uses the active layer. The capture helper
failed twice
with `window crop is outside captured monitor` while opening the PCB panel menu,
so capture was suspended. An accessibility-only observation showed the open
menu, and dismissing it with Escape restored image capture. Component Connections
prompted Choose component; clicking the synthetic pad opened Component Designator
instead of resolving the component. That result set is not claimed verified.

The repeatable fixture author is `eda-io/examples/pcb_active_bar_probe.rs`.
Its source-free binary output did not load successfully in AD24. A new empty
board saved by native Altium supplied the required source container; preserving
that container while inserting the synthetic geometry loaded successfully.
Templates, native documents and captures remain ignored local research files.
No source-free native-write interoperability is claimed by this probe.

## Remaining differences

The following 31 catalog commands are disabled: re-route, break track, drag track
end and group resize; room connections; component text positioning,
the four maintain-spacing alignments;
Reuse Block and both body placements; differential, quick, quick differential
and multiple routing; both length tuning commands; Slice Polygon Pour; all ten
dimensions; and Text Frame. These need actual model/engine workflows before
being enabled.

Icons are original vector renditions. Their exact rasterization, font metrics,
hover/pressed colors, all selection preferences, native properties/modal details,
3D/board-planning Active Bar variants and Linux runtime/DPI remain unverified.
Native capture width was 1707 and height 1026; the maximized PCBCraft capture was
1707 by 1019. Both strips start at y=82; their x positions differ by two pixels
with the current dock widths. This is not acceptance of equal-size pixel parity.
Native filter/snap child clicks through the capture helper dismissed the popup;
their switch/preset semantics are therefore supported by official documentation
and PCBCraft real-window verification, not successful native child clicks.
The existing application Undo path clears PCB selection; the native alignment
probe retained its selection after Undo. Selection retention is not yet matched.
The latest continuation QA restored a 553-pixel Projects dock and a 282-pixel
Properties dock from local settings, so absolute bar X coordinates in that
capture differ further. It checks command behavior and dialog/strip dimensions,
not equal-dock pixel parity.

## Verification

Real Windows QA and final automated gate results are recorded in PLAN.md.
Regressions exercise actual egui hold/release, short click and keyboard events,
persisted last defaults, keepout flags, rectangular insertion/Undo, read-only
region selection, four-anchor arcs, rotation pivot ordering, and atomic/stale
Align dialogs. All three dialog dimensions and measured-offset acceptance/Undo
are covered. Core checks cover copper rather than empty bounding-box corners,
and locked child geometry versus locked component transactions. Native control
artwork, keepout hatching and Shift+Space arc behavior remain unverified.
Additional regressions cover separate center references, partial cancellation,
single-step Undo, the four spacing commands, component/free-pad route boundaries,
single-layer results, connection seed modes and off-grid/locked exclusions.

Official behavior references:

- [Design environment elements](https://www.altium.com/documentation/altium-designer/design-environment-elements)
- [PCB tracks and arcs](https://www.altium.com/documentation/altium-designer/pcb/tracks-arcs)
- [Placement and editing techniques](https://www.altium.com/documentation/altium-designer/pcb/placement-editing-techniques)
- [Cursor snap system](https://www.altium.com/documentation/altium-designer/pcb/cursor-snap-system)
