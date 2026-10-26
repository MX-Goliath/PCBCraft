# Schematic ↔ PCB / Engineering Change Order

Working implementation, 2026-10-26. Full Altium parity is **not** established.
Altium Designer Enterprise 24.4.1 is an observation reference; no Altium process
is used to compile or execute PCBCraft changes.

## Observations

On the original ReferenceA project, without executing changes:

- PCB Design starts with `Update Schematics in ReferenceA.PrjPcb`, then
  `Import Changes From ReferenceA.PrjPcb`, then Rules.
- Import opens Engineering Change Order, 1085 × 503 physical pixels on the
  observed 1918 × 1030 window. The grid has Modifications / Status headings,
  Enable, Action, Affected Object, Affected Document, Check, Done and Message.
  Groups include Add Components (12) and Add Pins To Nets (22).
- Validate marks the unavailable library footprints `Footprint Not Found` and
  their dependent pins `Unknown Pin`. Check is separate from Done.
- Footer: Validate Changes, Execute Changes, Report Changes…, Only Show Errors,
  Close. No original project modifications were executed or saved.
- Reverse update first reports 47 differences, of which 34 can generate an ECO.
  Its first group is Remove Pins From Nets (22); collapsing it exposes Remove
  Component Class Members (12). These are not twelve symbol deletions.
  This observation does not prove
  that all wire/symbol changes can be performed automatically. The automation
  helper intermittently failed to activate owned dialogs or timed out.

