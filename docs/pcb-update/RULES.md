# PCB Rules and DRC — 2026-10-26

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
Polygon, Width, Routing Layers, Routing Via Style, Hole Size, Layer Pairs, Hole-to-Hole
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
workspace tests were compiling. At that checkpoint the slowdown was unresolved (diagnosed and fixed in the
2026-10-26 section below); no performance fix is
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


## 2026-10-26 — Hole Size Percent and ReferenceA DRC latency

Implementation:

- Hole Size now exposes **Absolute / Percent**, with separate `MINLIMIT` /
  `MAXLIMIT` and `MINPERCENT` / `MAXPERCENT` values. Switching mode retains the
  inactive values. Percent values appear in the rule summary and use the same
  validation in batch/online DRC and interactive via placement. Invalid, negative,
  non-finite or reversed percentage ranges are rejected before Apply. Percentages
  over 100 are accepted (a mechanical hole can exceed its land).
- New Hole Size defaults match the documented 1–100 mil and 20–80% ranges.
  Existing imported defaults are retained. Apply is one undoable edit; browsing
  either mode does not modify the draft. Native ReferenceA serialization/reimport keeps
  the percentage mode and fractional values (25.5 / 82.25) exactly, in memory.
- Hole-to-hole DRC builds each drill contour once and sweeps bounds expanded by
  the maximum enabled clearance. Rotation and slot length use drill geometry,
  not copper bounds. Only candidate pairs consume the explicit pair budget.
  A full-pair reference comparison covers scoped priorities, coincident holes
  and rotated slots; 3,000 distant drills no longer consume the budget.
- Full circular arc strokes now produce a sampled annulus directly, or a disk
  when the stroke covers the centre. This avoids pathological centreline buffer
  joins when the inner radius is close to zero. Partial arcs keep their existing
  path. Geometry tests cover a 1 nm inner hole, zero/negative inner radius, odd
  stroke widths, translated centres, exact axial bounds and area error below
  0.011% at the existing 256-segment full-circle resolution.

Observations and measurements:

- Read-only ReferenceA profiling isolated three Top Overlay arcs with radius 75,001 nm
  and width 150,000 nm. Their generic buffer construction cost 7.84–7.94 seconds
  each, repeated in board-outline and silk/mask checks. Hole pairing was not the
  main source of the previously recorded 49–82 second delay.
- Same Windows optimized debug profile: baseline 49.7204 s; final 1.0234 s.
  Another final run during Clippy/roundtrip work took 2.4197 s. These are individual
  measurements, not a latency guarantee. All runs returned 335 violations with
  identical per-kind counts, 22 diagnostics and no truncation: outline 14,
  clearance 137, hole-to-hole 1, annular ring 36, mask sliver 22, short circuit 58,
  silk/mask 16, unrouted 51. Counts do not establish exact marker-coordinate parity.
- ReferenceA native in-memory roundtrip: 3,331,072 bytes, all rule fields equal after
  reimport. Source SHA256 remains
  `redacted-source-sha256`.

Reference and remaining uncertainty:

- Altium's [manufacturing rules documentation](https://www.altium.com/documentation/altium-designer/pcb/design-rule-types/manufacturing)
  defines the Percent method relative to pad/via size, with 20/80 defaults.
  **Implementation assumption, not a new Altium observation:** for rectangular
  lands we use the smaller nominal dimension (independent of rotation), and for
  varying stacks check each physical copper land. Exact Altium behavior for these
  cases and slotted/custom pads still needs a controlled comparison.
- Live AD 24.4.1/PCBCraft window verification could not be completed: Computer Use
  returned `window.app must be a non-empty string and window.id must be an integer
  >= 0` on window selection/activation, including the recovery attempt. No new
  pixel-parity claim is made. No additional PCBCraft instance was launched.
- Stacked-microvia hole-clearance exemption remains unsupported and now emits an
  explicit diagnostic when enabled. Other missing rule families, glyph outlines,
  docked violation browser and UI differences above remain open.


