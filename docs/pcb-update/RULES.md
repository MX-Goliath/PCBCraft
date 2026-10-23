# PCB Rules and DRC — 2026-10-23

Status: partial. This is an independent implementation, not complete Altium parity.

## Observed reference

Live Altium Designer Enterprise 24.4.1, original ReferenceA project, 1920×1030
window. The reference PCB was not saved or edited during observation.

- Design → Rules: PCB Rules and Constraints Editor [mm], category/type/instance
  tree, name/comment/unique ID, two object queries, constraints, clearance matrix,
  Priorities, default rules, OK/Cancel/Apply.
- Tools → Design Rule Check: Design Rule Checker [mm], Report Options and Rules
  To Check, separate Online/Batch columns, maximum violations, Run/OK/Cancel.
- Tools → Reset Error Markers and Browse Violations (Shift+V).
- Polygon Connect Style: Simple and Advanced; Through Hole Pad / SMD Pad / Via
  columns, Direct / Relief / No Connect, Air Gap / Conductor Width, 2 / 4 / Auto,
  45 / 90 Angle. ReferenceA's PolygonConnect_0.4mm uses three prefixed constraint sets.
  Auto was observed but is not yet implemented; its control is disabled.
- Further live Altium navigation encountered `failed to activate captured window`.
  Existing screenshots and official documentation were used for subsequent work.
  No reference edits were applied.