Primary behavioral references:
[Keeping the schematics and PCB synchronized](https://www.altium.com/documentation/altium-designer/sch-pcb/keeping-synchronized),
[Placed component types](https://www.altium.com/documentation/altium-designer/schematic/placed-components).

## Implemented

- `eda-core::design_sync`: source snapshots, stable Unique ID links, unique
  designator fallback only when the PCB has no source ID, multipart folding,
  explicit ambiguity/unannotated/duplicate-ID rejection. Mechanical and graphical
  components are excluded; Standard, Net Tie and Standard (No BOM) participate.
- Native pin endpoints are rounded in source fractional coordinates before deriving
  the integer-nm pin length; independent rounding previously severed ReferenceA BQ1-2,
  C2-1 and C20-2 by one nm. No wire-connection tolerance was widened.
- Flat electrical compilation uses exact document coordinates, an R-tree and
  disjoint sets. Wire T endpoints and junctions connect; bare interior crossings
  do not. Repeated physical pin numbers in multipart symbols are unified.
  Named power objects merge across sheets; labels remain sheet-local. A collision
  between different sheet-local names is an error, not an implicit PCB net merge.
- SCH → PCB: add/remove components, designators, comments, descriptions, component
  types, parameters and source links; real footprint geometry replacement at the
  previous pose/side; net creation/removal and all pads of each logical pin.
  A bijective rename keeps the net index and existing routed copper assignments.
- Footprint geometry resolves from project PcbLib/IntLib entries and installed
  local libraries, in that order. PCB6 mixed primitive streams, pad stacks,
  masks, graphics and available embedded 3D resources are decoded independently.
  Unsupported primitive/count/encoding cases make the footprint unavailable.
  Existing board instances no longer substitute for missing libraries in the UI.
  Pad stacks, masks, owned graphics and available 3D resources are retained;
  copied component/item identities are renewed. New components are placed beside
  the board. Missing geometry is a failed operation, never a synthetic footprint.
- PCB → SCH in the native UI: linked designators/comments/current footprint.
  The core can also edit descriptions/types/base parameters and bijective named
  nets, but the native back-annotation policy disables those extended operations
  in app/CLI orders, following the documented native direction limits.
  Existing parameter DisableSync is respected. Automatic symbol creation/removal,
  arbitrary rewiring, compiled class updates and pin/part swaps remain unavailable.
- Selected operations are staged on private documents. Failed operations leave
  their candidate unchanged; successful dependencies are visible to following
  operations. Every result is validated before any History commit. Successful
  rows become Done; remaining rows can be reselected, validated and executed.
  Source document identity/content and app session/revision guards reject stale
  orders, closed documents and unfinished edits. Each changed document gets one
  ordinary Undo entry per execution.
- App: project-scoped Design commands in PCB/SCH, board chooser when needed,
  Project → Show Differences, grouped ECO checkboxes, validation messages,
  Check/Done states, error-only filtering, enable/disable-all and text report.
  Other projects and alternate boards are excluded from the chosen order.
  ECO rows use natural reference order (BQ1…TP9…TP17…WA1); display sorting does
  not reorder the staged dependency execution.
- Binary PcbDoc base component parameter serialization: changed/new/deleted
  values, Unicode and source links. Untouched native parameter/variant/system
  groups and unrelated per-parameter fields are retained. Serialized files are
  re-imported and verified before writing a destination.
- Native ASCII/CFB SchDoc ECO writing: designators, comments, descriptions,
  component types, base parameter insertion/removal/Unicode, current/new footprint
  models and named-net text. New child records remain within their owner's tree;
  Weight, OWNERINDEX and existing model maps are retained/updated appropriately.
  A re-import must reproduce the editable scene before atomic destination replacement.
  New parameter/model Unique IDs are deterministic and collision-checked, so
  repeated saves of the same scene retain their identities and archive bytes.
  Unchanged archives are byte-identical; unsupported geometry and opaque record/index
  dependencies fail without touching the destination. Save/Save As now expose SchDoc.

## Explicit remaining differences

- No DbLib/Workspace lookup, manual Component
  Links editor, per-row bidirectional comparator, native ECO generation options,
  class/room/rule/variant propagation, bus/harness/hierarchical instances or
  pin/gate swaps. Unsupported connectivity is diagnosed and is not applied.
- Native hidden-pin connectivity is absent from the current schematic importer;
  a schematic containing such records cannot silently overwrite PCB pin nets.
- Net naming preferences, aliases, single-pin/project scope behavior and arbitrary
  multipart/library corner cases are not an Altium-compatible project compiler.
- Automatic reverse creation/deletion of symbols and wire edits is not implemented.
  Back-annotated supported properties can be saved as native SchDoc; arbitrary
  symbol/wire/geometry edits still require JSON. Native PCB saves can also reject unsupported 3D body,
  opaque-reference/index, typed stack or polygon dependencies; JSON preserves them.
- Local PCB6 libraries are supported; native model-to-pattern aliases, library
  refresh/search priorities for ambiguous names, external STEP resources, unsupported
  STEP assembly transforms and full footprint text/font parity remain incomplete.
  Placement packs actual footprint bounds beside the board; native rooms are absent.
- Earlier GUI notes called Home "Fit Document" incorrectly: the PCB binding is
  center-at-pointer; Fit Document is Ctrl+PageDown / View command. The empty view
  after Home is not evidence of an Undo bounds/cache failure. Actual Ctrl+PageDown
  after the latest ECO → Undo retained the visible clean ReferenceA board.
- Altium's inline parameter table displays the diagnostic newline marker `~1`
  literally. Changed/new multiline parameters are therefore refused on native
  save and require JSON until their native encoding is verified. Existing
  unchanged native parameter payloads remain preserved.
- Menu/dialog font, icons, exact widths, selection/context menu behavior, report
  printing and Linux runtime visual equivalence are not yet verified as exact.

## Verification

The native fractional-pin regression covers all four orientations, positive/negative
coordinates, exact net membership and an intentional one-nm gap that stays disconnected.
Core tests cover source links and renames, pin connectivity, missing footprint and
pad dependencies, multipart shared pins, parameters, locks, partial re-execution,
local-name collisions, stale/closed sources, convergence and Undo. App tests cover
scoped commits, validation requirements, dirty state and Undo invalidation. Native
parameter tests verify read/write, Unicode, source IDs and untouched variant data.

Read-only ReferenceA inventory command:

```
cargo run -p eda-app --example design_sync_profile -- <ReferenceA.PrjPcb>
```

The initial inventory produced 41 changes / 30 applicable on ReferenceA.PcbDoc.
After including Standard (No BOM) types and fixing native endpoint rounding, the
inventory produced 60 changes / 49 applicable in 1.379 s, including 12 Add Components
and 22 Add Pins To Nets, with no spurious pin removals on the original board.
It uses existing board footprint
geometry and different comparator settings; these counts are not equal to Altium's
47 comparator differences or its ECO rows. GUI Validate/Execute and one-step PCB
Undo restored the clean document; the original PcbDoc SHA256 remained
`redacted-source-sha256`.
Final build/test/GUI results are recorded in PLAN.md.

The current default diagnostic uses library resolution; pass
`--reuse-board-footprints` only to reproduce the earlier explicit reuse experiment.
`--library=<PcbLib-or-IntLib>` adds a diagnostic library and `--reverse` selects
PCB → schematic comparison. None of these comparison modes writes a project file.

Native writer verification on a separate ReferenceA copy caught a defect missed by
self-import: Altium ignored globally appended records beyond the old Weight.
After correcting Weight/tree placement, AD24.4.1 loaded the copy and displayed
the new footprint, Unicode base parameter, comment and description on WA1.
The original project was not executed or saved. Seven native writer regressions
cover archive byte equality, unchanged record payloads, Windows-1251/UTF-8,
owner remapping, model maps, net names and failure-before-write.
PCB library tests exercise standalone and compressed integrated containers through
actual core ECO insertion, plus unknown primitive/count/truncation rejection.
Real ReferenceLibraryA PcbLib: 29 electrical footprints, four graphical patterns
explicitly unavailable because they have no pads. Real TPS22929 PcbLib and IntLib:
three footprints each; unsupported STEP assembly transforms remain a warning.
