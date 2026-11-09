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
text/custom shapes retain existing geometry approximations. No-net routes merge
with their contacted terminals and their own copper. Obstacle geometry is cached
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

## Connected track dragging, 2026-10-25

The reported failure was reproduced in the controller. Ordinary primary-button
drag used a rigid transform and disconnected the segment. M,D only stretched
neighbour endpoints and could leave arbitrary-angle tracks.

Ordinary dragging of free copper tracks, M,D for a track selection and track
endpoint handles now use the core routing solver. Edited tracks and neighbours
are rebuilt with 45-degree or orthogonal legs. Fixed pad/via contacts receive
connecting legs. Straight-track T junctions, crossings and existing interior
contacts on affected neighbours become routing waypoints. Width, net, layer and
primitive metadata are retained; the first leg keeps the edited primitive UUID.
Unrelated no-net copper remains an obstacle, including at a new endpoint.

The solver uses the editor's conflict mode and clearance. Walkaround searches
for a complete connection; Stop rejects a blocked edit as one transaction.
Neither mode commits a shortened, disconnected prefix. Final changed copper is
checked again against the completed multi-net geometry. Locked-neighbour errors
and cancellation leave the original board intact. Pointer release commits the
exact cached preview, with one Undo/Redo entry. Controller changes request a new
frame after canvas painting, so the last preview and release appear immediately
without requiring another mouse movement.

Validation on pinned Rust 1.90.0: 414 workspace tests, formatting and strict
all-target Clippy passed. New regressions cover fixed pads/vias, neighbour and
branch rerouting, crossing junctions, selected chains, locked objects, no-net
contacts and obstacles, walkaround/stop, endpoint handles, preview commit,
cancellation, frame requests and Undo/Redo.

The full application was also built and exercised in a real Linux X11 window
using XTest primary-button events without modifiers. A 20 mm track between two
fixed pads was dragged 5 mm. Preview and committed artwork showed the moved
segment and both connections. Save produced three tracks, Undo restored one,
and Ctrl+Y restored the exact saved board. Pad records stayed unchanged.
Captures and saved test documents are under `target/track-drag-check/`.
Only this generated test document was saved. No new live Altium comparison was
performed.

This covers straight track selections. Mixed/component selections retain the
previous endpoint-stretching behavior, and explicit M,M retains breaking Move
semantics. Shove, gloss, redundant-loop removal, posture continuity, arbitrary
arc/pour contact topology, large-board connected-drag latency and Windows runtime
remain outside this verification. Exact Altium interactive-drag parity is open.

## ReferenceB routing startup and obstacle indexing, 2026-10-25

On the Windows ReferenceB/ReferenceBBoard.PcbDoc (15,513 items), RouteSession
startup aborted on `InPadClass('TP_Pads') Or CompParameterValue('ID') Like '216'`.
InPadClass was already supported; component parameters were archived but unread.
The next blocking query after that fix was `HasPad('BM1-0')`.

The importer now reads base component parameter groups in PrimitiveParameters,
checking record bounds, owner index and native UNIQUEID. Footprint/System and
named-variant groups remain separate and preserved in the source archive.
Component parameters have their own model field, independent of native metadata.
Old JSON recovers missing data from its archived native source using component
identity, including after index/position edits. Unavailable parameters remain
unknown and cannot silently fall back to a generic clearance rule.

Supported additions: CompParameterValue with Like (`*` and `?`), string `=`/`<>`,
HasPad full names (including Free pads), wildcard HasFootprint, ObjectKind string
equality and pad Name string equality. Existing boolean/priority evaluation
continues to reject unsupported branches. The ReferenceB `ID=216` query returns 24 pads
on the 24 TP components, consistent with an independent read of native groups.
Native parameter edits and ASCII export of populated parameters are refused;
JSON retains them. Unchanged binary parameter streams survive byte-for-byte.

The R-tree optimization described in PERFORMANCE.md applies to routing, via
clearance and connected track drag. Regressions compare indexed blockers with
exhaustive checks across mixed shapes, changing clearances, room/matrix rules,
errors and extreme coordinates. Source order remains deterministic.