Primary semantic references: [Manufacturing rules](https://www.altium.com/documentation/altium-designer/pcb/design-rule-types/manufacturing),
[rule scoping and priority](https://www.altium.com/documentation/altium-designer/pcb/defining-scoping-managing-design-rules),
[polygon connections](https://www.altium.com/documentation/cstu/polygon-connect-style?version=4).
These documentation references supplement, rather than prove, AD 24.4.1 visual parity.

## Implemented

Rules are preserved as native fields in the GUI-independent board model. A draft
is validated and committed atomically to history. Opening/cancelling is read-only;
Apply is undoable. Drafts cannot replace a newer board revision. Native binary
edits, duplication, reordering and deletion preserve rule prefixes and untouched
records. New native types require an existing record of that type as a template;
otherwise native save refuses and JSON remains available. ASCII rule count changes
remain unsupported. Rule/class tables are checked by reimport before native save.

Priority resolution uses the lowest priority number, enabled state, symmetric
pair scopes, net scope and the native object-clearance matrix. Supported queries
include All, boolean AND/OR/NOT and parentheses, primitive types, copper/side/layer,
net, component, footprint, named polygon and imported class membership. Unknown
queries produce an explicit error instead of silently using a generic rule.
Class membership support is bounded to imported MEMBER fields; a full query
language and class editor are not implemented.

Editable/executed constraints: Clearance, Short Circuit, Un-Routed Net, Modified
Polygon, Width, Routing Layers, Routing Via Style, Hole Size, Hole-to-Hole
Clearance, Minimum Annular Ring, Board Outline Clearance, Solder Mask Expansion,
Paste Mask Expansion, Polygon Connect Style, Minimum Solder Mask Sliver and
Silk To Solder Mask Clearance (non-text primitives). Mask expansion is consumed by the existing mask renderer;
it is not a standalone pass/fail DRC test.

Interactive routing resolves net-specific preferred width/via dimensions,
enforces width/layer/via limits, and uses per-pair clearance and board-edge rules
for Stop/Walkaround. Polygon repour consumes clearance and board-edge rules;
changing those constraints invalidates the previous pour. Name/comment edits do
not invalidate geometry. Polygon connections resolve both object scopes and
priority, simple or THPAD./SMDPAD./VIA. constraints, Direct/Relief/NoConnect,
2/4 conductors and 45/90 degree rotation. Automatic or unknown thermal settings
produce explicit diagnostics/errors. Isolated pads do not anchor copper islands.

Mask checks share actual expanded/tented apertures with the mask model. Manual
mask primitives merge with pad/via openings before sliver checks. Top and Bottom
are independent. Silk checks distinguish exposed pad copper from mask openings;
text glyphs are explicitly reported as unchecked. Spatial bounds prune distant
mask comparisons. Board cutout constraints use the stricter electrical Region
and CutoutEdge rule in DRC, routing and pours. Zero outline clearance permits edge
crossing. Cavity/split-region columns remain disabled.

Routing reports width and layer violations independently and rejects ordinary
via placement that violates Hole Size or Minimum Annular Ring constraints.
New rules receive highest priority; duplicates rank directly below their source.
Name/Comment/Unique ID use one row; query scopes use All/Net/Layer/Custom Query.

DRC evaluates physical copper layers independently of displayed layers. It uses
polygonal copper geometry, slot outlines and padstack geometry. The background
worker checks snapshots and discards results after document, preview or settings
changes. Online/Batch filters, report limit and marker creation are persisted by document
path in PCBCraft preferences (not written into native Altium checker options).
Current reports can be exported as standalone escaped HTML, including unchecked
conditions, incomplete status, coordinates and object identities. Green
markers, filtering, jump-to-object, and reset/show controls are implemented.
Caps on reported violations and candidate pairs explicitly mark incomplete checks.

## Verification

Automated coverage includes priorities, disabled and reversed scopes, native
matrix units, query parsing, width/clearance/board-edge routing, actual copper
distances, layer isolation, allowed shorts, annular rings, report limits, rule
Undo, stale drafts/results, unchanged opening, dialog bounds, and native
edit/duplicate/delete/reorder with opaque field retention. Polygon rules have a
repour invalidation/geometry/idempotence regression.

Read-only ReferenceA probe: 982 items, 68 rules. The supported batch checks produced 297
violations in approximately 0.94 s on this Windows machine before the final UI
changes. This is not an Altium result-count comparison or a claim that the board
passes DRC. Unsupported checks are listed separately.

Previous-stage validation: `cargo fmt --all -- --check`, strict workspace/all-target Clippy,
329 workspace tests and the Windows build. A changed Width rule in ReferenceA was
serialized and reimported in memory (3,328,512 bytes); rule fields matched and the
source was never written. Live Windows checks covered Rules layout and Cancel/
Escape, Online/Batch setup, batch execution, green markers and jump-to-track.
Only one PCBCraft instance is run at a time.

Reference PCB SHA-256 remains
`redacted-source-sha256`.
Reference project SHA-256 remains
`redacted-source-sha256`.

## Remaining scope

Full restoration remains open. Missing engines include creepage, unconnected-pin
semantics, advanced SMT, differential pairs, high-speed/length/impedance,
testpoints, complete silk glyphs and merged-aperture internal mask slivers,
net antennae/acute-angle checks,
component placement/3D clearance/height, power plane connection-rule
evaluation, and signal integrity. Polygon Auto spokes, per-pad thermal overrides
and exact custom-pad thermal placement remain unsupported. Unknown imported types are retained read-only.

Routing remains the existing bounded Stop/Walkaround implementation, without
shove, native same-net exception semantics, gloss or complete short-circuit/net-tie
routing behavior. Copper text strokes, negative planes, cavity/split edges, hole span/microvia
exceptions and arbitrary native property/query semantics are incomplete.
Geometry approximations are not certified native DRC equivalence.

Rule Wizard, Query Builder, document rule view and unsupported report options are
disabled. Native report formatting/automatic Create Report File, exact icons,
constraint illustrations, all dialog/context-menu behavior, complete visual
parity and Linux GUI validation remain unfinished.

## Checkpoint at user-requested stop

Feature work stopped at the user's request before final GUI comparison of the
follow-up changes. New Rules header/scope/thermal UI has headless layout and
read-only regressions, but the latest version has not been checked in a live
PCBCraft window. Do not treat it as visually matched to Altium.

ReferenceA rules (Width, PolygonConnect, mask-sliver and silk/mask comments) roundtripped
in memory: 3,329,024 bytes; original file not written. Rule-driven repour of three
polygons completed in approximately 1.58 s. New DRC checks reported 335 violations
and 22 unchecked conditions; this is not a native Altium comparison. The first
mask-check implementation took about 49–50 s on ReferenceA. A spatial/grouping optimization
is included in this checkpoint, but a subsequent run still took 82.33 s while
workspace tests were compiling. The slowdown is unresolved; no performance fix is
claimed. All 339 workspace tests, fmt check, and strict all-target Clippy pass at
this checkpoint.