Validation for this continuation:

- `cargo fmt --all -- --check` and
  `cargo clippy --workspace --all-targets -- -D warnings`: pass.
- `cargo test --workspace`: 458 unit tests passed, one existing GPU test failed:
  `routing_tail_matches_full_gpu_pixels_and_keeps_buffers_across_bends_vias_and_backtrack`.
  `Rgba8Unorm`, frame 13, dithering enabled: maximum channel difference 4 versus
  tolerance 3. All seven added tests passed (six core, one app).
- The failure reproduced independently on the unchanged `9c60e1c` source exported
  to `target/rules-head-validation`, using the same Windows GPU. The test tolerance
  and renderer were not changed. Therefore the full workspace gate remains red
  on this host for an existing rendering issue, not a newly claimed green run.


## 2026-10-26 — Layer Pairs

Observed in AD Enterprise 24.4.1 on the original ReferenceA: Manufacturing → Layer Pairs
→ LayerPairs; one object scope, a Constraints section and the label **Enforce
layer pairs settings** to the left of a checked box. The native ReferenceA rule stores
`ENFORCE=TRUE`; Board properties contain `LAYERPAIR0LOW=TOP`,
`LAYERPAIR0HIGH=BOTTOM` and one substack GUID. Observation did not edit the rule.
The owned-modal screenshot must be used for input: main-window activation failed
intermittently. Closing the Altium dialog later timed out twice, so further
Altium input stopped; the reference dialog may remain open.

Implemented:

- Layer Pairs is editable in the Rules tree, summaries and wizard and selectable
  in the Online/Batch checker. The observed label/checkbox order is reproduced.
- DRC and interactive via placement share scope/priority/enabled/`ENFORCE`
  resolution. Disabled enforcement skips the check. Via span is compared in
  physical stack order, accepting reverse traversal. Diameter/drill constraints
  remain the responsibility of Routing Via Style and Hole Size.
- An edited typed layer stack is authoritative; its via definitions specify
  allowed spans. The empty typed table uses the existing manager's default
  through span. Without a typed stack, imported `LAYERPAIRnLOW/HIGH` values are
  read without changing board properties. An absent native table uses the
  default through span. Incomplete/unknown/multiple-substack tables produce
  explicit diagnostics and routing errors rather than falling through.
- Drilled pads use their full-stack drill span. Rejected routing transitions
  preserve the route draft, anchor, layer and backtrack history. Native rule
  roundtrip retains the `ENFORCE` toggle and unrelated rule/board fields.
- Five core regressions cover typed/legacy precedence, physical/reverse spans,
  dimensions independent of span, pad drilling, scoped priorities, disabled
  enforcement, checker exclusions, malformed input and atomic routing rejection.

ReferenceA validation: 335 violations, no truncation, **21** diagnostics (Layer Pairs is
now checked), 1.0222 seconds. Native roundtrip after toggling `ENFORCE` in memory:
3,331,072 bytes; all rule fields equal after reimport; original not written.

Remaining: multi-region/flex substacks, mirrored/skip-microvia definition
metadata, per-via technology identity and complete native via-type import/editor
roundtrip are not implemented. The manager's native legacy blind/buried table
presentation still needs restoration; a typed edited table is checked correctly.
The whole Rules dialog still differs in outer dimensions, row metrics, fonts and
icons; no pixel-exact claim is made. Linux runtime was not checked this turn.


Layer Pairs validation: Windows build, fmt and strict Clippy pass. Final workspace
run: 463 passed, one existing GPU dithering test failed with the same 4-versus-3
mismatch reproduced earlier on unchanged `9c60e1c`; test tolerance unchanged.
The Windows PCBCraft form, draft toggle and Cancel were exercised on ReferenceA; no
source save. One live instance is retained after sequential build restarts.

Final Windows rebuild was re-observed with the corrected left-aligned label and
16 px checkbox. The sole running PCBCraft instance is left on the unchanged
ReferenceA LayerPairs rule for review. Outer-dialog metric differences remain open.