Validation: 438 workspace tests, strict all-target Clippy, formatting and Windows
dev build. Native file probe verifies preview/place/finish and exact Undo. Real
Windows window: Fit Board, start NetC13_2, place, Escape to finish, Ctrl+Z restores
the clean document. No source writes; original SHA256 is recorded in the JSON
measurement file. Altium Enterprise 24.4.1 was inspected on a research copy at
1707 px width; applying the test PCB Filter query returned “Error occurred while
Running script”, so this does not verify live query-result or visual parity.

Semantics references: [Altium component parameters](https://www.altium.com/documentation/altium-designer/pcb/query-functions/parameters),
[HasPad and membership functions](https://www.altium.com/documentation/altium-designer/pcb/query-functions/membership-checks),
[Like wildcards](https://www.altium.com/documentation/altium-designer/query-language).
These supplement the native evidence and do not prove version-specific parity.

Still open: full query language (including native complex hole/annular scopes),
variants, parameter editing and native writing, shove/gloss/loop removal, preview
copies/validation/artwork, full-board presentation latency and exact Altium UI.

## Route preview reuse, 2026-10-25

RouteSession's draft is private and exposed read-only. Hover uses a reusable
validated prefix plus new legs, with stable active-leg IDs and immutable retained
snapshots. Placing, via changes, backtracking and History keep complete validation.
The app skips unchanged requests and releases its previous preview before an
update; connectivity workers own separate geometry snapshots. No model command
depends on the renderer or GPU.

The renderer keeps the source spatial tree, updates explicitly split tracks, and
paints the placed/active tail in the original layer ordering. Unchanged pad,
drill, mask and stationary GPU buffers survive tail count/posture changes.
Source geometry can be reused on entry; unrelated previews/revisions cannot.
Live tessellation uses the stationary document anchor to avoid one-pixel rounding
shifts. GPU tests compare full redraws with linear/sRGB, masks, highlight, draft,
single layers, mirrored camera, splitting, vias and backtracking.

On the full Windows ReferenceB board, CPU preview median dropped from 57.5230 to
5.7226 ms in the initial optimized run; this does not measure executed GPU work
or presented FPS. A 90.7778 ms cold frame remained. See PERFORMANCE.md and
`route-preview-performance-2026-10-25.json` for conditions, later source-reuse
measurements, raw summaries and limitations.

449 workspace tests pass. The native Windows window verified NetC13_2 placement,
Backspace, Finish and Undo/Redo/Undo to a clean document without saving the source.
Linux all-target compilation passes; Linux runtime and exact Altium parity remain
unverified. Shove/gloss, arc routing, loop removal and the remaining query/native
I/O restrictions above are still open.

Preview snapping now shares the displayed source's immutable spatial tree rather
than building a second complete geometry cache. Route splits and transformed
previews cannot alter that source tree; candidates keep original coordinates
and document order. Revision changes expire it, and cold entry has a spatial-only
fallback. Off skips object lookup while preserving the grid; Ctrl+Shift bypass
remains raw. Actual canvas tests compare both source and preview endpoints,
cold/warm entry, All/Current/Off, grid and bypass with full model snapping.
Windows Current Layer route/place/Finish/Undo passed on the full ReferenceB board;
the original Off mode and clean document were restored. 451 workspace tests pass.
Whole-frame latency improvement from this specific cache change is not proven;
see `route-snapping-performance-2026-10-25.json` and PERFORMANCE.md.

Stationary artwork now survives source-to-route entry in groups matching the
original layer/type ordering. Route-tail slots exist in the source capture;
stale polygon copper and split source tracks expire their groups while other
meshes/atlases/GPU buffers remain shared. Native Windows ReferenceB reused 155/155
groups at entry and 151/155 when copper became stale. Source pad labels, holes,
masks, fresh/stale transitions and a reused GPU scene lacking mesh zero are
covered by complete-redraw pixel comparisons, including camera movement.

452 workspace tests and all required gates pass. Native route placement,
Backspace, Finish and Undo restored the clean original board; no source save,
SHA256 unchanged. Final headless first CPU samples were still 33–37 ms; native
Finish/repour/Undo still rebuilt all groups and had artwork costs up to 67.69 ms.
This is a bounded rendering optimization, with full routing/Altium parity still
open. See `artwork-groups-performance-2026-10-25.json` and PERFORMANCE.md.

## Routing startup and controls, 2026-10-31

### Observed

Altium Designer Enterprise 24.4.1 was inspected at 1707 px width, with the
330 px View Configuration dock and 316 px Properties dock. Route menu order,
Interactive Routing/Ctrl+W, floating Active Bar, Net Information columns,
layer/via/dimension controls, conflict/corner controls and the Tab pause overlay
were observed directly on the existing research PCB copy. The user project was
not saved or edited for these observations.

PCBCraft had a different fixed text toolbar. Its floating PCB Active Bar was
unreachable and its routing button had no command. P,T was intercepted by the
global Cross Probe T prefix. In addition, a no-net routing session treated its
starting pad/track and its own tracks/vias after a layer change as obstacles.
The existing native research copy reproduced a session without a departing leg;
isolated geometry regressions establish the self-obstruction independently of
that board's other constraints.

### Implemented

- PCB Active Bar is displayed over the canvas in the observed order, with a
  working Interactive Routing command and selected state. Route menu entries
  follow the observed order; unavailable engineering commands stay disabled.
- PCB command sequences run before the inactive Cross Probe prefix handler.
  Explicit placement commands leave Cross Probe. Routing opens Properties.
- Starting on a visible copper object switches to its layer. Direct copper hits
  take priority over nearby pads. Drill-only holes cannot become routing targets.
  A new free-space route does not inherit the previous route's net.
- No-net sessions exempt only contacted terminals and their own added copper.
  Explicit endpoint contacts can connect, while unrelated no-net copper,
  keepouts and drills still obstruct the route. Contacts restore on Backspace,
  and layer changes reuse existing plated-through pad connections.
- Net Information uses Name/Class columns. Layer/via dimensions, preview,
  conflict controls and disabled advanced options follow the observed section
  order. Tab pauses, focuses Width and displays a clickable pause overlay.
  The status bar reports starting/next location or a clearance blockage.

### Verification and limits

Regression coverage includes P,T through the full shortcut dispatcher, Ctrl+W,
Cross Probe coexistence, visible/hidden layer starts, fresh no-net starts,
terminal versus unrelated copper, track splits, via/backtrack and History Undo.
A panel-height regression protects the visible dimension/conflict controls.
Windows GUI on `target/routing-check/RoutingReference.PcbDoc` verified the Active
Bar, a two-pad No Net route with an intermediate corner, automatic termination
and Undo to the clean board. This generated ASCII fixture was rejected by Altium
and is not evidence of native interoperability or equal solver behavior.

The final Windows build also exercised the native 15,513-item ReferenceB copy at
`target/routing-check/ReferenceBRouting.PcbDoc`: Ctrl+W, ExampleGround startup at
(192.273037, 190.895608) mm, approximately 0.996 mm placed copper, Tab/Width,
single Escape to resume, Escape to finish, Undo/Redo/Undo. The final title was
clean. Both this copy and `.tools/pcb-padstack-reference.PcbDoc` retained SHA256
`redacted-source-sha256`.
An obstructed start elsewhere on ExampleGround displayed the clearance-blocked status;
the follow-up below identified and corrected that inherited-pour error.

Final checks: formatting, strict workspace/all-target Clippy, all 563 workspace
unit tests and the Windows dev build passed. The test log is
`target/routing-check/workspace-tests.txt`. Native saving/reopening of the new
route and a new Linux GUI comparison were not performed in this iteration.

Exact pixel/DPI and solver parity remain open. Push/HugNPush, arc corners,
gloss, loop removal, route auto-completion, width/via favorites and full
preferences are not implemented. Net class/delay and electrical length remain
limited as above. No new Linux runtime comparison was performed. Observed UI
states and implemented behavior must not be interpreted as acceptance of these
remaining features. The existing research PCB source was not saved.

## Follow-up: no track after selecting a terminal, 2026-10-31

The user's clarification prompted a comparison at the previously blocked
ExampleGround via (166.180275, 199.690002) mm, native Vias6 record 141. Altium 24.4.1
starts and places routing from that terminal. PCBCraft's read-only probe
originally returned zero segments in all three sampled directions.

The blocker was imported pour copper: Regions6 and ShapeBasedRegions6 record
17 have no explicit NET and refer to polygon 1. Connectivity already inherited
the polygon's ExampleGround net, while routing treated these fragments as foreign
No Net copper overlapping the starting via. This was an incorrect obstruction,
not an actual clearance violation at the starting location.

`Board::item_net` now provides the shared explicit-net/parent-polygon resolution.
Routing obstacles, rule net scopes, InNet/InNetClass and connectivity use that
identity without rewriting the imported fragments. Shelved pour fragments are
excluded from routing obstacles. Explicit foreign fragment nets, unrelated
foreign pours and keepouts still blocked in that iteration; the foreign-pour
restriction is corrected by the following follow-up.

New regressions cover starting inside one's own inherited pour, layer-change
vias/backtracking, foreign versus shelved pours, explicit fragment overrides,
keepouts and scoped clearance membership. The native probe now reaches the
(+1,+1) mm and (+3,0) mm sample endpoints and completes preview/place/finish/Undo
without writing its source. It supports `--at X Y --blockers` for read-only
startup diagnosis. Fmt, strict all-target Clippy, 566 workspace tests and the
Windows dev build pass. Native saving and full solver parity remain unverified.

Final Windows GUI repeated that exact terminal: Ctrl+W, click at the ExampleGround
via, move the cursor without a placement click, then place the preview. The
previously absent departure now appeared and the connection completed; Undo
returned the copy to a clean title. Both PCB file hashes above remain unchanged.

## Follow-up: routing across foreign polygon copper, 2026-10-31

The preceding fix only removed same-net false blockers. User feedback exposed
the remaining error: generated copper from another net's polygon was treated
as an immutable solid obstacle. Managed pour fragments (regions, fills, tracks
and arcs with a known polygon owner) are now omitted from routing obstacles.
Placement invalidates the pour; the existing background repour cuts clearance
around the committed tracks and vias. Free copper, orphaned polygon references,
keepouts and board cutouts still obstruct. This is not Ignore Obstacles mode.

A regression routes from a SIGNAL pad inside GND pour in Stop and Walkaround,
previews/places a leg, changes layer through a via, backtracks and finishes.
After repour it checks actual polygon intersection against a 0.6mm scoped
clearance envelope, plus single-step Undo and Redo of the route and derived
copper. The source board remains unchanged during the draft. Boundary tests
cover managed foreign fragments versus free copper, unknown owners, keepouts
and board cutouts. All 567 workspace tests, fmt check and strict all-target
Clippy pass on Windows.

Observed in AD24.4.1 on the research copy: HugNPush permits a 2.87mm preview
across the filled area; it was cancelled without saving. This particular
observation started in free space (N/A net), so it does not establish exact
pad-start parity. Official polygon documentation describes repouring after
layout changes: https://www.altium.com/documentation/altium-designer/pcb/signal-layer-polygons

Windows GUI on ReferenceBRouting: start at ECALL_SW via (screen 710,683 at Fit Board),
move to (760,640), place, Escape. Preview and placed track cross the pour;
automatic repour creates the clearance channel. Undo restores a clean title.
The updated executable is target/routing-check/pcbcraft-polygons.exe, launched
separately because the previous running build has unsaved user edits. The old
window remains untouched. Full solver/visual parity and native saving of this
route remain unverified.


## Existing-route replacement ? 2026-11-06

Observation: live AD Enterprise 24.4.1 on PROC010B showed Automatically Remove
Loops enabled, its Shift+D shortcut, and the separate enabled Remove Loops With
Vias option. Startup and Tab pause were inspected without placing segments or
saving the original PCB. Altium exited during inspection and was relaunched;
the original documents reopened. A separate RerouteResearch.PcbDoc copy was
prepared for further experiments. This is not a verified solver parity claim.
The official interactive-routing documentation describes replacing a parallel
old path after reconnecting to existing copper and finishing routing:
https://www.altium.com/documentation/altium-designer/pcb/routing/interactive

Implementation: RouteSession distinguishes placed route IDs from split old
copper, including split descendants and Backspace checkpoints. At Finish, an
unambiguous unbranched same-layer old track path between placed contacts is
removed in the same validated History transaction as the replacement. Source
splits preserve outside tails. Multiple contacts are considered even when
routing continues after a rejoin. Cleanup runs on a clone: draft preview,
Cancel and source board remain unchanged. Automatically Remove Loops is enabled
by default; the Properties checkbox and Shift+D toggle it for the editor.

Bounds: old paths containing locked/component tracks, interior terminals,
geometric T/crossing contacts or branches are retained conservatively, as are
ambiguous existing loops. Multilayer/via and No Net loop replacement, per-net
loop-removal preference editing, live deletion preview, gloss and exact sliding posture
remain incomplete. Remove Loops With Vias stays disabled. These limits are
explicitly narrower than Altium's routing engine.

Native Nets6 records on the research board contain LOOPREMOVAL=TRUE. Both native
and ASCII imports retain this field; a FALSE/0 per-net value now prevents old
path removal regardless of the editor toggle. Conservative bounds also retain
old paths near arbitrary arc/fill/region contacts which this graph cannot prove
safe to remove. Regression coverage includes both cases.

The live rerouting experiment on the isolated native copy used a differential
pair member and Ignore Obstacles. It did not establish reliable deletion parity:
the selected old track remained visible after rejoining and finishing. Conflict
mode was restored to Walkaround and Undo was invoked without saving.
Consequently only startup/options/width pickup are live observations; the new
replacement algorithm follows the documented behavior and has independent tests.

Verification on 2026-11-06: 781 workspace tests pass, strict all-target Clippy
and formatting pass. Native Windows portable GUI verifies middle-of-track
replacement with both tails retained, Ctrl+Z/Ctrl+Y, Shift+D disabling cleanup
and the enabled Properties checkbox; the unsupported via-loop option stays
disabled. Artifacts: target/routing-check/reroute-*.jpg and
reroute-workspace-tests.log. Exact Altium solver/sliding parity remains open.


## Body dragging / single-segment sliding ? 2026-11-06

The user's priority is ordinary primary-button dragging of an existing segment.
The old drag translated both endpoints by the full pointer delta, then routed
neighbours afresh. On a diagonal-ended straight chain this introduced extra
elbows instead of shortening/lengthening the original adjacent straight legs.

The core now projects a single segment's body motion onto its normal. For a
simple chain, the moving supporting line is intersected with each unique free
neighbour's original supporting line. The outside neighbour endpoints stay
fixed; widths, IDs and metadata are retained. Corner inversion or passing a
fixed outside vertex rejects the candidate; prepared previews refine/retain
the last valid location. Along-track pointer motion is an exact no-op.
The immutable Slide description is prepared once per gesture and is shared
by the complete transaction and preview paths. Endpoint edits and rotated or
multiple-segment selections keep their existing explicit behavior.

The source topology is inspected before endpoint reshaping. Interior junctions,
fixed terminals and unresolved arc/fill/region contacts use the bounded connected
rerouter after normal motion; they do not acquire a false angle-preservation
claim. Shove, gloss, topology collapse through multiple corners, general curved
sliding, arbitrary-angle solver parity and full multi-segment sliding remain open.

Reference: Altium's official tracks/arcs and interactive-routing documentation
specifies sliding, angle preservation and shortening/lengthening connected legs:
https://www.altium.com/documentation/altium-designer/pcb/tracks-arcs
https://www.altium.com/documentation/altium-designer/pcb/routing/interactive
Implementation is independent Rust code, with no Altium runtime dependency.

Verification on 2026-11-06: all 785 workspace tests, strict all-target Clippy,
formatting and Windows build pass. Core regressions cover rotated chains,
diagonal sliding, tangential no-op, preview/commit identity, source restoration
and stopping before corner inversion. The ordinary mouse controller regression
covers normal and fast press/release and one-step Undo/Redo.

In the real Windows GUI, dragging the middle of a three-track chain with both
horizontal and vertical pointer displacement changes the middle from
(4,4)-(12,4) to (6,6)-(10,6) mm. The count stays at three, outer ends stay fixed,
and adjacent 45-degree lines remain straight. Undo restores the original and
Redo restores the shortened middle in one step each. Screenshots and check logs
are under target/routing-check/sliding-*. The clean test instance was closed.

Live AD24.4.1 attempts on disposable documents did not establish the equivalent
three-leg coordinate comparison. These implementation checks therefore do not
certify full native solver parity. No original user Altium document was saved
or edited for this sliding investigation.
