# PCB Rules and DRC — 2026-10-24

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
Class membership accepts imported numeric `M0`/`M1` and `MEMBER0` fields,
checks class kind, and supports the observed All Nets/Components/Pads superclass
records. A full query language and class editor are not implemented.

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
Name/Comment/Unique ID use one row. Scope presets include imported net, layer,
component and class lists; manufacturing hole rules have Pad/Via presets.

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

Document rule view and unsupported report options are disabled. Rule Wizard,
Query Builder and Query Helper have bounded implementations described below.
Native report formatting/automatic Create Report File, exact icons,
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

## Live follow-up — 2026-10-24, still in progress

### Observed

Altium Designer Enterprise 24.4.1 and PCBCraft on the same Windows desktop,
with captured logical main-window sizes 1707×1026 and 1707×1019 respectively.
This is a live layout comparison, not a certified equal-window/equal-DPI pixel
diff. The available reference is ReferenceB adaptive `ReferenceBBoard.PcbDoc` (130 rules).
Further potentially modifying exploration uses `target/rules-reference.PcbDoc`.
An early wizard inspection implicitly applied an Altium dialog draft in memory;
the original PCB was never saved. Its SHA-256 remains
`redacted-source-sha256`.

Observed root/category/type summary tables, rule priorities, Wizard pages,
Query Builder, Query Helper, DRC options/type table, and Clearance, Width,
Routing Via Style, Hole Size, Annular Ring, Hole To Hole, mask expansion and
Polygon Connect constraint layouts. The native tree keeps file order, which
differs from the numeric priority order on this board.

### Implemented

- Category/type/instance tree and summary tables, independent of the list of
  executable checks. Unsupported families remain disabled/read-only.
- Draft-only Wizard for supported kinds, scopes and priority. Finish adds to
  the current rule draft; Apply/OK is the board/history transaction.
- Query Builder for a conjunction of supported predicates; helper insertion at
  the caret/selection, board-name lists, escaped quotes and syntax validation.
  Cancel/Escape do not modify the rule. Unsupported operators are disabled.
- Fixed dialog regions, caption-only dragging, clipped table cells, reserved
  footer positions and separate status windows. Numeric editors preserve
  incomplete decimal input while focused; sibling regions use distinct IDs.
- Observed dimensions/sections and independently drawn constraint illustrations
  for the supported forms. Clearance Simple groups Track/Arc and copper types;
  mixed groups display N/A without rewriting native matrix values on viewing.
- Editable per-layer preferred width in the Width table and core resolution/
  validation of native `*_PREFWIDTH` overrides.
- Duplicate native rule IDs no longer select the first matching record blindly.
  Untouched and distinctly named records retain their source/prefix; ambiguous
  renamed duplicates refuse native save. Unicode numeric shadows and UTF-8
  shadows are decoded into canonical property values, while archived bytes are
  preserved. Editing a comment replaces obsolete encoding shadows.

### Verification and remaining work

Automated checks cover read-only browsing of every catalogue page, draft/history
isolation, Wizard footer bounds, query-dialog bounds/insertion, decimal input,
child IDs, class membership, per-layer widths, duplicate-ID source identity and
Unicode shadow replacement. Native layer-width changes on the ReferenceB board have
been serialized and reimported entirely in memory; the original is not written.

Live PCBCraft checks have exercised Rules/summary/custom scopes, Query Builder,
Query Helper and Wizard navigation/cancellation, plus hole and mask forms.
Final live verification of the latest drag/ID/Unicode fixes is pending.
The latest checkpoint passes 352 workspace tests, strict all-target Clippy,
format check, Windows bins/examples build and Linux all-target compilation.
The updated ReferenceB layer-width probe reimported 5,086,720 serialized bytes in memory.
The source PCB was not written.

Full visual parity is **not achieved**. Remaining differences include unsupported
family constraint pages, native Document View, complete query vocabulary and
builder nesting, all Wizard scope subpages and artwork, exact raster icons and
illustrations, several link/popup styles, per-side/percent/hole-edge mask rule
controls, annular-ring waiver semantics/help, native report generation/formatting,
the docked violations browser, all context menus, and equal-size/DPI pixel and
Linux GUI validation. Linux all-target compilation has passed for this follow-up.

Further native observation was interrupted when opening Panels caused two
captures to fail with `window crop is outside captured monitor`. The user reported closing
the menu. On the next observation attempt, Computer Use reported a physical
Escape stop and instructed that no further app input be issued in this turn.
Final comparison remains pending; the task is not complete.
