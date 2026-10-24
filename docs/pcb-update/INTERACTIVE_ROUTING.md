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
open. Supported scoped/matrix clearance and board-edge rules are evaluated;
general query compilation, neck-down and impedance rules remain incomplete.
The solver is not a full DRC engine. Native
text/custom shapes retain existing geometry approximations. No-net starts on
existing no-net copper can be blocked by that copper. Obstacle geometry is cached
per route/layer; already-existing illegal starting clearances stop the route.
Net class/delay are N/A; length is a geometric sum, not an electrical topology
measurement. Exact glyph/raster/DPI and Linux runtime parity remain unverified.

## ReferenceM routing startup, 2026-10-24

The read-only probe reproduced a startup failure on the 41,475-item ReferenceM
board: `Unsupported query: InLayerClass('L3_L4')`. Its clearance rules also use
`WithinRoom(...)`, `InPolygon` and the `InPoly` alias. Unsupported scopes abort
obstacle preparation, so the initial click never created a route session.

The shared rule evaluator now handles native layer classes and their `M#` or
`MEMBER#` entries, known built-in layer classes, configured layer names and native
Top/Bottom aliases. Missing named classes have no members; their names are not
used to guess a layer set. ReferenceM references L3_L4/L2_L5/layers classes whose
definitions are absent in its Classes6 records.

Straight-edge RoomDefinition contours support `WithinRoom`. Tracks and other
copper shapes must fit inside the actual filled outline, including concave
rooms. Pad/via membership uses the centre. Native retraced closing edges are
normalized with even/odd fill; ReferenceM's COUPLE contour needs this normalization.
Malformed and curved rooms remain explicit unsupported scopes. Unrecognized
predicates still fail instead of silently selecting a lower-priority rule.
The polygon aliases match generated signal-layer pour copper.

Room-scoped clearance is resolved for each proposed route segment, after a
conservative bounds check. The cached probe at document origin cannot decide
the room membership of a later segment. Obstacle geometry remains cached, and
matrix/scoped rules retain their priority. This also applies after placing a
leg, changing layer or backtracking. General query compilation and full DRC
remain outside the implemented coverage.

The query semantics were checked against Altium's
[membership checks](https://www.altium.com/documentation/altium-designer/pcb/query-functions/membership-checks)
and [attribute checks](https://www.altium.com/documentation/altium-designer/pcb/query-functions/attribute-checks).
There was no new live Altium comparison.

Core regressions cover layer-class kinds/members, native layer aliases, missing
classes, concave-room containment, pad/via centres, retraced closing edges,
unsupported room shapes, polygon aliases and clearance changes after entering
a room. The native-file probe now checks preview, placement, completion and Undo
in memory. On ReferenceM, AR_QSPI_CS starts with the native 0.254 mm preferred
width; preparation took about 413 ms and the three sampled solves 5–8 ms.
These timings exclude GPU painting and do not establish presentation FPS.

A native eframe window exercised the full application with application-local
synthetic primary-button clicks on that pad and a nearby free endpoint. The
first click started the session; moving the pointer created visible preview
copper without changing the document. Placement and Finish Route committed one
history entry. A capture of the application's GPU framebuffer was inspected,
then Undo restored the saved document exactly. The source file was never saved.
This verifies the native canvas/controller path, not physical mouse input or a
new Altium comparison.

Validation: 389 workspace tests, formatting and strict all-target Clippy passed.
ReferenceM PcbDoc/PrjPcb hashes remained unchanged.
