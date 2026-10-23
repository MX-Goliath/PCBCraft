# Interactive Routing — 2026-10-23

## Observed in Altium Designer Enterprise 24.4.1

The reference is ReferenceA. Experiments used a separate native PCB copy at
`target/interactive-routing-reference/ReferenceARouting.PcbDoc`; the original
project and its existing unsaved Altium state were not saved or overwritten.
The copy was opened as a free document, Interactive Routing was entered with
Ctrl+W, started on a test-point pad, and paused with Tab.

The native Properties panel has Net Information (name/class, length, delay),
Properties (layer, via preview, via diameter/hole, track width), then Interactive
Routing Options. The observed width is 0.254 mm; via diameter/hole 0.8/0.4 mm.
Conflict Resolution lists Ignore Obstacles, Walkaround Obstacles, Push Obstacles,
HugNPush Obstacles, Stop At First Obstacle, AutoRoute Current Layer and AutoRoute
Multilayer. Shift+R is shown beside it. Five corner buttons and Shift+Space are
shown. Routed/neighbor Gloss each have Off/Weak/Strong controls; automatic
termination and loop-removal options follow. Tab pauses routing and focuses
width; Escape first leaves parameter editing, then ends the current route.

## Implementation

The independent Rust router adds Ignore, Walkaround and Stop modes. All route
geometry remains in eda-core. Tracks use analytic capsule distances, avoiding
pathological buffering of extremely thin/long tracks; arcs use bounded chord
segments conservatively inflated by their sagitta. Other supported copper shapes
use existing polygon geometry. Layer spans, keepouts, mechanical holes and board
cutouts participate. Polygon-pour cutouts alone are passable. Assigned same-net
copper can merge; unrelated no-net copper remains an obstacle.

Walkaround uses bounded best-first detours around obstruction bounds. Its budget
is 96 expansions and 24 path points; on exhaustion it retains a collision-free
prefix. This is an independent conservative approximation, not Altium's solver.
Stop mode clips the current leg before the first collision. Preview and commit
use the same resolver; blocked destinations do not trigger automatic completion.
Via placement checks all copper layers in the span and fails atomically when
clearance is violated. Backtrack restores the draft, layer and obstacle cache;
finished routes retain existing History/Undo/Redo and native-save guards.

The Properties panel follows the observed section order and includes a drawn via
preview, layer choice, net/length, width, via dimensions, conflict mode, corner
buttons and automatic termination. Unsupported controls are disabled. Shift+R
cycles implemented conflict modes; +/- changes routing layer, with Tab,
Space/Shift+Space, Backspace and Escape retained. Initial width and via dimensions
read the highest-priority unconditional native rule; unknown higher-priority
scopes do not silently fall back. Clearance remains explicit/manual.

## Validation

Regression coverage includes obstacle clearance, a 1 nm wide obstacle between
endpoints, 45-degree detours, preview/placement agreement, independent layers,
via-span rejection, transactional backtrack, blocked-target completion, passable
pour cutouts versus board cutouts/keepouts and conservative rule fallback.

Windows GUI on the ReferenceA copy verified Ctrl+W, net NetDA1_34, a placed leg,
Top → Bottom via with keypad +, Backspace restoring Top, Shift+R, Tab pause,
route completion and Undo restoring the clean document. Original PrjPcb/PcbDoc
SHA-256 hashes remained unchanged.

`pcb_route_profile` is a read-only native-file probe. Initial ReferenceA measurement:
982 model items, obstacle preparation about 28 ms and three blocked-route solves
about 12–13 ms each. These exclude GPU painting and do not establish frame-rate
or Altium solver parity. Final build/test/GUI results are recorded in PLAN.md.

## Remaining differences

Full interactive-routing parity is not achieved. Push/HugNPush, multi-layer
routing search, rounded/arc corners, gloss, loop removal, hug/posture continuity,
auto-complete, width/via favorites and full preference/shortcut parity remain
open. The solver does not compile scoped/matrix clearance rules, board-edge
clearance, neck-down or impedance rules, and is not a full DRC engine. Native
text/custom shapes retain existing geometry approximations. No-net starts on
existing no-net copper can be blocked by that copper. Obstacle geometry is cached
per route/layer; already-existing illegal starting clearances stop the route.
Net class/delay are N/A; length is a geometric sum, not an electrical topology
measurement. Exact glyph/raster/DPI and Linux runtime parity remain unverified.
