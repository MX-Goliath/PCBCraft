# PCBCraft implementation plan

### Pre-publication privacy review — November 8, 2026

Reviewed tracked files, synthetic fixtures, contributor metadata and all locally
reachable Git objects. Removed residual private reference names and a local report
path from current documentation, retaining neutral reference labels and evidence.
No personal home-account names, private network addresses, personal emails or
credential findings were detected in the reviewed publication content. Public
GitHub attribution and third-party license attribution are retained.

The history scanner now recognizes the historical public service no-reply address
only in commit author/committer headers and .mailmap. Four regression tests cover
the exact contributor address, header/message boundaries, unrelated emails,
private paths, credentials and unreviewed design locations. CI includes these
tests alongside the release tests. All 10 Python tests and the full-history
privacy scan pass.

Older reachable snapshots still contain previous private reference labels;
this normal commit does not erase them. History was not rewritten. The local
audit report is kept in ignored target/privacy-audit-2026-11-08.json.

### English documentation and contributor identity — November 8, 2026

Translated all 24 Markdown files containing Russian text, including this log,
the full original target plan and AGENTS.md. Preserved example code blocks,
link targets, task completion states, technical evidence and historical limitations.
Existing English documentation remains in English.

Standardized project attribution on MX-Goliath in README, workspace/package
authors, project copyright notices, Windows publisher and Debian maintainer.
Repository-local Git identity uses the account's public GitHub no-reply address;
.mailmap maps historical repository identities without rewriting commits.
Third-party attribution is preserved. Privacy checks allow the exact public
account address and scope the historical T3 mapping address to .mailmap only.

Validation: cargo fmt --all -- --check, strict workspace/all-targets Clippy and
all 854 workspace tests passed through scripts/dev.ps1, including GPU tests.
All 6 Python release tests, targeted privacy/author-inheritance assertions,
documentation-preservation checks and the repository privacy scan passed.
No application interface changes were made in this stage.

### Direct footprint update from PcbLib — November 8, 2026

Continuation of §9/§11 of the original `ALTIUM_RUST_AGENT_PLAN.md`.
AD 24.4.1 verified Update PCB With Current Footprint and the 424 × 458
Component(s) Update Options dialog. Isolated PCBCraft_Update_6CF7B9E1:
90° rotation, U1 / LOCAL_VALUE, pad 1 from 2 × 2 to 3 × 2 mm; native UniqueID
LRMUVKPG, position and labels are preserved. Parameter update replaces old
Parameter 1 with LibraryVendor=LibraryVendor; Text / On Any Layer preserves the
component designator/comment. Selected options persist between calls.
Native verification files: `target/library-observations/footprint-update-20261108`.
User projects were not saved in Altium.

Implemented GUI-independent `footprint_update`: normalized geometry, instance
side/rotation/position, nets by pad number, identity/label/local-mask preservation;
separate mask updates without geometry replacement. The selected option replaces
footprint parameters, stored separately from schematic parameters in
`footprint_parameters`, native APPURTENANCE=Footprint / VARIANTGUID=Footprint,
preserving base/System/variant groups and recovering old JSON by native UID.
Properties shows Component / Footprint provenance. Tools / Current / All and
the PCB Library context menu cover all matching open boards. All replacements
are prepared before changing any history; every board gets Undo without implicit
writing. Revisions, paths, open-board membership, pending edits and read-only
state are checked. Ambiguous nets, connected-pad removal and locked/unsupported
data are rejected. Thermal Relief Overrides is disabled: cross-archive copying
is not modeled; matched native pads preserve original unread bytes.

Fixed U1 import from Texts6 when Components6 lacks SOURCEDESIGNATOR; a populated
source field retains priority. Binary regressions cover ASCII/Unicode, exact
unchanged Save and unrelated geometry changes without rewriting Components6.
Removed an inapplicable missing Board6-outline warning in PcbLib.
Real-window verification found a 7 px dialog expansion caused by the close box;
layout was fixed and rechecked in the window.

Intermediate GUI check: Current / Cancel without board changes; All / OK,
Undo to clean baseline, Redo, LibraryVendor / LibraryVendor / Footprint table,
Save As to `GuiFootprintNamespaced_v1.PcbDoc`. AD opened v1 clean; native
Footprint/System/base groups are separate. Byte comparison found a device-path
prefix and retained old HEIGHT=212.9921mil instead of library HEIGHT=0mil.
Fixed native-path conversion, HEIGHT and FOOTPRINTDESCRIPTION writing/removal,
read-back checks and Properties height display. v1 remains an intermediate file;
the final run followed source/test finalization.

Final verification: `cargo fmt --all -- --check`, strict workspace/all-targets
Clippy, Windows dev build and all 854 tests passed through `scripts/dev.ps1`:
app 225, core 344, geom 1, io 166, render 118, including GPU. Log:
`footprint-update-20261108/final-854-rust-gates.log`. New checks cover both board
sides/rotation, nets, masks independent of geometry, stale/atomic/Undo, parameter
namespaces and legacy JSON, variant-group preservation, height changes/removal,
description and ordinary Win32 source links.

Final GUI check: original 5.410 mm height displays correctly; Current Footprint →
OK changes pad 1 and height to 0.000 mm; Undo restores clean baseline, Redo
restores the update. Save As wrote separate `GuiFootprintVerified.PcbDoc`.
Reimport confirms U1 / LOCAL_VALUE, position (2461387000, 76835000) nm and 90°,
pad 1 ≈ 3 × 2 mm, pad 2 ≈ 2 × 2 mm, base parameters = {},
footprint_parameters = {LibraryVendor: LibraryVendor}. Raw check: UniqueID
LRMUVKPG, HEIGHT=0mil, ordinary library path; Texts6/Data matches baseline
byte-for-byte. AD 24.4.1 opened the final file without a dirty marker;
PCB/Components and Properties show U1 / LOCAL_VALUE, 90°, Height 0mil, correct
geometry and LibraryVendor / LibraryVendor / Footprint. One PCBCraft instance
is open with clean final file and library.

The overall request remains partial. Full parity is not claimed: PCB-side
comparison/ECO, thermal overrides, PcbLib Place, native auxiliary/3D, all pad
mapping changes, exact typography/icons and Linux GUI remain in `docs/features.json`.

### Instance updates from SchLib — November 8, 2026

Continuation of §9/§11 of the original `ALTIUM_RUST_AGENT_PLAN.md`.
Observation: AD Enterprise 24.4.1 on isolated UpdateReference verified Update From
Library, Advanced, individual parameter values and ECO. Original 10k/47k become
library 12k unless excluded from updating. AD output saved separately as
NativeUpdated.SchDoc; user projects were not saved.
Generator: `eda-io/examples/library_update_probe.rs`.

Implementation: GUI-independent `library_update`, preserving identity, position,
rotation, designator and selected local parameters; separate graphics/models/
labels/lock-field settings. Wizard from schematic and SchLib, sheet/instance
selection, grouped ECO and report. Comparison is guarded by revisions and file
fingerprints; closed sheets open with Undo without implicit writing.
All replacements are checked before histories change. Ambiguous sources are rejected.

Parameters follow the observed AD matrix: system/user columns, cell selection,
Update/Reject Selected and All, Show all parts. Bulk commands cannot reenable
protected values. Instance page separates schematic, library and Actions columns;
wizard buttons use observed 75 × 23 dimensions. Full paths distinguish sources
with identical filenames. Preserved labels are not duplicated by hidden scalar
Comment/Vendor records; body/pin order is checked in the native file itself,
not merely normalized in model comparison.

Verification: final `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, Windows dev build and
`cargo test --workspace` passed through `scripts/dev.ps1`. Total 844 tests:
app 222, core 340, geom 1, io 163, render 118, including GPU. New checks:
atomicity/stale sources, closed-sheet Undo without writing, locks, unchanged
external wires, native rotations/UID/graphics order, physical Comment/Vendor
counts, single-cell selection and pending Properties before direct Update Schematics.

Real Windows window 1707 × 1019: partial update, U2 / Value → Reject Selected,
ECO Validate/Execute, Check/Done, Undo/Redo and native Save As.
Final `update-20261108-v8/GuiUpdatedVerified.SchDoc` reread: U1 = 12k, U2 = 47k,
Vendor = Example, one Comment/Value/Vendor record each, two bodies and four pins,
original positions/rotations. AD 24.4.1 opened this GUI file without dirty marker;
Fit All Objects shows P1/P2 and 12k/47k. Intermediate v6/v7 remain with defect
descriptions rather than being replaced by successful results. Direct SchLib /
Update Schematics checked separately: Information “Updated 2 components / In 1
Schematic Documents”; `GuiDirectUpdate.SchDoc` has 12k/12k and Source = Parts.SchLib;
one Undo restores the clean source sheet with 12k/47k. One PCBCraft instance is open.

Next-path observation: unique PCBCraft_Update_6CF7B9E1 on a separate synthetic PCB,
90° rotation, U1 / LOCAL_VALUE, pad 1 from 2 × 2 to 3 × 2 mm.
PcbLib / Update PCB With Current Footprint applies options immediately; native
UniqueID, position, rotation and local comment are preserved. The implementation
and designator-import fix for this path are described in the stage above.
User projects were not saved in AD.

The overall request remains partial: exact visual equality, complete PCB updates,
full multipart/variants, Choose Component, DbLib/Workspace and remaining original
scope are not declared complete. Linux GUI/DPI were not checked at this stage.
Remaining boundaries: `docs/interface/library-authoring.md` and
`docs/features.json` (`libraries.update_instances`).

### File-based component libraries — November 8, 2026

Work under §9/§11 and the §7/§25 visual contract in the original
`ALTIUM_RUST_AGENT_PLAN.md`. Full Altium equivalence is not claimed.

- Observation: Altium Enterprise 24.4.1, synthetic SchLib/PcbLib only.
  Checked part tree, General/Pins, Tools/Mode, empty new variants, mode renaming,
  PCB Library and Edit → footprint properties. Separately saved NativeModes.SchLib
  with `CustomDisplayModeName1=Alternate QA`. Original user projects were not saved.
- Implementation: delete parts/modes across variants with renumbering and Undo/Redo;
  protect Normal/last part; shared Type and mode names. Native names read/write
  without retaining deleted keys. Library Designator is separate from the placed
  part suffix: repeated saves do not accumulate A/B.
- UI: one symbol per list entry, Part A/B, Description/search, Part in Properties,
  Tools/Mode, Pins with visibility/Edit/Delete, Hidden/Locked, footprint properties
  and height in current units. Creation opens Library. Unsupported native Tools
  disabled; additional commands separated.
- Fixed renamed footprint-model replacement instead of addition, preservation of
  other models/current flags/native metadata, source index during pin edits,
  Cancel/Esc and name errors without losing entered text. 2D preview uses an
  unambiguous model and prioritizes an open PcbLib.
- Available Libraries: Refresh/Remove, retain previous snapshot on read failure,
  preserve file on Remove. Open published libraries refresh after applied revision
  and Undo/Redo; Save As relocates the entry without duplicates.
  Windows canonical/raw paths normalized.
- Verification: final `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, Windows dev build and
  `cargo test --workspace` passed. 827 tests: app 214, core 332, geom 1, io 162,
  render 118, including GPU. Commands use local GNU toolchain in `scripts/dev.ps1`.
  First run caught a canonical/raw-path Remove defect; repeated full runs passed after fixing it.
- Windows GUI 1707×1019: File/New/Library/Create, new SchLib, pin and native Save;
  tree/Part A/B, Normal/Service GUI, Rename/Cancel, pin visibility and Undo,
  SCH/PCB Library header switching, footprint properties, open-model 2D preview,
  Edit/double-click, renaming one model row and Undo. Final alignment/arrow checked
  after rebuilding. PCBCraft instances ran sequentially; final instance left open.
- Native: GUI-created `GuiNew.SchLib` opened in AD 24.4.1 with Component_1, U? and
  pin 1. GUI edits of native source saved separately as `GuiVariants.SchLib`:
  AD shows Part A/B, original graphics, TEST_2PAD model and `Service GUI` in Tools/Mode.
  Rust example generated AuthoringVariants/AuthoringReduced.SchLib and
  SchLib/PcbLib/IntLib without manual byte edits. Artifacts only in ignored
  target/library-observations. Linux GUI/DPI and full pixel equality unverified.

Remaining: pixel/DPI/Linux GUI comparison, placed-instance updates/conflicts,
full multipart annotation, DbLib/Workspace/PvLib, integrated library projects,
wizards/reports and unsupported native resources/3D.
The last file-library member cannot yet be deleted.
Details: `docs/interface/library-authoring.md`; `libraries.authoring`
in `docs/features.json` remains partial.

### Independent panel movement — November 8, 2026 clarification

The user's explicit clarification makes every header area and tab move only the
active panel. The earlier interpretation of free header space as a whole-group
drag was removed. Neighboring Git, Projects and other panels stay in place.

Close, context commands and pin also act individually. Autohide detaches the
selected panel from the tree while preserving neighbors' position/visibility;
allowed dock directions persist individually across group moves and restarts.
Old root settings provide fallback.

Added regressions for free-header dragging of each of eight panels, independent
close/pin and Allow Dock persistence. All 818 workspace tests passed: 209 app,
330 core, 1 geom, 160 io, 118 render. Formatting, all-targets type checks, Windows
dev build and strict Clippy passed. In the real window with the user's existing
right-hand group, dragging free header space detached only Git;
Projects/Properties/Components/Messages stayed grouped. The next drag docked
Projects separately on the left; Git retained its native window and others
stayed right. Fixed build left running as the sole PCBCraft process.
The user explicitly authorized closing the previous session without saving.

### Movable panels — November 8, 2026

Work under §7 and §25 of the original `ALTIUM_RUST_AGENT_PLAN.md`.

- Observed Altium Enterprise 24.4.1 headers/tabs, Allow Dock, pinning, side buttons
  and popup; restored original Altium layout. Automated header dragging in Altium
  produced no observed result; other grouping rules supplemented from official documentation.
- Replaced two fixed areas with a persistent tab/split tree: four edges,
  panel/group movement, tab order, nested splits, Ctrl/Esc, separate native
  viewports, autohide, F4 and Allow Dock.
- Closed panels retain position; group membership, active tabs, ratios, sizes,
  native bounds and visibility persist. Old settings migrate. Properties works
  after moving and in Layer Stack Manager.
- Windows GUI: detach Projects, Dock back, merge with Properties, vertical split,
  pin/side buttons, layout restoration after restart and detach from popup.
  Fixed Windows-frame offset in native bounds; two successive observations
  confirmed stable position.
- Fixed width at initial 0 px window, wide popup content overlapping side buttons,
  and coordinates between viewports with differing DPI. Deep split geometry is
  constrained to actual available space.
- Final `cargo test --workspace`: 815 passed (206 app, 330 core, 1 geom, 160 io,
  118 render), including GPU. Formatting checked. Adjacent popup switching and
  DPI conversion covered by regressions. `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings` and Windows dev build
  passed on the current tree. Commands used `scripts/dev.ps1`, supplying local
  GNU 1.90.0 and linker; Cargo.lock unchanged.
- Final executable rechecked in Windows: Messages → Properties → Messages through
  side buttons works with popup already open. F4 checked from native panel and
  main window. Computer Use native edge-resize produced no observed size change;
  it is not counted as successful GUI verification. Instances tested sequentially
  on the built-in example.
- Linux cross-check stopped at missing target OpenSSL for existing reqwest/native-tls.
  Linux GUI and real monitors with different DPI unverified. Wayland does not
  report global window coordinates to eframe.

Full Altium parity is not claimed: exact pixel metrics, floating-panel opacity,
popup animation/preferences, desktop snapping, TLT and moving a complete fractal
root by one shared header remain differences. Details/controls:
`docs/interface/docking.md`; `shell.docking` in `docs/features.json` is partial.

### Repository privacy — November 4, 2026

Private reference names were replaced with neutral ReferenceA and subsequent
labels. Personal paths, source-project checksums and personal author emails were
also removed from Git history. Synthetic fixtures and the complete plan log were
preserved. Added private-file ignore rules and CI privacy checks; local reports
default to target. See docs/privacy.md.

Remote main/master and local service-ref histories were cleaned; old objects
removed from the local Git database. All-reachable-history checks, negative
scanner scenarios (index, UTF-16, commit metadata), cargo fmt, strict Clippy and
cargo test --workspace passed. Sources after history rewriting match the verified
tree. An old SHA remains available through GitHub API outside branches: removing
server copies requires a separate GitHub Support request; local text is prepared,
but no request was sent.

### Current PCB debugging — ReferenceA, October 23, 2026

At the user's instruction, the sole current real reference is
`private-projects/ReferenceA/ReferenceA.PrjPcb`.
Compare with installed/running Altium Designer Enterprise 24.4.1.
Do not change source files; use copies to save edit results.

- [x] Prepared Windows MSVC environment with Rust 1.90.0; build/launch checked.
- [x] Reproduced/fixed `V7_LAYER=CONNECT` region import failure.
- [x] Fixed hidden layers for native `SINGLELAYERMODESTATE=3` (Off).
- [x] Connected native range/layer mode/arc centre/board outline snapping.
- [x] Deferred initial fitting of an empty viewport.
- [ ] Full visual/functional equivalence with Altium's PCB editor.

Observations, checks and remaining differences:
`docs/pcb-update/VISUAL_PARITY.md`, `docs/features.json`.

October 19, 2026. Full work program: `ALTIUM_RUST_AGENT_PLAN.md`.
This file defines iteration order and current-stage status.

## Required visual contract

The user's clarification requires an exact visual copy of available Altium
Designer Enterprise 24.4.1 on Windows and Linux. Original
`ALTIUM_RUST_AGENT_PLAN.md` remains the complete master plan; sections below only
sequence/log its tasks. Visual-parity work in sections 7 and 25 starts now and
continues in every iteration, rather than waiting for the engineering core.

- [ ] Compare windows of the same editor, size, DPI and state.
- [ ] Complete menus, quick access, Projects, right dock area and tabs.
- [ ] Reproduce Active Bar, Properties, status bar and editor canvas.
- [ ] Check shared fonts, metrics, icons, colors and states on both operating systems.
- [ ] Record measurements/comparisons; do not claim exact parity beforehand.

Current-work mapping to original plan: workspace §2, coordinates §3,
model/history §4, original IO §5, canvas §6, shell §7,
visual comparison §25, completion criteria §26.

## Initial state

The directory contained only an architectural plan; no Rust workspace or Git.
Altium Designer Enterprise 24.4.1 is available with ReferenceB.PrjPcb.
Home, PCB Editor, File and Properties observed.
Other editors/commands need their own investigation.

## Stage 1 — main window (current iteration)

- [x] Create Rust workspace: geom, core, io, render, app.
- [x] Separate documents from view/interface state.
- [x] Create menus, toolbar, tabs, document tree, Properties,
      Messages, status bar, panel visibility/size controls.
- [x] Empty PCB/SCH creation, sample board, switching/closing.
- [x] Canvas: pan/zoom, fit, grid, coordinates, layer visibility, pad selection.
- [x] Pad parameter changes in one transaction, undo/redo.
- [x] Original versioned JSON, Save / Save As / Open / Recent.
- [x] Save / Discard / Cancel prompt when closing changed documents.
- [x] Unit/grid settings, settings/panel-size persistence.
- [x] Windows build, fmt, clippy, tests and basic real-window manual scenario — see `docs/verification.md`.
- [ ] Run Linux CI and visual X11/Wayland launch.
- [ ] Full docking, autohide, separate native windows and session recovery.

### Main-window and Properties visual comparison iteration

- [x] Observe Board and selected Pad in open Altium without project edits.
- [x] Shared title strip, Quick Access, original vector icons,
      window commands through portable eframe/winit.
- [x] Properties: search, collapsible Selection Filter, Snap Options,
      Board Information, Net Information, Properties and Pad Stack.
- [x] Real pad-selection filter, axes, snap settings and geometric snapping to
      original pads/tracks without renderer model mutation.
- [x] Coordinate/size/layer/Designator edits through history;
      apply on input completion, manual X/Undo check.
- [x] Embedded free Linux fonts, system Segoe UI on Windows, custom font paths.
- [x] fmt, clippy, 6 tests, Windows build and real-window inspection;
      all workspace targets cargo-checked for x86_64-unknown-linux-gnu.
- [ ] Resolve remaining docs/interface/visual-parity.md differences;
      exact stage visual parity is not yet confirmed.

Stage boundary: first working window with original documents.
Having a shell does not imply functional parity with all of Altium.

## Stage 2 — model, geometry and files

### Opening/viewing Altium schematics (user priority)

- [x] Independent Schematic model, original records/source archive; ADR 0002.
- [x] PrjPcb: document references, relative Windows paths, portable case lookup.
- [x] SchDoc CFB/ASCII 5.0: records, encodings, fractional coordinates,
      components/parts/display modes and main graphical primitives.
- [x] Schematic canvas: sheet, wires/symbols/text/images, zoom/pan/fit,
      component/object selection, Properties and incomplete-support messages.
- [x] Original-format Save As retains scene/native original; guard against JSON over Altium.
- [x] Original minimal fixture/headless inspector; several real projects read.
- [ ] Complete visual templates, port/IEEE styles and text.
- [x] Basic scene editing through commands/history — see iteration below.
- [ ] Full editing, native writing, electrical connectivity and ERC.

### ReferenceB visual comparison — October 20, 2026

- [x] Separate exact Altium GOST Type A and GOST Type AU names;
      system family lookup and free Liberation Sans fallback for Linux.
- [x] IEEE Dot size, High Current, LED arrows, separate pin fonts,
      pin-name/GND/NoERC spacing; junction color from adjoining wire.
- [x] Shared Designator/Comment in alternative symbol modes;
      XP1 and Comment restored; pin-owned Net Name remains metadata.
- [x] Sheet fit, Editor footer, border/zone dividers, system font,
      standard title block; PgUp/PgDn.
- [x] Preserve original wire coordinates; physical-pixel stroke width/alignment
      for axis-aligned lines belongs only to the renderer.
- [ ] Confirm exact pixel parity for every fragment and Linux GUI;
      remaining differences/observations: docs/interface/visual-parity.md.

Partial implementation of original-plan §5 and §9–10; original editing/native
roundtrip criteria remain unmet. PCB import deferred at user request.
Compatibility boundaries: docs/altium-import.md.

### Basic schematic editing — October 20, 2026

- [x] Typed eda-core Insert/Delete/Transform/Properties/Text/Width/Batch commands;
      UUID/revision checks, atomic validation, Undo/Redo.
- [x] New schematic scene, R/C, wire, net label, text, GND, junction, graphic line;
      preview, confirmation and Esc without committing unfinished actions.
- [x] Single selection, mouse/command Move, component-group rotation;
      Copy/Cut/Paste/Duplicate replacing UUIDs/owners/fonts.
- [x] Properties: Designator/Comment, X/Y in mm/mil, text, line/wire width.
- [x] Snapping to grid, electrical pin ends, GND/junctions and wires.
- [x] Original JSON retains edits; source_modified marks the historical native
      archive. Original records/bytes unchanged.
- [x] Windows GUI: R1 placement, 10k→47k, Move/drag/rotate/Undo/Redo, Ctrl+C/V,
      pin-to-pin wire, save/restart/open; 27 tests, warning-free fmt/clippy,
      Linux GNU all-targets compilation.
- [ ] Group selection, wire vertices, stretching, automatic junctions,
      libraries/annotation/hierarchy/compiler/ERC and native writer.
- [ ] 10 pm native roundtrip, durable journal, operation ID/idempotence,
      crash recovery and Linux GUI verification.

Observed Place/Drawing Tools in open Altium; menu order preserved, unsupported
commands disabled. Part…/Active Bar/Properties remain partial.
Contract/controls: docs/schematic-editing.md, ADR 0003; differences: features.json.

Typed commands/units, identity, transactions, preview/cancel, revisions, format
migrations and recovery. Primitives, spatial index, hit testing, snapping and
geometry editing. Check native Altium containers on minimal fixtures before
promising compatibility.

Criterion: create, place, edit, move, undo, redo, save and reopen real objects
through GUI and headless commands.

## Stage 3 — libraries and schematics

### Priority schematic Properties iteration, October 20, 2026 — in progress

User condition: start Components and IntLib import/placement only after full
Properties parity. Not yet met; this iteration did not implement libraries.
User-specified ReferenceB.IntLib remains input for the next work.

- [x] Observe Document General/Parameters, Component General/Pins,
      Parameter/Designator, Text, Net Label, Power Port, Wire, Text Frame,
      Rectangle and Port in AD 24.4.1. Map: docs/interface/schematic-properties.md.
- [x] Separate context forms, 12 selection filters, original Comment,
      hidden parameters, visibility/name display, font family/size/styles,
      colors, coordinates/rotation, sheet settings and vertex table.
- [x] SchDoc model metadata, model/parameter/link tables, favorites and Show More;
      reads existing schematic instances, not library import or Components implementation.
- [x] Validated model commands/history; new data saved in original JSON without
      source-archive changes. Old documents read through defaults.
- [x] Regressions: dock width with long names/values; hidden/raw parameters,
      models, separate fonts, atomic rejection and history.
- [ ] Full visual comparison of all contexts, mixed selection, placement,
      special objects, templates, advanced fields, compiled net metadata.
- [ ] Complete exact dimensions/icons/typography and check Linux GUI.
- [ ] Then Components, SchLib/IntLib and library-component placement.

Symbols/pins, footprints/pads, components, wires, labels, junctions, ports,
connectivity compiler, ERC, hierarchy and annotation.
Investigate corresponding Altium editors separately.

Criterion: verification schematic with independently checked netlist/ERC.

## Stage 4 — PCB and first manufacturing workflow

Layers/stack, components, pads/vias/tracks/arcs/regions, nets/ratsnest,
ECO, rules and DRC. Manual routing → walkaround → shove,
pours with thermals. Gerber, Excellon, BOM, pick-and-place.

Criterion: original schematic → ECO → board → DRC → outputs;
independent reader checks resulting manufacturing files.

## Stage 5 — advanced subsystems

HDI/rigid-flex, differential pairs/tuning, high-speed/xSignals, 3D/STEP,
variants/BOM, simulation/SI/PI, OutJob, CAM, Draftsman, multi-board, harness,
Workspace server. Each subsystem gets its own observations, model, operations,
files and criteria.

## Stage 6 — parity and performance

Continuous feature/difference registry. Windows/Linux CI, DPI/input,
small/medium/large fixtures, release benchmarks, latency and memory.
Do not promise schedules/performance without measurements.

### User priority: Components and libraries — October 20, 2026

New explicit instruction “Start components and libraries” authorizes this stage
before full Properties completion. The previous conditional sequence above
remains as history; unresolved Properties differences remain.

- [x] Observe Components AD 24.4.1 with ReferenceB.IntLib: library selection,
      search, table/Results, Details/Symbol/Model and menus.
- [x] Independent SchLib Binary 5.0 and embedded IntLib SchLib reading;
      31 supplied-library components, part/display variants, binary pins,
      fractional coordinates, UTF-16, parameters, fonts and resources.
- [x] Components panel, local library installation, path-list persistence,
      local search, details, preview and placement. Existing SchLib/IntLib
      PrjPcb references are read and available in the tree.
- [x] Independent instances: UUID, free designator, rotation, repeated placement,
      Esc, revision check, history, original-format saving/loading.
- [x] Import/corruption/limit regressions, multiple parts/modes, resources,
      saving, hidden-parameter preview, placement and history.
- [ ] Full Components visual parity, drag-and-drop, connection update/removal,
      library editor and instance updates.
- [ ] Multipart packing/annotation, managed/DbLib/SVNDbLib,
      footprint/3D (PCB stage), Linux GUI verification.

Observation map/boundaries: docs/interface/components-libraries.md.

### Components: visual comparison and drag-and-drop — October 20, 2026

- [x] Reobserve IntLib in AD 24.4.1: Models with white card, raw Comment,
      LMV321 electrical markers, References and lower sections;
      clarify differences from earlier Symbol/Model state.
- [x] Table/Details/Models geometry at dock 316; column sorting/widths,
      divider, active filter, favorites and Show More/Less.
- [x] Preview with original gray expressions, hidden parameters, portable fonts,
      fitted scale and input/output markers.
- [x] Row/card drag onto canvas: preview, one atomic insertion, snap, cancellation,
      session/revision guard, Undo/Redo; no Windows-only API.
- [x] Windows GUI: 1396/7086, drag C1/C2, Undo/Redo and save/reopen on original
      QA schematic. 43 workspace tests; Linux GNU all-targets check.
- [ ] Full pixel compatibility for all symbols/modes/DPI, remaining pin markers,
      native dialogs and Linux GUI. Differences explicitly listed in
      docs/features.json and docs/interface/components-libraries.md.

### Pre-publication checkpoint — October 20, 2026

At the user's explicit instruction, development paused; current state is
committed/published to PCBCraft through remote upstream.

- Draft connected Drag in eda-core: component/connected-wire changes in one
  validated transaction, orthogonal bends, fixed branches/labels, shared nodes,
  groups and rotation.
- Separate Drag/Move, M → D / M → M menus, command object picking, cancellation
  on Esc/focus loss/leaving canvas/tab change; preview hides original geometry
  of modified objects. Move retains connection-breaking behavior.
- Observed Edit → Move menus and native R9 body Drag on a separate ReferenceB
  copy; original user project not edited for research.
- 52 workspace tests, fmt --check, Clippy -D warnings, Linux GNU all-targets check
  passed. Updated PCBCraft has not yet been checked in a real window.
- Resume: verify GUI on ReferenceB, angles/rotation/groups and save/reopen;
  complete wire-segment drag, autojunction/merging and cleanup.
  Full parity with every Altium editing scenario is not yet claimed.

### PCB v2 integration — October 21, 2026

At user request, integrated the cumulative PCB package based on bd76173:
PCB model/commands, PCB6/ASCII import, manual routing, editable polygons,
Properties and experimental native saving with backup/reimport validation.
Fixed compilation/Clippy errors, updated lockfile. 109 Rust tests, fmt, strict
Clippy and Linux all-targets build passed. Linux GUI: Properties Apply, repour,
route/Undo/Redo, Save with backup and reopen.
Details/limitations: docs/pcb-update/VALIDATION.md and IMPLEMENTATION.md.
Altium interoperability, Windows GUI, real boards, DRC/shove/ECO/fabrication and
full visual parity remain unverified or unimplemented.

### Schematic text during zoom — October 21, 2026

Fixed user-reported text metric/anchor jumps during zoom: fixed glyph layout,
camera scaling and subpixel-position preservation; text blocks retain wrapping.
Added two real-tessellation regressions that fail before the fix; 113 tests,
fmt, Clippy and build pass. Linux GUI and canvas equality after zoom in/out checked.
Details/boundaries: docs/verification.md and docs/features.json.

### Editing memory growth — October 21, 2026

Reproduced 2.49 GB RSS after 200 visibility edits of a native TI schematic.
Removed source/records/images duplication between snapshots, limited Undo/Redo
(128 steps / 64 MiB estimated editable snapshots per document), cleared stale
textures/PCB triangulations. Font slots reused, clipboard excludes unrelated
images, deletion releases unused image resources.
Eight memory probes finished without unbounded growth; 600 visibility iterations
stay around 31 MB RSS in the model probe, GUI 200 toggles around 159 MB.
122 tests, fmt, strict Clippy and all-targets build pass. Details: docs/memory.md.

### Resumed schematic editing — October 21, 2026

- [x] Compared native wire Drag in AD 24.4.1 on a separate ReferenceB copy:
      fixed connections, bends and stretching T-branches. Change undone;
      user project not edited.
- [x] `Edit::DragWire`: segments/vertices, shared preview/commit geometry,
      UUID/style preservation, revision checks and atomic Undo/Redo.
- [x] Wire-handle markers; direct mouse drag and M,D, cancellation outside
      canvas/on Esc/focus loss/tab change. M,M retains connection-breaking movement.
- [x] Automatic T/pin connection points from local geometry including preview.
      Interior crossings without nodes do not connect; manual Junctions retained,
      cache bounded by View lifetime.
- [x] Backspace/Delete removes the last wiring vertex; clicking an existing
      connection finishes the wire as one transaction and readies the next wire.
- [x] Windows GUI: connected R1 Drag, segment Drag with T-branch, Undo/Redo,
      original-schematic JSON Save/reopen. 131 workspace tests, fmt, strict Clippy;
      Linux GNU all-targets check without a Linux build release.
- [ ] Full ReferenceB comparison, all angle modes, wire-group drag, segment
      deletion, overlap cleanup, net-change warnings, compiler/ERC and new Linux
      GUI verification. Boundaries: docs/schematic-editing.md/features.json.

### Group operations and Wire segments — October 21, 2026

- [x] Observed green Wire markers and red repeat-selection segment markers in
      AD 24.4.1 on a ReferenceB copy. Native Delete/all preferences not reproduced
      separately; official documentation used.
- [x] Mixed component/wire/segment groups: Move, connected Drag, rotation,
      Copy/Cut/Paste/Duplicate/Delete; one history step, revision/primitive-owner
      checks; unused designators within groups.
- [x] Segment selection between bends/electrical nodes, including Shift.
      Deletion preserves other portions; changed topology rejects stale selection.
      Automatic junctions update.
- [x] Local overlap merging preserves style, T-nodes and ordinary unconnected
      crossings. Preview/commit share a command.
- [x] 145 workspace tests, fmt --check, strict Clippy, Windows debug build;
      Linux GNU all-targets check without a Linux release. Real window:
      segment selection/Drag, overlapping Duplicate, Undo/Redo, original QA-scene
      JSON Save/reopen. Mixed groups/Shift checked through egui events.
- [ ] Rectangle selection, bulk Properties, all angle modes/locked semantics,
      global cleanup/preferences, net-change warnings, compiler/ERC, native
      SchDoc writer, full Altium comparison and new Linux GUI verification.
      The earlier unfinished-work list above refers to the previous iteration.

### ReferenceB PCB opening fix — October 21, 2026

- [x] Reproduced `Invalid PCB Pad geometry` on project `ReferenceBBoard.PcbDoc`:
      NPTH Pad 0 with zero copper and drill ~0.45 mm.
- [x] Supported this object without invented copper: geometry validation,
      bounds/picking, outline display and physical cutout during repour.
- [x] Binary regression fixture, JSON/native roundtrip after movement,
      unknown-tail/archive preservation; invalid zero Pads rejected.
- [x] Windows GUI: PcbDoc double-click from project, 15389 primitives,
      69 components. User file unchanged; old unsaved window left open.
- [x] 148 workspace tests, fmt --check, strict Clippy, Windows build,
      Linux GNU all-targets check. Details: docs/pcb-update/VALIDATION.md.

### PCB display against open Altium — October 21, 2026

- [x] Compared current board, Top/Bottom palette and Object Visibility in
      AD 24.4.1; file settings separated from application-wide palette.
- [x] Modern layer identities/V9 names, layer visibility/opacity/snap grid,
      NAMEON/COMMENTON, font kind/bold/italic, mirroring and stable text zoom.
- [x] ComponentBodies 2D projection import, cutout/body hatching; paired legacy
      Regions/Bodies retained without double rendering (57 + 62 pairs).
- [x] Contour-arc traversal direction, seams without huge miter triangles;
      individual Pad radii for renderer/pour/picking; drilling pass.
- [x] 159 tests, fmt --check, strict Clippy, Windows build, Linux GNU all-targets
      check. Source unchanged; no new Linux artifacts built.
- [ ] Full visual parity: stroke fonts/embedded glyphs, text styles, via-span/
      adaptive labels, exact raster/hatch and PCB Properties/tools/layer bar.
      Observations, implementation/boundaries: docs/pcb-update/VISUAL_PARITY.md.

### PCB Properties — October 21, 2026

- [x] Observed AD 24.4.1 Board General/Parameters, Component, Track, Pad,
      Via, Text, Polygon; contextual layout replaces generic Apply editor.
- [x] Supported fields through commands/History, origin-relative coordinates,
      corrected Texts6 Comment, dirty/Save/Close and independent drafts.
- [x] Filters/implemented snaps connected to PCB canvas; Shift+E.
- [x] 316 px layout regressions for all contexts, mm/mils Enter/Escape,
      stale/invalid drafts, Undo/Redo/Save; Windows via 0.7 → 0.8 → Undo/Redo/Undo.
- [x] 169 workspace tests, fmt --check, strict Clippy, Windows debug build,
      Linux GNU all-targets check without Linux artifact build.
- [ ] Full pixel parity, 3D padstack/Full Stack/templates, masks/rules,
      component area/opacity, advanced text and mixed Properties.
      Status/limitations: docs/pcb-update/PROPERTIES.md.

### PCB Properties: padstack, 3D and masks — October 21, 2026

- [x] Compared Pad Stack modes and Copper/Hole/Paste/Solder rows,
      Slot/offset and Top/Bottom tenting on a separate AD 24.4.1 copy.
- [x] Typed per-layer eda-core stack, shared picking/render/pour geometry,
      Round/Square/Slot, offset, Flip; PCB6 read/write and opaque-tail retention.
- [x] Interactive eda-render 3D: surfaces/holes/barrel/masks, rotate/zoom/reset
      without document changes; unified stack editor in Properties.
- [x] Manual/Rule masks, priority/bounded scope evaluator, tenting/percentage/
      hole edge; existing-mask rule editor through History and Rules6 writing.
- [x] PCBCraft-written real-board copy with changed masks opened in Altium;
      Top Tented confirmed with Bottom untented retained. Source unchanged.
- [x] 188 tests, fmt --check, strict Clippy, Windows build, Linux GNU all-targets
      check; no separate Linux release. Details: docs/pcb-update/PROPERTIES.md.
- [ ] Full pixel/material parity, templates/custom apertures/thermal relief,
      modern native independent masks, plane stack and native-reference remapping
      after repour. Native Save continues to refuse unsupported remapping.

### PCB continuation: Pad Edges — October 22, 2026

- [x] Fetched origin/main updates to e8f7ed7 without conflicts.
- [x] Enabled Pad Edges in existing PCB Snap Options from recorded AD 24.4.1
      observation; snap settings do not change document/history.
- [x] Projection onto actual outer copper boundary with shape, corner radii,
      rotation, Copper Offset, padstack and visible layers; analytical arcs.
- [x] Geometry/radius/layer/switch regressions and real PCB canvas events,
      including Ctrl+Shift bypass. Boundaries: pcb-update/PROPERTIES.md.
- [ ] New live Altium comparison of snap priorities/custom shapes/all DPI;
      other unfinished PCB stages above remain.

193 workspace tests, fmt, strict Clippy and Linux debug build verified.
Linux GUI: Pad Edges toggle switches edge/grid snapping without dirty state.
TI PMP23595 imports; PMP22650 remains blocked by padstack validation.
No new live Altium comparison yet.

### PCB controls — October 22, 2026

Fixed separate PCB input path after user report: Ctrl+wheel handles egui
zoom_delta; pinch/PgUp/PgDn preserve pointer anchor; right/middle pan,
fit document/board/selection and Home. Small 90 replaces unconditional full
crosshairs, with persisted cursor/wheel settings. Fixed fast drag-release,
M,M/M,D command object picking, gesture cancellation on focus loss/tab switching,
grid-off drag, rotation during mouse move/paste. Added event/history regressions.
New live AD 24.4.1 comparison unavailable; full parity not claimed.
Controls/boundaries: docs/pcb-update/CONTROLS.md.

Controls validation: 210 workspace tests, fmt and strict Clippy. Real Linux GUI
checked Ctrl-wheel roundtrip, pan, V,F, move + Space, Undo/Redo, Esc and exact
PcbDoc coordinate/angle persistence; Ctrl+W adds a conductor with expected net/ends.
Fixed discovered ScrollArea conflict between two component Descriptions.
Test boards remain outside the repository. Cancellation also suppresses same-frame
mouse press/release events until release; added a regression for this event race.
Locally built PCBCraft-0.1.0-controls-20261022-x86_64.AppImage (x86_64,
glibc >= 2.39, outside Git). Package launch, zoom, component selection, absence
of Description conflict and Esc + mouse release checked with byte-identical
saved PcbDoc.

### Schematic/board Cross Probe — October 22, 2026

- [x] Tools > Cross Probe / T,C, persistent mode, Ctrl+click for a one-shot jump;
      Esc/right-click ends mode.
- [x] Components by native ID with designator fallback, pins/pads by number;
      nets by wire/pin connectivity, junctions and labels.
- [x] PrjPcb search scope, opposite-document opening on demand,
      choice among multiple tabs and ambiguity messages.
- [x] View state only: centering, layers, selection/highlight;
      model, dirty state and Undo unchanged.
- [x] Automated tests/Linux GUI for both directions, pin and net.
- [ ] Hierarchical-net compilation, harness/bus/variants and split-view;
      exact live behavior/scale comparison with AD 24.4.1.

Description/boundaries: docs/pcb-update/CROSS_PROBE.md. Test project uses only
original fixtures; TI files remain outside Git.
Local TI PMP23595: 164/167 components match; explicit physical-instance selection
added for 36 repeated-channel components. Native SOURCEUNIQUEID accepts both
path separators, / and \.
Repeated nets without an unambiguous PCB net remain unsupported;
name matching does not substitute for a hierarchical compiler.
Final: 219 workspace tests, fmt, strict Clippy and debug build pass.
Linux GUI also checked physical-instance selection CHANNEL1/CHANNEL2.

### PCB 3D Layout Mode — October 22, 2026

- [x] Keys 3/2 and View menu, separate camera, orbit/pan/zoom, Top/Bottom/Fit.
- [x] Volumetric board with thickness, through-holes/cutouts and outer-layer
      geometry; body projections with height/standoff and pad fallback.
- [x] Depth picking, selection/Properties, rotation about component origin,
      Undo, Cross Probe and component-visibility switch.
- [x] Scene/texture cache, model immutability during navigation, fast-gesture/
      occlusion/2D-return/pending-edit regressions.
- [ ] STEP/Parasolid meshes, exact materials/masks/overlay, collision,
      rigid-flex and complete live AD comparison. Bodies explicitly simplified.

Description/limitations: docs/pcb-update/3D_VIEW.md.

3D checks: 228 workspace tests, fmt and strict Clippy pass. Local release
AppImage checked on TI PMP23595: selection/Properties, orbit/zoom, bottom view
with through-holes and return to 2D. TI files outside Git.

### Layer Stack Manager — October 22, 2026

Previous 3D changes committed separately as eb513d9.

- [x] Design > Layer Stack Manager, Stackup/Via Types/Impedance, Properties,
      cross-section, mm/mil, Apply/Discard and atomic Undo.
- [x] Edit/add/reorder layers, materials, Dk/Df, thicknesses;
      stable copper IDs and removal reference checks.
- [x] Through/Blind-Buried/Micro presets with spans, diameter/hole,
      used during via placement.
- [x] Single/differential impedance profiles, analytical estimate,
      width solving and single-ended width application to routing.
- [x] JSON persistence, original archive preservation, consistent 3D thickness.
- [ ] Native stack serialization/custom-data import, field solver,
      rigid-flex/backdrill, auto symmetry, native rules and exact live AD comparison.

Detailed limitations: docs/pcb-update/LAYER_STACK_MANAGER.md.

LSM checks: 236 workspace tests, fmt, strict Clippy and debug build. Linux GUI:
layers/via/50 Ω impedance/Apply/Undo; TI PMP23595 — original 6 layers/materials,
2.2082 mm; open/Apply without changes stays clean.

### Schematic Annotation — October 22, 2026

- [x] Tools > Annotation > Annotate Schematics..., Annotate and separate ECO.
- [x] Project sheets/selection scope, traversal/sheet order, Start Index,
      locked-number preservation, reset/duplicates and preview.
- [x] Shared multipart package number, compatible new-part packing,
      collision/stale-project guards and all-sheet validation before Apply.
- [x] Visible/hidden text update, unchanged Unique IDs, per-sheet Undo.
- [ ] Live AD comparison, compiled channels, advanced sorting/part swapping,
      back annotation/quiet mode/native SchDoc writer and project-wide Undo.

Behavior/limitations: docs/ANNOTATION.md.

Annotation checks: 247 workspace tests, fmt and strict Clippy. Linux GUI:
menu/preview/ECO/Undo/Redo/JSON; TI PMP23595 — 5 sheets, 167 components,
existing-number preservation and full-renumbering Validate without Apply.

### PCB View Configuration and connections — 22.10.2026

- [x] Previous schematic annotation committed and pushed to main: 84951bf.
- [x] Right dock View Configuration, L/Panels/menu/layer-bar entry points.
- [x] Layer visibility/colors, physical ordering, object transparency, labels,
      single-layer dim/hide, grid/system colors and basic 3D view controls.
- [x] Physical copper islands and minimum spanning connection guides; native
      polygon NET inheritance, plated/via spans, holes and stale pour handling.
- [x] Bounded background calculation/cache with stale-result rejection.
- [ ] Exact AD visual/functional verification: official docs blocked by proxy;
      native profile persistence, draft objects, via span markers and full 3D
      settings remain incomplete. See docs/pcb-update/VIEW_CONFIGURATION.md.

Validation: 255 workspace tests; fmt, strict Clippy, debug build. Linux GUI:
fixture connection lines and view toggles; TI PMP23595 Layers & Colors and
3D View Options. No TI project files committed or modified.

### View Configuration — live AD 24.4.1 comparison, October 22, 2026

- [x] Compared Layers & Colors, System Colors, 2D/3D View Options,
      Object Visibility and Section View on a research PcbDoc copy.
- [x] Left panel 330 px, independent right Properties, tabs/rows/spacing,
      mechanical-layer role/pair tree, Used On and configured empty layers.
- [x] Independent visibility/draft/opacity, Apply/Cancel matrix, colors,
      mirrored bottom view, labels/connection modes; fast mouse gestures.
- [x] Native 3D colors/opacity/background, perspective, thickness scale,
      materials/transparency, independent single-layer and dim/highlight.
- [ ] Full parity: profiles/import/export, Section View, native color picker,
      docking/autohide, categories without model/renderer, text draft, via-span,
      native range/parent matrix editing and exact Linux states/metrics.

Status remains partial; disabled elements do not count as implementation.
Boundaries/checks: docs/pcb-update/VIEW_CONFIGURATION.md, docs/features.json.
Checks: 267 workspace tests, fmt --check, strict Clippy, Windows debug build,
Linux GNU all-targets cross-check. Linux GUI/artifacts not run/built.

### PCB editor performance — 22.10.2026

- [x] View Configuration committed separately: 3402caa.
- [x] Read-only CPU probe on the 15,513-object research PcbDoc: original warm
      2D frame ~150 ms, first synchronous 3D frame ~14.8 seconds.
- [x] Revision/preview-bound geometry caches, broad-phase picking, shared
      previews and generation-based connection results; linear selection lookup.
- [x] Bounded background scene/image preparation, stale-result rejection,
      matching depth-buffer picking, cached triangle sort inputs.
- [x] Optimized normal dev profile with debug symbols/assertions retained.
- [x] Updated warm 2D CPU paint+tessellation ~22–23 ms; first 3D image ~0.87 s
      in background, UI request ~10 ms and subsequent waiting frames <1 ms.
- [x] Windows GUI: Fit Board, via selection/move/Undo, scroll, 2D/3D zoom/switch.
- [x] 273 workspace tests, fmt --check, strict Clippy, Windows dev build and
      Linux GNU all-targets cross-check; no Linux distributable built.
- [ ] Continuous 60 FPS software 3D, large-board repour/model-copy costs and
      Linux native UI/performance verification remain open.

Measurements, method and limits: docs/pcb-update/PERFORMANCE.md.

### PCB release latency — 22.10.2026

- [x] Isolated synchronous auto-repour as the release stall: ~1.4 s on the
      15,513-object / three-polygon board.
- [x] Commit position immediately, reuse matching preview, avoid cloning the
      replaced board; measured ready-preview commit ~9 ms + worker snapshot ~11 ms.
- [x] One background repour per document; stale results rejected, newer edits
      coalesced, application deferred during gestures/Properties/model dialogs.
- [x] Typed derived-geometry update through core History: validation/revision
      checks, one Undo per move, Redo preserved, saved-content state retained.
- [x] 280 tests, fmt --check, strict Clippy, Windows GUI via move/Undo/Redo,
      Windows dev build and Linux all-target cross-check.
- [ ] Full Altium pour algorithm parity and native Linux latency measurements;
      explicit repour/native save rebuilds remain synchronous.


### Document tabs and editor bottom strips - 23.10.2026

- [x] Observe live AD 24.4.1 PCB/SCH strips on the original ReferenceA project.
- [x] Shared 26 px document tabs, file icons, Home Page, guarded context close.
- [x] 22 px PCB layer strip, swatches, copper ordinals, mechanical order, LS and scroll controls.
- [x] Schematic Editor/sheet strip; unsupported compiled view disabled.
- [x] Windows GUI switching documents, active copper layer, LS and scrolling; view-only catalog regressions.
- [x] 288 workspace tests, fmt check, strict Clippy and Windows dev build.
- [ ] Exact icon/raster/overflow/DPI parity, custom layer sets, batch close and split windows.

Observation and limitations: docs/interface/document-bars.md.

### Projects panel — 23.10.2026

- [x] Observe live AD 24.4.1 ReferenceA tree, Settings/Output Jobs, Generated, variants and nested sheets.
- [x] Shared 554 px dock, 16 px tree rows, source file icons and sheet numbering.
- [x] Source/generated metadata separation, variants/BOM flags and best-effort sheet references.
- [x] Stable expansion/selection, independent search, guarded document activation/close.
- [x] Fix zero-sized startup collapsing Projects and retain width across PCB/SCH.
- [x] Windows GUI on ReferenceA, 292 workspace tests, fmt check, strict Clippy and dev build.
- [ ] Exact raster/DPI parity, native status/menus, docking and compiled project behavior.

Observation and limitations: docs/interface/projects-panel.md.

### Interactive Routing — 23.10.2026

- [x] Observe live AD 24.4.1 routing panel, Ctrl+W, start/Tab/Escape and conflict list on a separate ReferenceA PCB copy.
- [x] Independent core Ignore/Stop/bounded Walkaround; analytic track clearance and bounded arc approximation.
- [x] Atomic via-span checks, Backspace restoring layer/cache, preview/commit geometry agreement.
- [x] Prevent blocked targets from automatically finishing; preserve pours for empty previews.
- [x] Native-style routing Properties sections, initial unconditional width/via rule values, Shift+R and keypad layer switching.
- [x] Windows GUI on NetDA1_34: route leg, via, backtrack, mode, pause, finish and Undo; original source hashes unchanged.
- [x] 301 workspace tests, strict Clippy, fmt check and Windows dev build.
- [ ] Full parity: scoped/matrix design rules, shove/HugNPush, arc corners, gloss/loop removal, autocomplete and exact UI/DPI/Linux behavior.

Observation, timing and limitations: docs/pcb-update/INTERACTIVE_ROUTING.md.

### PCB component selection by mounting side — 23.10.2026

- [x] Fix opposite-side component primitives intercepting selection on Top/Bottom; use owner side even for multilayer pads and respect owner locks.
- [x] Share filtering between click, drag, box selection and Select All; prioritize active-layer geometry in hit ordering.
- [x] Regression coverage for overlapping top/bottom components through real canvas pointer events and editor box/drag paths.
- [x] Windows ReferenceA GUI: DA1 selects on Top, G1 on Bottom; selecting DA1's location on Bottom no longer selects DA1. Source PCB not saved or modified.
- [x] Workspace tests, fmt check, strict Clippy and Windows dev build pass.
- Scope: active outer copper layers in component selection mode; full Altium selection preference equivalence remains unverified.

### Layer Stack Manager restoration — 23.10.2026

- [x] Commit and push preceding work: 83f2b7c on origin/main.
- [x] Review existing manager and official behavior documentation.
- [x] Implement independent symmetric material edits, paired layer operations and atomic invalid-operation rejection.
- [x] Add plane insertion, isolated draft Undo/Redo and stack-specific Save/Close command handling.
- [x] 310 workspace tests, fmt check, strict Clippy and Windows dev build; original ReferenceA hashes unchanged.
- [x] Live AD 24.4.1 reference after unlock: Stackup, dielectric Properties, Via Types and empty Impedance on original ReferenceA without edits.
- [x] Colored stack grid, imported overlay rows, total thickness including masks, stack/board tab navigation and contextual menus.
- [x] Real ReferenceA window: symmetry, paired insertion, Undo, PCB/Stackup switching with retained draft; 311 workspace tests, strict Clippy, fmt and Windows build pass.
- [ ] Final pixel parity and advanced Layer Stack Manager coverage remain incomplete.
- [ ] Full restoration remains open: document tabs/workspace behavior, exact layout, material library, native stack interoperability, whole-stack mirroring, backdrill, rigid-flex and solver/rule bindings.

Evidence and limits: docs/pcb-update/LAYER_STACK_MANAGER.md.

- 2026-10-23: Layer Stack Properties follow-up: observed copper/mask fields in AD; added persistent fabrication metadata, type-dependent property panels, collapsible sections and field search; constrained via spans and impedance references. Advanced native import, roughness and populated-profile parity remain partial.

- 2026-10-23: Committed and pushed all Layer Stack work as 91a4110. PCB cursor fix: draw the crosshair at the raw screen pointer, independent of snapped tool coordinates; add rendered-cursor regression for motion inside one grid cell and mirrored view.


### PCB Rules and DRC — 23.10.2026

- [x] Observe live AD 24.4.1 Rules and Design Rule Checker on ReferenceA without saving reference changes.
- [x] Native-field rule drafts, query/priority/matrix resolution, atomic history and safe native roundtrip.
- [x] Routing width/layer/via/clearance/outline integration and scoped polygon clearances.
- [x] Background online/batch checks, stale-result rejection, limits, violations and marker navigation.
- [x] Regression coverage for geometry, scopes, routing, rules/history, UI bounds and native preservation.
- [ ] Complete rule-family coverage and query language; native checker report/persistence and advanced routing behavior.
- [ ] Exact UI/DPI/Linux parity; Rule Wizard and Query Builder.

Evidence, validation and explicit remaining scope: docs/pcb-update/RULES.md.

- 2026-10-23 Rules follow-up: native Simple/Advanced polygon connections drive pours; added mask sliver and non-text silk/mask checks, shared cutout clearances, zero-edge crossing, independent routing violations, via manufacturing checks, HTML reports, per-file checker preferences and native new/copy priorities. Full rule-family/query/UI parity remains open; details and validation in docs/pcb-update/RULES.md.


### PCB Rules visual follow-up — 24.10.2026 (in progress)

- [x] Pull the configured upstream/main updates; no origin remote is configured.
- [x] Observe live AD 24.4.1 ReferenceB rule summaries, priorities, Query Builder/Helper,
  Wizard, checker and supported constraint forms; further exploration uses a copy.
- [x] Implement bounded query/wizard drafts, native catalogue order, summary pages,
  fixed dialog regions, per-layer preferred widths and imported numeric class members.
- [x] Fix numeric input buffering, child widget IDs, duplicate native rule identities
  and Unicode encoding shadows; original PCB hash remains unchanged.
- [x] Linux all-target compilation.
- [ ] Complete equal-size/DPI visual comparison and all related Rules surfaces.
- [x] 352 workspace tests, strict all-target Clippy, fmt check and Windows bins/examples build.
- [ ] Resume live comparison: after manual closure of the Panels popup,
  Computer Use reported a physical Escape stop. Latest drag/ID/Unicode GUI
  verification remains pending; no completion claim.

Evidence, exact implemented scope and remaining differences: docs/pcb-update/RULES.md.

### ReferenceB adaptive PCB 3D correction — 24.10.2026

- [x] Reproduce the user's ReferenceBBoard.PcbDoc in Windows and inspect the same
  source in AD Enterprise 24.4.1, including both sides, bodies, materials and
  native 3D View Control commands; source never saved in either editor.
- [x] Correct solder-mask/copper/paste depth and apertures; copper polygon
  cutouts no longer drill the laminate. Remove legacy duplicate geometry from 3D.
- [x] Read embedded STEP using independent Rust Truck/ruststep, preserve AP214
  face colours and share meshes. Native Y/Z mounting reflection fixes the large
  asymmetric bottom connector; validated move/rotate/flip follow history.
- [x] Render planar text, correct bottom camera/Flip, perspective depth,
  translucent shared edges and camera-space lighting. Add native-order camera
  menu commands; unsupported Fold/Unfold stays disabled.
- [x] Read-only reference audit: 16 models / 10,293 triangles / 62 placements,
  no missing meshes, maximum saved-projection XY bound error 0.000122 mm.
- [x] 361 workspace tests, strict all-target Clippy, formatting, Windows dev
  build and Linux GNU all-target compilation. Original source SHA-256 unchanged.
- [x] Live Windows top/bottom/flipped-isometric/model picking and locked-object
  protection checked after changes.
- [x] Unlocked bottom component Space/Undo in the live 3D window; native source
  never saved. Existing unsupported InPad repour query remains outside this fix.
- [ ] Exact pixel parity: native lighting/specular/antialiasing, font metrics,
  exact camera/Fit values, equal viewport/chrome/docks and Linux runtime GUI.
- [ ] Parasolid/textures/transformed assemblies, section/collision/rigid-flex
  and native writing of changed imported 3D placements remain unsupported.

Evidence, independent dependencies, regression coverage and explicit limitations:
docs/pcb-update/3D_VIEW.md and docs/pcb-update/THIRD_PARTY_NOTICES.md.

### ReferenceM PCB opening hang, 24.10.2026

- [x] Reproduce the reported 2D PCB opening hang with a memory-limited read-only
  release probe; GDB identifies unbounded STEP trimming-curve subdivision on NaN.
- [x] Reject non-finite converted surfaces before tessellation and bound boundary
  curve subdivision/evaluations. Preserve source archives and warn on fallback.
- [x] Accept native unassigned `POURINDEX=-1` using stable source order, while
  retaining the exact property and rejecting other invalid unsigned indices.
- [x] Regression tests, workspace tests, strict all-target Clippy and fmt check.
- [x] Unchanged ReferenceM: 41,475 items, load ~0.9 s, headless peak ~270 MiB;
  rebuilt AppImage rendered the board in the real Linux KDE/Wayland window.
- [ ] General STEP conversion coverage, complete resource isolation for arbitrary
  CAD inputs and full-frame large-board performance remain open.

Evidence and limits: docs/pcb-update/PERFORMANCE.md.

### ReferenceM 2D cursor and pan performance, 24.10.2026

- [x] Separate 2D paint/tessellation from loading and GUI document comparisons:
  ~92,000 shapes / 1.37M vertices per frame, ~11 ms per saved-content comparison.
- [x] Cache saved-content checks by History revision; explicitly refresh on Save
  and retain live pending-edit checks. Stop sending unchanged window titles.
- [x] Cache tessellated artwork and keep wgpu buffers resident for camera pans;
  preserve screen-anchored hatching, live tools, clipping and font-atlas ownership.
- [x] Spatial candidates for exact picking/snapping, including internal pad lands
  and original-board snapping during previews.
- [x] Real GPU image comparison against egui; cache/invalidation, preview,
  spatial snapping and saved-state regressions; fmt/Clippy/workspace checks.
- [x] Real Linux window: Fit Board, cursor movement and middle-button pan.
  Warm CPU frame medians ~6–11 ms / ~5–7 ms under concurrent CPU load;
  callback submission probe ~2.8 / ~2.5 ms. Rebuild the AppImage.
- [ ] Cache rebuild spikes on zoom/configuration/content changes,
  editing-preview costs, presentation FPS, Windows runtime and Altium parity.

Measurements and limitations: docs/pcb-update/PERFORMANCE.md.

### ReferenceM 3D orbit performance, 24.10.2026

- [x] Reproduce slow software refresh after an orbit change: ~782 ms on ReferenceM.
- [x] Resident wgpu 3D vertices/styles, hardware depth and cached image target;
  pan/zoom update the camera, opaque faces need no sorting. Sort translucent
  faces with stable linear radix passes on orientation/thickness changes.
- [x] Current-camera BVH ray picking, with perspective/flip/thickness and hidden
  material checks; retain selection of parent components and material styling.
- [x] GPU pixel comparisons vs software, buffer reuse, full-canvas handoff,
  stable depth ties and BVH-vs-raster regression checks.
- [x] Read-only completed-GPU probe on 2.18M triangles: orbit median 6.12/p95
  8.18 ms, pan 0.55/zoom 1.44 ms; native eframe automatic navigation probe.
- [x] Inspect the native application's own captured GPU frame; run 376 workspace
  tests, fmt/strict all-target Clippy and rebuild the Linux AppImage.
- [ ] First-scene latency (~3.7 s), arbitrary-board memory budgets, presentation
  FPS on other hardware/Windows and full native pixel/STEP support parity.

Measurements and limitations: docs/pcb-update/PERFORMANCE.md.

### ReferenceM intermittent pan stalls, 24.10.2026

- [x] Reproduce long-pan boundary stalls: 16/360 CPU frames >33 ms, maximum 158 ms.
- [x] Keep complete visible-layer artwork at a fixed document anchor; all camera
  translations reuse meshes/GPU buffers. Keep grid and origin live, with grid
  outline intersections computed per row. Cull offscreen hatch generation.
- [x] Invalidate glyphs on font-definition changes, rather than egui UI-atlas
  replacement; preserve the artwork's private atlas and DPI invalidation.
- [x] Regressions: offscreen objects revealed by long pans vs fresh GPU images,
  live grid/origin, atlas replacement/font changes and scanline boundary rules.
- [x] Release sweep at 15/150/1500 px/mm: no CPU frames >33 ms after initial build.
  Real XWayland long-pan batches: CPU medians ~1–2.7 ms, maximum 7.75 ms;
  no artwork rebuilds during short or long pan. Sampled GUI RSS ~672 MiB.
- [x] Workspace tests, strict all-target Clippy and formatting; rebuild and
  launch the AppImage. Package through a temporary file so a running old image
  does not prevent replacing the artifact.
- [ ] Cold scene construction on zoom/content/display changes, whole-scene
  memory at extreme zoom, editing previews, presentation FPS and Windows runtime.

Measurements and limitations: docs/pcb-update/PERFORMANCE.md.

### Oblique 3D transparency stripes, 24.10.2026

- [x] Reproduce incorrect blending of two tilted translucent sheets with
  opposite triangle diagonals; centroid ordering failed by 28/255 per channel.
- [x] Replace CPU triangle ordering with per-pixel front-to-back depth peeling,
  opaque depth rejection and RGBA16Float accumulation; retain resident geometry.
- [x] Reuse two depth targets and one layer image, with eight depth layers per
  pixel and a two-ULP separation between layers to avoid self-blending.
- [x] Check sheet ordering across triangle seams in orthographic/perspective
  cameras; retain material, opacity, selection, picking and cache regressions.
- [x] Capture and inspect ReferenceM at a fixed oblique angle in a native window;
  the large stripe/triangle patches disappear. Completed GPU orbit median 3.66/
  p95 4.20 ms, zoom median 5.07 ms on the local RX 7900 XTX.
- [x] Workspace tests, strict all-target Clippy/fmt and rebuilt Linux AppImage.
- [ ] More than eight translucent layers per pixel, software-fallback centroid
  transparency, antialiasing and full native pixel parity remain limited.

Measurements and limitations: docs/pcb-update/PERFORMANCE.md.

### PCB visible-layer cycling with mouse side buttons, 24.10.2026

- [x] Back/Forward presses over the focused 2D canvas select the previous/next
  visible layer in tab order, with wraparound and live visibility changes.
- [x] Skip system display categories; handle hidden active layers and empty or
  single-layer strips. Ignore releases, holds, panels and modal dialogs.
- [x] Send layer requests through the editor's existing route-layer handler,
  preserving via checks, previews and Undo. Normal switching only changes view.
- [x] Real egui input and controller regressions; native ReferenceM window with
  application-local synthetic pointer events and inspection of its GPU capture.
- [x] Formatting, strict workspace/all-target Clippy and 382 workspace tests.
- [x] Rebuild the Linux AppImage, verify its checksum and launch ReferenceM without
  renderer errors. Reference PCB/project hashes remain unchanged.
- [ ] Physical side-button input and matched Altium/Windows runtime comparison.

Controls and validation: docs/pcb-update/CONTROLS.md.

### ReferenceM routing startup failure, 24.10.2026

- [x] Reproduce route startup abort on `InLayerClass('L3_L4')`; inventory the
  board's clearance scopes, layer classes and native RoomDefinition contours.
- [x] Support layer-class membership, native layer aliases, straight room
  containment and InPoly/InPolygon; retain errors for unknown predicates and
  unsupported room geometry. Normalize native retraced room closing edges.
- [x] Resolve room-dependent clearance for the proposed segment, with cached
  obstacle geometry and conservative bounds covering matrix clearances.
- [x] Query/geometry and post-placement room/matrix regressions; read-only
  ReferenceM preview/place/finish/Undo probe on AR_QSPI_CS, 0.254 mm width.
- [x] Full native window with application-local synthetic mouse events: start,
  copper preview, placement/completion in one transaction and exact Undo;
  inspect the application's GPU screenshot. No source file writes.
- [x] Formatting, strict workspace/all-target Clippy and 389 workspace tests.
- [x] Rebuild the Linux AppImage, verify its checksum and launch ReferenceM without
  renderer errors; reference project/PCB hashes remain unchanged.
- [ ] Curved room scopes, complete query language/DRC, physical input and new
  Altium/Windows runtime comparison remain outside verified coverage.

Cause, implementation and validation: docs/pcb-update/INTERACTIVE_ROUTING.md.


### PCB 2D zoom stalls, 25.10.2026

- [x] Reproduced synchronous whole-scene rebuild on every zoom frame using
  read-only ReferenceM, 41,475 items. Parent baseline: zoom CPU mean 49.56 ms,
  p95 54.55/max 128.62 ms, 119/120 frames above 33 ms.
- [x] Pan/zoom transform resident buffers. Exact detail refines after 120 ms
  idle on one background worker with independent fonts/DPI/context and GPU
  preparation. Scale/content keys reject stale results; new content retains an
  old running job until it exits. Live overlays and current-camera picking stay
  active. Tests compare refined and fresh geometry, GPU scale pixels/buffer
  reuse, stale jobs, disconnection and content changes during refinement.
- [x] Release zoom CPU mean 7.62/p95 14.48/max 18.56 ms, zero >33 ms of 120;
  completed GPU median 2.46/p95 5.59/max 9.09 ms, refinement publication max
  6.29 ms. Native window zoom/settle and screenshot inspected; warm CPU median
  2.20/p95 13.38/max 51.60 ms, six >33 ms of 1530 under concurrent compilation.
  Source PCB hash unchanged. These are not presentation FPS.
- [x] `cargo fmt --all -- --check`, strict workspace/all-target Clippy and
  `cargo test --workspace` pass. Linux AppImage rebuilt, SHA256 sidecar checked,
  packaged app opened the source PCB in the real desktop session. Source hash
  remains unchanged.
- [ ] Cold content/display rebuilds, including incoming connection results,
  remain synchronous. Snapshot cost, arbitrary-board/extreme-zoom memory bounds,
  Windows runtime and exact Altium visual/performance parity remain open.
  Gesture antialiasing/labels use scaled previous detail until refinement.
  See `docs/pcb-update/PERFORMANCE.md` for reproduction and limits.


### PCB performance, October 25, 2026

- [x] Measured movement and 3D rebuilds on ReferenceA/ReferenceM.
- [x] Reusable validated move draft, immutable background snapshots,
  partial redraw and spatial-index updates.
- [x] Per-model-instance 3D transform preparation; base/mask/unaffected-object
  geometry caches, cutout index and accelerated BVH construction.
- [x] 398 tests, fmt, strict Clippy; real 2D/3D windows, captures/GPU readback;
  release build and updated Linux AppImage.
- [ ] First heavy frames and global base/mask rebuilds of large boards remain
  expensive. Sustained 60 FPS for every operation is not confirmed.

Measurements/check boundaries: `docs/pcb-update/PERFORMANCE.md`,
`docs/pcb-update/performance-2026-10-25.json`. Source boards unchanged.

### PCB track dragging, October 25, 2026

- [x] Fixed ordinary track drag previously breaking connections. M,D Drag and
  free-copper-track endpoint handles use the eda-core router.
- [x] Neighboring segments rebuilt; pad/via contacts, T-branches and straight-track
  crossings preserved. 45°/90° corners, obstacle avoidance, clearance/locked-object
  checks and atomic refusal.
- [x] Commit matches preview, one Undo/Redo operation; cancellation/errors remove
  draft. Preview/commit changes request repaint.
- [x] 414 workspace tests, fmt check, strict all-targets Clippy and dev build on
  pinned Rust 1.90.0. 16 new regressions.
- [x] Real Linux X11 window, primary-button drag without modifiers: a 20 mm track
  between two pads moved 5 mm; both connecting tracks visible in preview/after
  release. Save, Undo and exact Redo checked through JSON; pads unchanged.
  Used an original temporary document.
- [ ] Shove, gloss, loop removal, mixed groups, arbitrary arc contacts and Altium
  parity need further implementation/verification.

Details: `docs/pcb-update/INTERACTIVE_ROUTING.md`, `pcb.track_drag` in
`docs/features.json`. No new Altium observation or AppImage build.

### ReferenceM component selection/movement, October 25, 2026

- [x] Identified two causes on user ReferenceM: no body-center selection and
  whole-component blocking due to locked children. Of 37 such bodies, 31 parents
  are themselves unlocked.
- [x] Body selection by visible footprint bounds, independent Components filter,
  shared click/drag logic, immediate selection/Properties updates.
  Mounting-side selection retained.
- [x] Whole unlocked-component Move/Drag/Rotate/Flip/Delete preserves child lock
  flags. Independent locked-child editing and locked-parent movement remain
  prohibited. Locked components remain inspectable/explicitly unlockable.
- [x] 420 workspace tests, fmt, strict Clippy/all-targets on Rust 1.90.0.
  Read-only full-source-board probe: R189/U10/C62, selection and move/rotate.
- [x] Real AppImage window on an original U10 copy: body selection, drag,
  JSON Save, Undo/Redo; both locks and all child IDs preserved.
  AppImage updated, SHA256 checked. Source PcbDoc unsaved, hash unchanged.
- [ ] Exact arbitrary-outline selection, full Altium preference parity,
  Windows runtime and full-board drag latency remain unverified.

Details/check boundaries: `docs/pcb-update/CONTROLS.md`,
`pcb.component_selection_editing` in `docs/features.json`.

### Shared 2D/3D camera area, October 25, 2026

- [x] Following user preference, modes transfer board-area center/scale while
  retaining the chosen 3D angle. Pan/zoom transfer both ways.
- [x] Transfer waits for ready scene/actual viewport dimensions. Returning during
  preparation preserves source camera. Deferred Fit does not cross modes.
- [x] Repeated switching retains exact parameters without drift; panel resizing
  does not alter scale. Edge-on view preserves board anchor. Navigation does not
  change model/Undo.
- [x] 429 workspace tests, fmt, strict all-targets Clippy on Rust 1.90.0.
- [x] Real Linux AppImage window on separate U10 copy: pan/zoom, tilted 3D and five
  repeat switches. Returned 2D/tilted 3D match every pixel. AppImage updated,
  SHA256 checked. Source ReferenceM unchanged.
- [ ] Full Altium camera/projection parity and Windows runtime unverified.

Details: `docs/pcb-update/3D_VIEW.md`, `pcb.shared_camera` in `docs/features.json`.

### ReferenceB routing and obstacle index, October 25, 2026

- [x] Reproduced startup failure on full ReferenceB (15 513 objects): missing
  CompParameterValue/Like and HasPad; PrimitiveParameters were not read.
- [x] Base parameters imported separately from native metadata; owner/bounds/
  duplicate checks. Old JSON recovered by native UNIQUEID, parameters retained in
  model/JSON; unsupported native writing of changes refused.
- [x] CompParameterValue/Like, HasPad and wildcard HasFootprint; clearance-priority,
  error/unknown-condition checks. Found all 24 ReferenceB pads with ID=216.
- [x] Spatial index replaces repeated full obstacle scans; preserves order,
  room/matrix clearances, 1 nm tolerance and refusal on rule error.
- [x] Windows, 40 001 tracks: median Stop 4.0011 -> 0.0084 ms, Walkaround
  1.9503 -> 0.0056 ms. Preparation 12.234 -> 27.768 ms. CPU solver, not FPS.
- [x] 438 workspace tests, fmt --check, strict all-targets Clippy, Windows build.
  Real full-ReferenceB window: NetC13_2, segment, Finish, Undo to clean.
  Source PcbDoc unsaved, SHA256 unchanged.
- [ ] Copies/validation and route-preview graphics, costly large-pour distances,
  full rule language/variants, shove/gloss and exact Altium parity.
  Altium 24.4.1 PCB Filter returned a script error; live query parity unproven.

Data/boundaries: `docs/pcb-update/routing-performance-2026-10-25.json`,
`docs/pcb-update/INTERACTIVE_ROUTING.md`, `docs/pcb-update/PERFORMANCE.md`.

### Route preview without rebuilding the entire board, October 25, 2026

- [x] Private validated RouteSession draft; reusable immutable preview,
  stable active-segment IDs, new-geometry validation on hover.
  Full place/via/backtrack/History validation and retained-snapshot immutability.
- [x] Skip identical requests; separate connectivity geometry snapshot without
  retaining editor Arc or copying the whole board on every movement.
- [x] Source tree/contours retained, split tracks refitted; tail length/membership
  changes independently of stationary artwork. Source-board geometry reused at
  entry; foreign preview/revision invalidates cache.
- [x] Shared document anchor for live/cached tessellation removes a one-pixel shift.
  GPU readback linear/sRGB, masks/highlight/draft/mirror/single-layer/via/backtrack:
  <=1 channel level without dithering, <=3 with production dithering at overlaps.
- [x] ReferenceB, 60 CPU samples: median 57.5230 -> 5.7226 ms, p95 64.6967 -> 9.0993.
  Varying tails: 6.3729/9.2021 ms. First frame up to 90.7778 ms, 1/60 >33 ms.
  Repeat measurements after source reuse recorded separately; not presented FPS.
- [x] Final source reuse: 7.1928/9.6066 ms, max 72.3750 ms;
  varying tail: 7.6839/10.3159, max 62.3378 ms. Full redraw in the same final
  executable: 83.2955/103.6451, max 112.6602 ms. Between-run variation retained
  in data; cold frame still 1/60 >33 ms.
- [x] 449 workspace tests, fmt, strict all-targets Clippy, Windows build;
  Linux GNU all-targets compilation. Windows GUI: place/Backspace/Finish,
  Undo/Redo/Undo to clean; source PcbDoc unsaved, SHA256 unchanged.
- [ ] Cold artwork/snapping and place/Finish/repour delays, arbitrary large
  boards/long routes, Linux runtime and full Altium functionality/behavior/UI
  parity remain in the original scope.

Data: `docs/pcb-update/route-preview-performance-2026-10-25.json`.

### Shared source-board index for preview snapping, October 25, 2026

- [x] An Arc of the source spatial tree replaces a second full display cache.
  Source frame supplies the ready tree; preview refit detaches its copy without
  modifying original snap coordinates/indices.
- [x] Revision expiry, spatial-only fallback before the first source frame;
  Off skips object search but retains grid; Ctrl+Shift returns raw point.
- [x] Real canvas events: All/Current/Off, grid/bypass, cold/warm preview,
  source track endpoint and no self-preview snap. Multilayer pad, split refit,
  retained snapshots, fallback and revision-change checks.
- [x] 451 workspace tests, fmt, strict all-targets Clippy, Windows build,
  Linux GNU all-targets compilation. Windows ReferenceB: Off -> Current, NetC13_2,
  move/place/Finish/Undo to clean, restore Off. PcbDoc unsaved; SHA256 unchanged.
- [ ] Extra full-frame speedup is not proven: baseline median/p95/first
  4.8551/6.4533/51.1992 ms, after change 6.7688/9.2336/65.6447;
  varied 7.7865/10.9781/42.9343. CPU frequency/background scheduling uncontrolled,
  GUI states differ; all data retained. Cold artwork/model, final presentation
  latency and Altium accuracy remain open.

Data: `docs/pcb-update/route-snapping-performance-2026-10-25.json`.

### Reusing stationary artwork at route start, October 25, 2026

- [x] Board/layer/type/masks/details/polygon-outline groups retain drawing order.
  Source capture contains route-tail slots; stale copper/split source tracks
  rebuild affected groups; other mesh/atlas/GPU scenes are shared.
  Revision/style/font/DPI/scale checked.
- [x] Camera also updates retained GPU scenes without mesh zero; identical uniforms
  are not rewritten. CPU refinement remap runs once per shared mesh;
  weak-reference test verifies old GPU-resource release.
- [x] Full ReferenceB: 151/155 groups retained at stale transition; final artwork
  1.99/2.62 ms. Final combined regular median/p95/first 7.2775/10.5462/37.1903,
  varied 8.1933/10.3123/33.3280 ms. Early first 21.6751/22.0493 retained too;
  frequency/background uncontrolled; CPU submissions, excluding GPU/FPS.
- [x] 452 workspace tests, fmt, strict all-targets Clippy, Windows build,
  Linux GNU all-targets compilation. GPU linear/sRGB, pads/labels/masks/holes,
  split/stale/via/backtrack/highlight/draft/mirror, camera without mesh zero;
  <=1 channel level without dither, <=3 with production dither.
- [x] Windows GUI: Fit/refinement, NetC13_2 route motion/place/Backspace/place,
  Finish/Undo to clean. Entry 155/155 (3.52 ms), stale 151/155 (5.39 ms).
  Source PcbDoc unsaved, SHA256 unchanged.
- [ ] Finish/repour/Undo and global state changes still rebuild all groups;
  native-log artwork up to 67.69 ms. Cold model/validation, split groups, other
  boards, RAM/VRAM, Linux runtime and full original functionality/Altium parity open.

Data: `docs/pcb-update/artwork-groups-performance-2026-10-25.json`.

### 2026-10-26 — PCB Rules continuation after main update

- Continued from `9c60e1c`; existing Wizard/Query Builder and routing acceleration
  preserved. Working Hole Size Percent added: mode fields/summary, separate
  absolute/percentage values, range checks, shared DRC/interactive-via validation,
  atomic Apply/Undo, native PcbDoc roundtrip.
- Percentage basis for rectangular/per-layer-varying pads remains an explicitly
  marked implementation hypothesis (smaller nominal size on each layer).
  Without new AD comparison, full parity for these cases is not claimed.
- Hole checks cache geometry and cull distant pairs; comparison with exhaustive
  enumeration includes rotated slots, priorities and rule scopes.
- Profiling original ReferenceA found the true cause of 49-second DRC: three full
  overlay circles with a 1 nm inner radius. Generic buffer built each nearly
  8 seconds twice. Full circles now use direct rings/disks, retaining holes and
  256 segments; partial arcs keep the old path. Area, bounds, very thin rings/disks checked.
- ReferenceA: 49.7204 s before -> 1.0234 s after; repeat under Clippy/roundtrip load
  2.4197 s. All runs: 335 violations with identical per-type counts,
  22 diagnostics, no truncation. This does not prove every marker coordinate
  matches. Roundtrip: 3,331,072 bytes, all rules fields match.
- Original ReferenceA not written; SHA256 `redacted-source-sha256` preserved.
- No new PCBCraft instances launched. Real-window verification blocked by Computer
  Use error `window.app must be a non-empty string and window.id must be an integer >= 0`;
  reselecting the window did not help. New visual verification remains incomplete.
  Details/remaining differences: `docs/pcb-update/RULES.md`.
- Checks: fmt/strict Clippy passed; workspace 458 unit tests passed, 1 failed.
  All 7 new tests passed. Existing GPU-test failure
  `routing_tail_matches_full_gpu_pixels_and_keeps_buffers_across_bends_vias_and_backtrack`
  (Rgba8Unorm, frame 13, dither=true: 4 vs tolerance 3) also reproduced on
  unchanged `9c60e1c` in `target/rules-head-validation`. Tolerance not relaxed,
  renderer unchanged; a fully green workspace run is not claimed.

### 2026-10-26 — Rules continuation: Layer Pairs

- Reinspected Manufacturing → Layer Pairs in AD 24.4.1 on original ReferenceA:
  one scope field, Constraints and Enforce layer pairs settings, label left of
  checkbox. Source pair: LAYERPAIR0LOW=TOP / HIGH=BOTTOM.
- Implemented ENFORCE editing, summary, wizard and Online/Batch checking,
  shared DRC/interactive-via rule resolution. Physical spans checked in either
  direction; via size is a separate rule.
- Definitions come from edited typed stack, otherwise original legacy drill
  pairs. Empty table matches manager's existing default Thru. Unknown/incomplete
  tables and multiple substacks produce explicit diagnostics. Disallowed
  transitions leave route/backtrack unchanged.
- 5 new core tests passed. ReferenceA: 335 violations, 21 diagnostics, 1.0222 s,
  no truncation. ENFORCE roundtrip: 3,331,072 bytes, all rules equal after import.
- Windows build/strict Clippy passed; full workspace: 463 pass, 1 existing GPU
  dithering mismatch (same baseline 9c60e1c failure, tolerance unchanged).
- Real PCBCraft checked form, Enforce toggle and Cancel without applying.
  Sole instance closed before build update; one new instance launched.
  Original ReferenceA not saved.
- Full Rules parity not achieved: outer-dialog/font/icon metrics, native
  blind/buried via-type presentation/save, multiple substacks and technology
  metadata remain in RULES.md. Altium modal Cancel timed out twice; further
  Altium input stopped. The form itself was inspected successfully without edits.

## 2026-10-26 — Schematic ↔ PCB / ECO (in progress)

- Observed both Design commands on original ReferenceA in AD 24.4.1. Forward ECO:
  Add Components (12), Add Pins To Nets (22), separate Check/Done, missing
  footprint/pin errors on Validate. Reverse comparator: 47 differences,
  34 proposed ECO actions, first group Remove Pins From Nets (22).
  Execute/Save were not pressed in source Altium.
- Added independent eda-core design_sync: comparison, Unique ID, multipart,
  flat net compilation, real components/pads/footprints, metadata/nets, reverse
  properties and bijective net rename. Dependency/stale-source errors are not
  disguised as successful operations; successful actions have History/Undo.
- App: Design Import/Update, Project Show Differences, project-board selection,
  grouped ECO, Validate/Execute, error filter, report. Executed rows excluded
  from reapplication; remaining rows can be revalidated.
- Binary native writer extended with component base parameters, Unicode and
  source links with read-back; variant/system namespaces retained.
  Five targeted native parameter tests passed.
- Initial description tests exposed an incomplete fixture; description added.
  DisableSync test corrected to use a real hidden parameter, since a new-name
  ParameterValue creates a value without a presentation object.
- Scope/gaps listed in docs/pcb-update/DESIGN_SYNC.md; feature remains partial.
  Native SchDoc writer, footprint libraries, hierarchy, wire back annotation,
  classes/rooms/variants and exact UI parity unfinished.
- Final checks/real window: verified below after the current build.

- GUI comparison found egui table compression and missing ✓ font glyph.
  Fixed width retention, row height, backdrop and drawn Check/Done.
  First check executed 51 applicable in-memory operations; one Undo restored
  clean PCB. Source native file unsaved.
- New components now pack by actual footprint bounds beside the board;
  position is independent of already placed schematic-element count.
- BQ1 diagnosis found a 1 nm gap from separately rounding native position/pin
  length. Import now rounds the absolute electrical endpoint once and derives
  length_nm. Net membership does not use expanded tolerance.
  Test of 4 directions × 2 signs passed; artificial 1 nm gaps remain disconnected.
- After fixing, ReferenceA: 60 changes / 49 applicable, 1.3786 s; Add Components
  (12), Add Pins To Nets (22); false Remove Pins C2/C20/BQ1 disappeared. This is
  not equality with all 47 native comparator differences: comparison settings
  and footprint-geometry sources still differ. Copy of ReferenceA: 97 / 88, 2.3110 s.

- Final native-endpoint-fix checks: fmt --check, strict workspace/all-targets
  Clippy and Windows dev build passed. cargo test --workspace: 488 pass / 1 fail.
  Sole failure is baseline GPU routing pixel test, Rgba8Unorm/frame13/dither=true,
  max channel difference 4 vs tolerance 3; previously reproduced on unchanged
  9c60e1c. Tolerance not relaxed. Workspace doc-tests also passed (0 examples).
  New ECO/IO tests pass; log target/eco-native-endpoints-workspace.txt.
- Latest Windows build checked both entry points: SCH Update with PCB selection
  and direct PCB Import, widths/groups/Check/Done, Only Show Errors and report.
  Source PCB SHA256 remains redacted-source-sha256. One PCBCraft process;
  Altium modal closed without Execute/Save.

- Latest executable after native pin-rounding fix repeated Validate/Execute:
  49 applicable operations, correct Check/Done and Report. Home shows new bodies
  beside the board; one Undo restores clean original ReferenceA.
  Latest PCBCraft PID 9212, sole live process.

- Additional GUI check after Undo: Home/Fit Document produced an empty viewport.
  View → Fit Board restored the original visible board without dirty state.
  Cause unknown; recorded in DESIGN_SYNC.md as open difference.
  Last view left at Fit Board, source ReferenceA unchanged.

## 2026-10-26 — ECO continuation: footprint libraries and native SchDoc

- Added supported-property writing in ASCII/CFB SchDoc: designator/comment,
  description/type, base parameters, current/new footprint models, label names.
  Original container/untouched payloads retained; result reimported/compared with
  scene before atomic destination replacement. Unchanged archive returned
  byte-for-byte; unsupported geometry/unknown index dependencies are not saved
  under a false supported claim.
- Comparing a separate ReferenceA copy in AD24 caught an error our importer
  missed: globally appended records beyond old Weight were invisible in Altium.
  Fixed Weight from actual record count, owner-tree insertion and OWNERINDEX
  remapping. AD24 confirmed new footprint, Unicode parameter, WA1 comment and
  description. Original ReferenceA was neither saved nor executed in Altium.
- A multiline diagnostic parameter displayed `~1` literally in Properties.
  Until native encoding is confirmed, SchDoc Save rejects modified multiline
  parameters; JSON retains them. This is a limitation, not parity.
- Independent PCB6 PcbLib/IntLib reader: pads, layers, stack/masks, graphics,
  text and available embedded models. Unknown tag/count/encoding does not create
  partial footprints. ReferenceLibraryA: 29 footprints with pads, 4 explicit graphical/
  no-pad refusals. TPS22929 PcbLib/IntLib: 3 footprints each; STEP assembly transforms
  warned as unsupported. PcbLib installation through File Open checked in real
  PCBCraft; source board stayed clean.
- App ECO uses project/installed local libraries. Copying a footprint from an
  existing instance no longer hides missing-library errors. Original ReferenceA
  now 60 changes / 26 applicable, 1.0121 s; all 12 missing-footprint operations
  fail as observed in Altium. Previous 49 applicable belonged to explicit
  board-reuse experiment; CLI retains that behind a separate flag.
- GUI repeated Validate/Execute (26 in memory), single Undo and actual Fit
  Document (Ctrl+PageDown). Board stays visible/clean. Previous log incorrectly
  called Home Fit: Home centers at pointer. No bounds/cache defect after Undo proven.
- Clarified native reverse ECO: Remove Pins From Nets (22) followed by Remove
  Component Class Members (12), not symbol deletion. Altium Validate again gave
  failed-to-activate; Execute not invoked, window closed with Close. General
  properties/net renames supported by core are not presented as native back
  annotation: app disables them, retaining designator/comment/footprint.
  Classes, compiled pin changes and swapping not yet reconstructed.
- ECO uses natural BQ1…TP9…TP17…WA1 order without changing execution dependencies.
  Added tests for native reverse policy, app library dependencies, owner/Weight/
  legacy Unicode and actual ECO footprint geometry installation from containers.
  Before final multiline guard: 500 pass / 1 existing GPU fail, fmt/strict Clippy
  passed. Latest-build final results below.

- Final run after guard, stable Unique IDs and moving irrelevant library warnings
  from ECO to Messages: fmt --check, strict all-targets Clippy and Windows build
  passed. Workspace 500 pass / 1 existing GPU fail (same Rgba8Unorm/frame13/
  dither=true/max-channel=4; tolerance 3 unchanged).
  Log target/eco-completed-stage-tests.txt. Repeated SchDoc saves retain identical
  new IDs/bytes; ASCII and CFB tests pass.
  SchDoc SHA256 redacted-source-sha256, PcbDoc SHA256 redacted-source-sha256.
  Full parity remains partial: Comparator/ECO options, classes/rooms/directives/
  rules/variants, Component Links, hierarchy/buses/hidden pins/swaps, full native
  saving and exact all-state visual verification unfinished.

### 2026-10-26 — local Windows portable release

- At user request, built Windows x64 MSVC release with Cargo.lock.
  Added portable.flag: eframe settings save to Data/app.ron beside executable.
  Without marker, normal system storage is used.
- scripts/build-windows-portable.py creates ZIP, SHA256, build-info, dependency/
  font licenses, checks PE imports and includes Visual Studio redistributable
  CRT. System DLLs/graphics driver not copied. Archive checked with testzip;
  user settings excluded.
- Output: dist/PCBCraft-0.1.0-windows-x64-portable.zip, 12 284 166 bytes.
  SHA256 redacted-source-sha256.
- Extracted exe launched from another working directory (TEMP), sole instance.
  Source ReferenceA SchDoc/PcbDoc opened in real window; normal close created
  local Data/app.ron. Documents unsaved.
- fmt --check, strict workspace/all-targets Clippy and release build passed.
  Workspace 500 pass / 1 existing GPU fail (routing pixels differ by 4,
  frame13/dither=true). Logs target/windows-portable-*.txt.
  Launch on clean Windows without developer tools not separately checked.

### 2026-10-26 — hang opening complex PCBs

- User-reported board is on another device, unavailable here. Local
  ReferenceCRevision2 reproduced a separate pre-first-frame hang:
  robust_triangulation, BOURNS_INDM135125X65N_SRP1265A.STEP, shell 2353.
  Original probe consumed over 56 CPU seconds without a result.
- Original-edge limits did not cover internal trimming PCurves. Added surface
  adapter with model-wide 250 000 evaluation budget, three-second deadline and
  bounded surface grid. Model errors produce warnings and existing body
  projection; STEP stays in source archive. Failed imports cached too; after
  15 s model-library budget, no next model starts. These are cooperative limits,
  not isolation of arbitrary parser/topology code in a forcibly terminated process.
- File, Recent, Projects tree and startup-argument opening moved into one worker;
  bounded queue coalesces identical paths. Loading window shows file, phase/time;
  Cancel stops queue and prevents publication. No new worker starts until a
  cancelled task exits.
- Real portable release: ReferenceC loading at 3D model 20/141, 2.7 s observed;
  Cancel removed loading without partial-document publication. Next Recent click
  successfully opened/displayed ReferenceA PCB. App closed normally; one instance
  at a time. First full ReferenceC GUI import returned separate Invalid PCB Region
  geometry error; window stayed responsive.
- Other large local files reveal independent padstack-land, parameter-group-owner
  and PLANE2 compatibility errors. Validation was not disabled to simulate fixes.
  Details/check boundaries: PERFORMANCE.md/features.json.
- Portable ZIP rebuilt; Diagnose.cmd enables import/STEP tracing in
  Data/diagnostic.log for another-device diagnosis. User settings/projects excluded.
  ReferenceC SHA256 remains redacted-source-sha256,
  ReferenceA PCB SHA256 redacted-source-sha256.
- fmt --check, strict Clippy/release build passed; workspace 506 pass / 1 existing
  GPU fail, same routing frame13/dither=true/channel difference 4.
  Real Truck PCurve, curved/planar grids, invalid/deadline budgets, import cancel,
  queue/worker-slot retention checked. Logs target/complex-pcb-*.
  Exact user case on the other device remains unverified.

### 2026-10-26 — false Invalid PCB Region geometry on valid boards

- Refined core diagnostics: source stream/index, outer-contour size and invalid
  inner-contour indices/sizes instead of a generic error. ReferenceCRevision2
  Regions6 record 401 has 2386 outer points, 71 interiors; interior 32 collapses
  to one point (250987781,70407355) nm. Same in record 546/duplicate ShapeBasedRegions6.
- Native decoder excludes only interiors with fewer than three distinct integer-nm
  points from editable geometry, with warnings. Strict core/JSON validation not
  relaxed; real holes, including 1 nm triangles, retained. Invalid outer contours,
  NaN/truncated blocks still rejected. Original native bytes stay archived.
- Two new CFB regressions: legacy/modern region families, empty/single-point/
  repeated/two-point interiors, ordinary/minimal holes. Unmodified native Save is
  byte-identical; after adding a track, both source region streams including
  opaque tails are preserved byte-for-byte.
- Read-only probes: ReferenceC 3.0.7 — 27202 items / 1116 components / 24 layers,
  11125.64 ms; ReferenceC 3.0.1 — 18128 items / 790 components / 23 layers,
  17676.31 ms under parallel build, with 3D time-budget warning.
  Added --load-only to pcb_frame_profile; timing excludes first render.
- New portable release opened/displayed ReferenceC 3.0.7 in real window;
  selection/PageUp zoom work, document clean. App closed without Save;
  only one instance. ReferenceC 3.0.7 SHA256 remains redacted-source-sha256.
- fmt --check, strict Clippy/release build passed; 508 workspace tests pass /
  1 existing GPU fail (frame13/dither=true/max-channel=4). First test build hit
  Windows file lock from running pcb_frame_profile; workspace rerun after probe
  finished. Logs target/region-*.
- ZIP rebuilt: SHA256 redacted-source-sha256.
  Fixed the confirmed interior variant, not every native-region type/topology.
  Other padstack/layer/parameter limits remain separate.

### 2026-10-27 — exact ReferenceGWorkspace/ReferenceGRevision1.PcbDoc

- Reproduced error on user-specified file (21 056 512 bytes): Invalid PCB Region
  geometry, Regions6 record 11, outer vertices=2. Read-only stream analysis found
  13 legacy vertices with two distinct x values on one y and repetitions, not
  damaged record framing. Modern counterpart has the same vertices plus closing
  point. Problem arose during importer dedup/closing-vertex removal.
- If outer-contour normalization leaves <3 vertices, preserve original native
  sequence with warning. Core dimension/coordinate validation unchanged.
  is_empty_region tests exact collinearity; only these records contribute no
  material/connectivity, cutouts, artwork, picking or Fit bounds.
  Signed-area=0 is not used as the empty criterion.
- Legacy/modern CFB regressions: byte-identical unchanged Save, exact original
  region streams after adding a track. Core: remnant does not short a broken
  net; noncollinear signed-area-cancelling contour retained. Render: empty contour
  invisible/unpickable. pcb_regions is a bounded diagnostic example for region
  framing/coordinates without STEP import.
- Exact read-only import: 8494.63 ms, 20261 items, 1271 components, 26 layers,
  98 meshes. Source unchanged: SHA256 redacted-source-sha256.
- fmt --check, strict workspace/all-targets Clippy, release build pass.
  Workspace 511 pass / 1 existing GPU routing fail (Rgba8Unorm/frame13/
  dither=true/channel difference 4). Logs target/ReferenceG-gaz-*.
- New portable ZIP built/packager-verified, SHA256 redacted-source-sha256.
  Existing portable exe not overwritten/restarted: real window has unsaved
  Controller.pcbcraft.json, ReferenceK.SchDoc active. Therefore new-exe GUI on this
  board unverified; no second instance launched. User must restart with new build.

### 2026-10-27 — recurring remote Region failure and PCB import audit

- User confirmed Invalid PCB Region geometry text, but problematic board is absent
  here. Exact reproduction impossible; fixing that specific file is not claimed verified.
- Short native outer contours with 1/2 vertices are closed by repeating an original
  vertex without adding area. Fully empty Regions remain source-only
  empty_region_records with original CFB archived; no fictitious geometry.
  Empty outer with real holes, NaN/Inf, out-of-range coordinates, oversized counts
  and truncated payloads still rejected.
- Native Save preserves families containing empty slots byte-for-byte, including
  ordinary regions after empty slots. Editing such families/native reindexing
  refused with JSON suggestion. Byte-identical Save, JSON recovery, unrelated
  track edit and refusal to lose empty records checked.
- Of 18 locally distinct SHA256 files, 14 imported before changes: ReferenceM
  needed UID-only PrimitiveParameters, ReferenceGTest PLANE2; ReferenceD/ReferenceE
  had zero-width arcs. Added unambiguous PRIMITIVEID owner resolution and UID-only
  header preservation without duplicates; PLANE1..16; zero-width arcs without
  invented width/copper/connectivity. Negative widths prohibited.
- Primitive errors include stream/record/byte offset; model-validation errors
  include item/layer/source, plus padstack dimensions/shape/drill. Loading adds
  phase/path. Logger enables actual pcbcraft module.
- fmt --check/strict workspace/all-targets Clippy pass. Workspace 516 pass /
  1 existing GPU routing fail (frame 13, Rgba8Unorm, dither=true, difference 4).
  All 98 IO tests pass. Logs target/import-audit-*.
- UI unchanged; no second instance launched. Existing portable PID 45684 left
  running without restart/executable overwrite.
- Final read-only run: 18/18 imported (previously 14/18). All before/after SHA256
  match. Machine-readable report: docs/pcb-update/import-compatibility-2026-10-27.json.
  Run concurrent with release build; not a user-perceived opening-latency measurement.
- Source-only reference checks use an ordinary-record index rather than rescanning
  all items per empty record. Final fmt/Clippy/workspace checks repeated after
  this change; 516 pass and same GPU fail.
- Portable release built, CRC/SHA checked. Separate archive:
  PCBCraft-0.1.0-windows-x64-portable-2026-10-27-import-fix.zip,
  12 317 060 bytes, SHA256 redacted-source-sha256.
  build-info.json includes UTC build time/exe SHA256. Existing running instance
  not replaced; user needs to open the new ZIP separately.

### 2026-10-27 — dark interface theme only

- The application has no separate light palette or switch. However, egui follows
  the OS theme by default, while set_visuals changes only the current style.
- theme::apply now explicitly selects Theme::Dark before palette/typography setup.
  Applied at startup/Restore Default Layout, preventing automatic light-OS-theme
  switching of the interface.
- Existing dark palette appearance unchanged; running instance with user documents
  not restarted. Portable ZIP not rebuilt for this change.
- Checks: fmt/strict Clippy pass; workspace 516 pass / 1 existing GPU routing fail
  (difference 4 vs tolerance 3).

### 2026-10-27 — ReferenceG hierarchical schematic, continuing stage

- Import/portable/theme changes committed before starting: 68eb55e. New hierarchy
  changes remain in the working tree; user did not request a separate push.
- Source: ReferenceGWorkspace/ReferenceG.PrjPcb. All 9 SchDoc imported; top Main has
  8 Sheet Symbols, 410 Entries and 410 wires; child sheets have 410 Ports.
  Keys previously rejected four 13-point Beziers: cubic chains 3n+1 now supported.
- Sheet Symbol/Entry gained typed models/parent identity, owned name/file labels,
  correct DistanceFromTop units (100 mil, _FRAC1 / 1e6), single rounding of absolute
  coordinates. Move/resize/name edits, group copy/delete and connected-wire drag
  go through core History.
- Native SchDoc writer preserves supported existing Symbol/Entry/label/wire edits;
  verifies parent ID/exact connections after reimport. Added ASCII/CFB roundtrip,
  Unicode filename, group move/delete and copy-identity regressions.
  New native hierarchy insertion unsupported; placement/Entry synchronization disabled.
- Tools > Up/Down Hierarchy and Ctrl+double-click link block/Entry to child sheet/
  Port and Port to parent Entry. Current project only, owner-relative paths,
  only necessary sheets loaded. Ambiguous instances explicitly rejected.
  PrjPcbStructure supplies initial sheet numbers in Projects/tabs; Main opens first.
- October 27 Computer Use recovered after Escape stop. Live Altium 24.4.1 inspected
  U_Connector, KL30 Entry and 4 Kind forms; Up/Down Hierarchy opened Connector.SchDoc
  at KL30. PCBCraft checked Main display (all 8 blocks), block selection and Entry
  ACC → Connector Port ACC. Properties/menu order being corrected from observations;
  Symbol Parameters, unrendered Kind shapes, Device/Managed and other unsupported
  actions disabled. Original SchDoc files in Altium unchanged.
- Compiler joins Entry/Port by explicit owner/name; repeated local designators
  on different sheets no longer short pins. Test with two child R1 and identical
  local Port names checks separation. Multichannel/buses/harness support not claimed.
- Full ReferenceG electrical verification incomplete: compile reports GND/1V8
  conflict; 68 Entries do not exactly meet wire ends (e.g. block width gives
  x=199999600 nm vs wire x=200000006 nm). No automatic proximity merge.
  Full Altium parity not claimed.

- Additional read-only trace found false GND/1V8 merge path: ReferenceJ record
  24078 GND → wire 27710 → C386-1 record 24353 → another C386-1 record 22702 →
  wire 27585 → 1V8 record 22759. Two distinct single-part components share a local
  designator. Compiler now rejects ambiguity before merging pins; separate
  regression checks refusal while retaining genuine distinct-part support.
  Full compiled annotation/Component Links for this project not yet reconstructed.
- First full run after navigation: 520 pass / same existing GPU routing failure
  (Rgba8Unorm frame 13, dither=true, difference 4). After duplicate checks and
  Sheet Entries table-width fix, final rerun underway. Strict Clippy/fmt pass.

- Final rerun: fmt --check/strict workspace/all-targets Clippy pass;
  cargo test --workspace: 521 pass, 1 existing GPU fail with same parameters.
  Dev executable built; Computer Use checked Main [1], U_Connector Properties,
  corrected Entries columns and KL30 selection from table in final window.
  New build left as one instance, no source-project writing.
  Logs: target/hierarchy-{clippy,workspace-tests,build,navigation-tests,trace}.txt.

- Last live Connector/KL30 comparison found an old Port bug: Output pointed right
  instead of left, Unspecified had an arrow, 2.5 mm height replaced with 2.54 mm,
  BorderWidth/Alignment/fill ignored. Import/rendering fixed; Properties shows
  Alignment instead of inappropriate Style. Both Port endpoints participate in
  connectivity RTree, cross-probe, wire handles/junctions, snapping/drag;
  I/O type does not change electrical points. Different net-identifier types do
  not connect merely by matching names. Official I/O/Direction/net-scope references:
  https://www.altium.com/documentation/altium-designer/schematic/multi-sheet-hierarchical-designs
  https://www.altium.com/documentation/altium-designer/schematic/creating-circuit-connectivity
- After Port: fmt --check/strict Clippy pass. Full workspace: 523 pass /
  1 existing GPU routing fail (difference 4). Native KL30 far-end roundtrip and
  horizontal/vertical Port snap/drag with every I/O pass. Port Direction preference,
  legacy styles, full project naming/net-scope settings and power-port localization
  remain separate differences.

- Final-build window repeated Entry KL30 → Connector/KL30: only Main [1] and
  Connector [7] opened; Output arrow points left, Gnd has no arrow, Properties
  shows 12.5 × 2.5 mm, Small border, Right alignment.
  Repeat read-only ReferenceG probe now stops at first VD12 duplicate in
  ReferenceL.SchDoc before connectivity calculation (ReferenceJ also contains the
  earlier C386). All 9 sheets load; source documents not written.

### 2026-10-27 — Synchronize Sheet Entries and Ports

- Reference: live AD Enterprise 24.4.1, Design > Synchronize Sheet Entries and
  Ports, original ReferenceG all-matched state. Dialog 864×511, three lists,
  block tabs, unmatched filter, directional transfer, unlink, add/delete and
  immediate Undo in changed document. Behavior also checked against official
  Multi-sheet & Hierarchical Designs (Synchronizing the Ports and Sheet Entries):
  https://www.altium.com/documentation/altium-designer/schematic/multi-sheet-hierarchical-designs
- eda-core compares Name/I/O Type/Harness Type, prepares transfer transactions
  without changing existing-object/wire geometry; places missing Ports/Entries
  as a group on a selected owner side.
- App: whole project or one Sheet Symbol, three lists/tabs, Ctrl/Shift/drag/
  Select All, unlink/relink, bidirectional transfer, add/delete. Preview leaves
  document unchanged; click/Enter places, Esc/right-click cancels and returns to
  dialog. Per-sheet history; session/revision guards reject stale placement.
  Adding Entries focuses camera on the relevant block. Harness Type editable in Properties.
- Native SchDoc: new records 16/18, existing Name/IO/Harness and Port geometry,
  deletion, tree insertion/Weight/owner remap, stable UID across repeated Save
  and rename without source-archive reload. Text frames 209 and Additional
  drawing regions 225 retained; unknown index dependencies still block unsafe
  writing. Exact electrical-contact checks precede permissible coordinate-rounding normalization.
- Original ReferenceG: all 410 Entry/Port pairs match. Copies of nine SchDoc saved
  under target/entry-sync-qa-20261027-a. Live PCBCraft Add Ports created
  QA_ENTRY_ONLY, P → E synchronized KL30 (Output → Input), Add Sheet Entries
  created QA_PORT_ONLY on U_Connector. Dialog showed all matched; Save All wrote
  both copies. Fresh disk import confirmed 412 pairs / 0 unmatched.
  Original user SchDoc files not written.
- GUI QA found horizontal Links expansion, Esc conflict and context input/memory
  deadlock when opening submenu. Fixed vertical layout, Esc handling and popup
  state retrieval before input closure; regressions cover long lists, Esc,
  submenu clicks, bidirectional transfer, unlink/relink, cancellation, stale guards
  and Undo. Native tests also cover ASCII/CFB/Unicode, insert/update/delete and
  Additional payload preservation.
- Full visual parity not claimed: native icons, typography and column resizing
  remain. Live Altium reopen of QA copy not confirmed: file dialog timed out
  on activate_window/click; after recovery, further Altium input stopped.
  Linux untested. Harness Type metadata does not mean bus/harness compilation is ready.
- Final sync checks: cargo fmt --all -- --check and strict workspace/all-targets
  Clippy pass. cargo test --workspace --no-fail-fast: 534 pass / 1 existing GPU
  routing fail (Rgba8Unorm, frame 13, dither=true, difference 4).
  All 6 new app interaction, 3 core sync and 11 native writer tests pass.
  Dev executable built. Logs:
  target/entry-sync-{fmt,clippy,workspace-tests,build,reopened-qa}.txt.
- Subsequent live check: single-U_Connector context invocation, matched list,
  Esc and global 8 tabs work; original Main unchanged. Fast dragging now handles
  press/move/release in one frame; one interaction test covers coalesced/separate events.

- Computer Use recovered after closing the blocking file window. AD24.4.1 opened
  SheetSync_QA.PrjPcb and both saved schematics: all matched, 8 tabs;
  U_Connector/KL30 Input. Links has QA_ENTRY_ONLY/QA_PORT_ONLY, both Input.
  New Entry visible on Main, new Port on Connector at 70 / 122.5 mm.
  This is live native reopen verification, not merely our reimport.
- Live dialog refined 864×511, three-list dimensions/spacing, 17 px rows,
  transfer buttons/tabs. Columns now resize; interaction regression checks
  resizing without accidental row selection. Icons use original vectors;
  exact raster parity/all placement settings unconfirmed.
- Created Port Properties in Altium confirmed ALIGNMENT=0 Center (1 Left, 2 Right).
  Renderer/Properties fixed. TEXTCOLOR now independent of COLOR: absent imported
  field defaults to native black; new synchronized Ports get red labels; native
  Save retains separate color. ASCII/CFB regressions check independent label/
  outline changes and alignment. FONTID=2/SYSTEMFONT=2 in copy correctly reference
  Altium GOST Type A; font table needs no change. Live Altium Font combo is blank
  for original KL30 too, so this state does not prove a created-Port FONTID error.
- After Port fixes: 536 unit tests pass; sole existing GPU routing/dither fail
  remains (frame 13, difference 4 vs tolerance 3). Tolerance unchanged.
  New renderer regression checks actual Left/Center/Right label positions and
  independent text color.
- New-build window checked on original ReferenceG: 864×511 dialog, all matched,
  then all eight tabs/populated Links. Fast CU divider drag exposed lost width
  changes on coalesced events; track initial press/last coordinate and expand
  test to same-frame press/move/release. Documents stay clean.
- Final Windows build: fast CU Links-divider drag x1242 to x1260 actually changes
  width without selecting rows. Fmt/strict Clippy pass; workspace still 536 pass /
  1 existing GPU fail. One PCBCraft on original ReferenceG; user schematics not written.

### 2026-10-27 — hierarchical schematic/PCB ECO on ReferenceG

- Implemented: removed the app's blanket hierarchy rejection. Current editable
  Sheet Symbol filenames resolve relative to their owners; full symbol/component
  UID paths and sheet/component pin identities prevent cross-sheet collisions.
  Duplicate/unannotated source can be reviewed without losing stable linked
  components; unsupported source nets are protected from silent destructive edits.
- Implemented: project scope/naming flags, Entry/Port union, localized power,
  multipart logical reference folding, native Compile Masks (expanded/collapsed,
  fully enclosed objects and strict mask boundaries), legacy/modern component
  kinds, preserved PCB Layer metadata, native ComponentLink URL `=`/`{}` codec.
  Native nonzero HierarchyMode and NameNetsHierarchically=1 are explicitly disabled
  pending observation; repeated sheet/multichannel instances and bus/harness/hidden
  pins remain unsupported. A masked Sheet Symbol requires whole-child exclusion
  and is diagnosed, not partially compiled.
- Observed: native ReferenceG fractional hotspots differ from attached wires by 10–1580 nm.
  Implemented: project compiler-only 2540 nm hotspot-to-segment tolerance; drawing
  and native source geometry stay unchanged, bare interior crossings stay separate.
  This is a corpus-calibrated compatibility rule, not a proven universal AD constant.
- Observed: AD24.4.1 exported the original ReferenceG project as Protel netlist,
  478 distinct names and 2647 pin memberships. Independent compilation matches
  all memberships/names ignoring case, with no missing native pins. Lowercase
  name ordering resolves TR_WK vs TRANSP_WK and Uni_*_PU_* aliases. Root-sheet
  spelling is deterministic for equal folded names. Exact case is audited below.
- Observed: the native ECO displayed four Add Rooms only, with no Execute Changes.
  The exported netlist omits single-pin HSM_OE/Shutdown present in the PCB. Exact
  native ECO/comparator-option agreement is therefore NOT established. The app
  retains explicit review/selection; unrelated floating source pins missing in
  the footprint no longer produce a spurious Unknown Pin disconnect.
- Implemented: scalar ECO operations preflight then mutate private staged state;
  geometric edits clone only affected documents. Final full validation and History
  guards remain. Original ReferenceG comparison+validation fell from minutes of per-row
  full-project clones to about 1.8 seconds in the dev profile.
- Verified: isolated target/hierarchy-eco-qa-20261027-c. DD1 unlocked only in the
  in-memory QA copy; forward comment + TCK→QA_TCK, reverse child-sheet comment,
  unchanged nontext PCB items/copper and net index, exact Undo/Redo, all nine native
  SchDoc saves/reimports and native PcbDoc save/reimport. Source project hashes
  checked independently. Regression tests cover identities, masks/boundaries,
  power scope, naming, tolerance, parameter namespaces, kind versions and staging.
- GUI limitation: later CU activation failed (`failed to activate captured window`);
  fresh captures of Altium and PCBCraft were black. Asked the user to unlock the
  session/close the old PCBCraft. A separate dev executable was prepared in
  target/hierarchy-qa/pcbcraft.exe without launching another instance. Final live
  window verification remains pending. No native source project was saved.
- Final check results and reference audit follow below.

- Safety follow-up: original ReferenceG's remaining four single-pin-removal rows are
  explicitly unavailable in project review, with the native ECO uncertainty
  explained. Export omission is not treated as authorization to disconnect PCB
  pins. A regression ensures no changed document is staged from such rows.
- Reference spelling audit: 79 memberships across 3V3_MCU/PROG/MCU_ON still use
  different case from Altium's export; all 478 names/2647 memberships agree
  case-insensitively. No case-only PCB pin update is generated. Exact native
  capitalization selection is not claimed.

- Final verification: fmt check and strict workspace/all-target Clippy PASS.
  Full workspace: 554 unit tests PASS; one previously existing GPU routing
  Rgba8Unorm frame-13/dither failure remains (pixel difference 4, tolerance 3).
  No tolerance in that test was changed. Original ReferenceG: 4 review-only unavailable
  single-pin rows, 0 applicable destructive operations, compare+validate ~1.8 s.
  Final reference audit: 2647/2647 memberships, 478/478 names ignoring case,
  no missing reference pins. All 11 original source SHA256 hashes unchanged.
  Native QA roundtrip PASS in target/hierarchy-eco-qa-20261027-c.
  Latest dev executable: target/hierarchy-qa/pcbcraft.exe (built successfully
  with an alternate rustc output path because the old debug executable is running).
  Logs: target/hierarchy-{fmt-final,clippy-final,workspace-final,build-final,
  net-audit-final,reference-audit,roundtrip}.txt.


### 2026-10-28 — resumed live hierarchy ECO verification

- Windows session became accessible. The old debug PCBCraft had unsaved Main
  changes; saved them through File/Save As to
  target/session-recovery-20261028/Main-before-restart.pcbcraft.json. The source
  archive and all 410 Sheet Entries are retained. Original 11 source files still
  match their prior SHA256 hashes. Closed the old app cleanly after that save.
- sky.launch_app twice failed before starting a process (`accessibility
  window-opened handler did not become ready`). A normal development-process
  launch started target/hierarchy-qa/pcbcraft.exe with isolated QA project
  target/hierarchy-eco-qa-20261027-c/ReferenceG.PrjPcb, after checking zero live instances.
  Exactly one new PCBCraft is running, initially titled ReferenceK.SchDoc.
- After launch, capture/activation failed twice despite refreshing selection:
  `foreground window did not report a process id`. Asked the user to foreground
  PCBCraft manually. No automated inputs followed the failed recovery. Final live
  ECO execution/reopen is still pending; prior core/native QA checks remain valid.


### 2026-10-28 — live hierarchical ECO and native component preservation

- Resetting the Computer Use JS kernel recovered capture after manual
  foregrounding. On isolated QA ReferenceG-c, all nine sheets and PCB loaded through
  Design/Update PCB. Baseline ECO showed four explicitly unavailable single-pin
  removal rows. No baseline destructive operation was executable.
- GUI forward QA: MCU/DD1 Comment -> GUI Forward Hierarchy QA; Validate/Execute
  succeeded. Tools/Cross Probe located PCB DD1 and Properties confirmed the value.
  GUI reverse QA: PCB DD1 Comment -> GUI Reverse Hierarchy QA; Design/Update
  Schematics produced exactly one enabled comment row targeting ReferenceK.SchDoc.
  Validate/Execute, child-sheet Properties, menu Undo and Ctrl+Y all confirmed it.
  Other eight sheets stayed clean. Save All wrote QA native files; fresh comparison
  returned the same four unavailable baseline rows. AD24.4.1 opened both files
  and showed the changed DD1 comment in both editors.
- Native reopening exposed a persistence defect: Components6 rewrote equal
  fields on unrelated edits. Cyrillic legacy fields became numeric HTML entities,
  and numeric spellings changed; AD then showed corrupt library metadata,
  rotation 0 and height 0 on DD1. Fix: delta-patch only semantically changed
  component fields, leaving unchanged native bytes and ID spelling intact.
  Added a binary regression for a comment ECO and unrelated track edit, including
  scientific notation, mil units, legacy Cyrillic, Unicode shadows and duplicates.
- New original-source QA roundtrip in target/hierarchy-eco-qa-20261028-d PASS:
  forward comment/net rename, reverse comment to correct MCU child, native
  save/reimport and exact Undo/Redo. Independent raw CFB audit: 1270/1271 component
  blocks byte-identical; DD1 changes only intentional LOCKED and SOURCECOMMENT.
  The entire remaining DD1 byte sequence is preserved, not merely decoded values.
- AD24.4.1 reopened fixed QA-d: DD1 Comment = PCBCraft reverse hierarchy QA;
  Rotation = 270, Height = 1.6mm, Design Item ID = 5696, Source = DB.DbLib;
  Cyrillic footprint and library path plus description now display correctly.
  This does not verify all newly changed Unicode/numeric native values.
- fmt check and strict workspace/all-target Clippy PASS. Workspace unit tests:
  555 PASS, 1 existing GPU routing dither mismatch (Rgba8Unorm, frame 13,
  difference 4 versus tolerance 3). Tolerance unchanged. Latest build succeeded
  at target/hierarchy-qa/pcbcraft.exe; old app closed cleanly before launching
  exactly one replacement on QA-d. Logs: target/hierarchy-{fmt-gui,clippy-gui,
  workspace-gui,roundtrip-gui,build-gui,native-fields-fixed,gui-save-audit}.txt.
- Remaining responsiveness issue: native Save All blocked GUI for about 20 s
  while PCB serialization/verification ran, then recovered with clean documents.
  Native comparator single-pin/rooms parity, exact capitalization, multichannel,
  buses/harnesses, nondefault scopes and all other listed limits remain open.
- Final fresh-window check: replacement PCBCraft PID 35832 loaded QA-d MCU
  cleanly and responds; exactly one live instance. Altium also shows the reverse
  comment on QA-d MCU/DD1. All 11 original SCH/PCB/PrjPcb SHA256 hashes remain
  unchanged after both native reopen checks. No commit or push performed.

### 2026-10-31 — PCB routing startup and Altium controls

- Inspected the running Altium Designer Enterprise 24.4.1 at 1707 px width:
  Route menu, floating Active Bar, Interactive Routing Properties and Tab pause.
  Used the existing research PCB copy; the user project was not edited/saved.
  Full visual and solver parity is not accepted by this observation.
- Fixed P,T interception by the Cross Probe prefix handler; full-dispatcher
  tests cover P,T, Ctrl+W, T,C and explicit tool changes. Restored the floating
  PCB Active Bar with working routing/placement commands and the observed menu
  order. Unsupported engineering commands remain disabled.
- No-net routing no longer obstructs itself at starting pads/tracks or at its
  own copper after a layer change. Only contacted terminals/session additions
  may merge; unrelated copper, drills and keepouts remain obstacles. Terminal
  contacts roll back with checkpoints; existing plated pads can provide a layer
  connection. Direct hits beat nearby pads; drill-only holes cannot be targets.
- Added visible-layer startup and prevented previous-net inheritance on a new
  free-space route. Reworked Net Information/via/conflict sections, Width focus,
  pause overlay and one-Escape resume. A real-window layout failure was caught
  and fixed; height/focus regressions protect these controls. Route status now
  exposes a clearance blockage instead of an unexplained missing leg.
- Windows GUI on the generated isolated fixture: two No Net pads connected via
  an intermediate corner, automatic finish and clean Undo. That ASCII fixture
  was not recognized by Altium and is not a native compatibility validation.
- Final Windows GUI on target/routing-check/ReferenceBRouting.PcbDoc (15,513 items):
  Ctrl+W, ExampleGround SMD start, approximately 0.996 mm placed copper, Tab/Width,
  Escape resume/finish, Undo/Redo/Undo to a clean title. Source research copy and
  GUI copy SHA256 both remain
  redacted-source-sha256.
  Altium returned to the original ReferenceB document; no PCB source was saved.
- Required gates PASS: fmt --all -- --check; workspace/all-target Clippy with
  -D warnings; all 563 workspace unit tests; Windows dev build. Test output:
  target/routing-check/workspace-tests.txt. Updated docs/features.json and
  docs/pcb-update/INTERACTIVE_ROUTING.md, keeping routing partial.
- Remaining scope: native save of this route not newly verified; independent
  bounded walkaround/stop and existing native I/O guards remain. Push/HugNPush,
  arc corners, gloss, loop removal, auto-completion, favorites, exact icon/DPI
  parity and a new Linux GUI comparison are unfinished. No commit/push.

### 2026-10-31 — follow-up: track absent after terminal selection

- User clarified that routing enters but no track appears after choosing a pad.
  Rechecked the earlier ExampleGround via failure instead of treating its clearance
  status as a valid constraint. Native AD24.4.1 research copy routes from the
  corresponding terminal; the Rust probe initially produced zero segments in
  all three sampled directions.
- Located false blockers at Regions6/ShapeBasedRegions6 record 17: fragment NET
  is absent, polygon index 1 supplies ExampleGround. Connectivity inherited that net,
  but routing compared the raw empty fragment NET and blocked itself at startup.
- Added shared Board::item_net, used by routing, clearance net scope,
  InNet/InNetClass and connectivity. Explicit fragment nets retain priority;
  native primitive data stays unchanged. Shelved pour fragments do not obstruct.
  Three regressions cover own/foreign/shelved pours, explicit overrides,
  keepouts, via/backtrack and net-scoped/class-scoped rule resolution.
- Read-only pcb_route_profile now accepts --at X Y --blockers. At native via
  (166.180275,199.690002) mm it changes from zero legs to reaching (+1,+1) and
  (+3,0) mm, with preview/place/finish/Undo and no source write.
- Final Windows GUI on ReferenceBRouting.PcbDoc: Ctrl+W, that same via, cursor movement
  without a placement click produces a departing preview; click places and
  completes the connection, Undo returns a clean title. Original research copy
  and GUI copy still have the prior 7C337B5A...CB08B40F SHA256. Altium original
  ReferenceB tab restored; research experiments were not saved.
- fmt, strict workspace/all-target Clippy, all 566 workspace tests and Windows
  dev build PASS. Registry/docs updated. Full routing parity/native route save
  remains unverified as above; no commit/push.

### 2026-10-31 — schematic signal harnesses

- Observed the running Altium Enterprise 24.4.1 Place/Harness menu, connector,
  entry, Signal Harness and Predefined Connector Properties/dialogs. Created
  isolated HarnessReference.SchDoc; the user's original project was not edited
  or saved. Native Additional records establish IDs 215-218 and local owner
  indexing, including omitted zero owners and the 100mil entry-distance scale.
- Added core Connector/Entry geometry and ownership, constrained entry drag,
  same-carrier connected drag, transforms/copy/delete and validated history.
  Signal Harness, Bus, Bus Entry, Port and Sheet Entry placement is enabled.
  Harness Properties, entry management/fonts, Predefined reuse with optional
  line/port, Tab pause, 90/45/any-angle paths, backtrack and Esc repeat work.
- Project compilation expands independent leaf channels through physical paths,
  scoped ports and unique-instance hierarchy. Nested harnesses, numeric bus
  bits/entries, harness-label net names and connector-free LOCKED definitions
  are supported; cycles/type/definition/bit conflicts are explicit diagnostics.
  Local cross-probing follows channel joins. No external EDA execution engine.
- Native preserving writer supports harness insert/edit/delete, hidden type and
  font changes, owner remaps across FileHeader/Additional, attachment verification
  and repeated saves. Sibling .Harness generation preserves LOCKED definitions
  and native nested syntax. General authored Port/wire/label writes support the
  reused group; unknown native index dependencies still refuse unsafe Save.
- GUI QA caught issues missed by the first tests: Properties/Add used a distinct
  authored sync default; writing DISABLESYNC fixes native Save. Power Port
  SHOWNETNAME remains separate from parameter SHOWNAME. The Predefined dialog's
  two columns require explicit top-down child layouts; a bounded-size regression
  now catches the overflow. Dirty graphical definitions supersede stale unlocked
  sidecars in the reuse dialog.
- Final Windows GUI on target/routing-check/HarnessGui.SchDoc: Add entry 0,
  Save, Predefined Serial + three entries + 500mil line + HarnessPort, one-step
  Undo/Redo and Save. Altium reopened the saved file and visibly displays the new
  entry, all three connectors and HarnessPort. No save was performed in Altium
  after reopening. Final QA file SHA256:
  redacted-source-sha256
  (the final group version is retained on disk).
- Required gates PASS: cargo fmt --all -- --check; cargo clippy --workspace
  --all-targets -- -D warnings; all 585 workspace unit tests. Linux GNU
  all-target check PASS; runtime not checked. Test log is
  target/routing-check/harness-workspace-tests.txt. Windows binary linked under
  target/debug/deps; root copy fails because the old pcbcraft.exe is running
  with unsaved user PCB edits. Preserved it, launched a separately named final
  binary, and closed intermediate harness QA windows without saving their drafts.
- Full Altium parity is still partial: Auto Wire/look-ahead preferences,
  classes/multichannel sheets, Device Sheet folders, Active Bar hold menus and
  context actions, exact brace/selection/table/font/DPI rendering. Native creation
  without a source archive remains unsupported. See docs/schematic-harnesses.md
  and schematic.signal_harnesses in docs/features.json. No commit or push.

### 2026-10-31 — follow-up: route across polygon pours of other nets

- Previous same-net inheritance fix was incomplete: foreign generated pour
  copper still blocked routing. Managed region/fill/track/arc fragments now
  allow routing, while free copper, orphaned references, keepouts and board
  cutouts retain obstacle handling. Committed routing invalidates derived
  copper and the existing background worker repours around the new geometry.
- Regression verifies Stop/Walkaround, preview/place/via/backtrack, actual
  0.6mm scoped polygon clearance after repour and atomic Undo/Redo. All 567
  workspace tests, fmt check and strict workspace/all-target Clippy pass.
- AD24.4.1 research-copy observation: HugNPush permits a free-space preview
  across the filled area, cancelled without saving. This was N/A net, not a
  pad-start comparison. No original Altium project edit/save.
- New Windows binary linked successfully under target/debug/deps; Cargo's
  final root copy failed because the old executable is running. Copied the
  newly linked artifact to target/routing-check/pcbcraft-polygons.exe and
  launched separately, preserving PID26472 with unsaved user edits.
- Actual new-build ReferenceBRouting GUI: ECALL_SW via -> foreground polygon area,
  preview visible, placement succeeds, Escape triggers automatic clearance
  repour; Undo restores clean title. Full parity/native save remain unverified.

### 2026-11-01 — schematic Active Bar restoration (partial)

- Reference screenshot establishes fourteen groups and the separate Arcs and
  Drawing buttons. Installed AD24.4.1 advsch.rcs TopBarSchematic establishes
  command captions/order, separators and power-port presets. Official vendor
  documentation establishes short click, one-second hold, right-click and
  last-used defaults. The resource's single Drawing group differs from the
  screenshot; the screenshot takes precedence. No Altium engine/code is used.
- Replaced the schematic placeholder strip with its own 476 by 33 1/3 point
  layout, fourteen original vector buttons, real hold/right-click menus, shared
  last-used defaults and Selection Filter. The 150% DPI interpretation remains
  an inference; exact visual equivalence has not been measured.
- Added core region/connection selection, edge/center/grid alignment and
  distribution, complete-owner stack ordering, Sheet Symbol flips and Sheet
  Entry operations. GUI actions use validated atomic history; offset and
  combined alignment dialogs guard document revisions. Existing Move/Drag,
  Components, Nets, Power Ports and Harness workflows are connected to the bar.
- Added graphical placement for rectangle/rounded rectangle, circular and
  elliptical arcs/circles, polygons, chained Bezier, Text Frame/Note, Sheet
  Symbols with owned texts and Generic No ERC. Escape/cancel, backtrack,
  rotation, repeated placement and modal canvas blocking are covered.
- Expanded the preserving native writer for these authored objects and owner
  groups. Stack changes survive serialization/reimport rather than being
  silently normalized away. Unknown native index dependencies and cross-stream
  stack changes still refuse save. Native verification found and resolved
  coordinate quantization and owned text presentation mismatches.
- Device Sheet Symbol, Reuse Block, Off Sheet Connector, Parameter Set,
  Differential Pair, Blanket, Compile Mask and Graphic import stay disabled.
  Text/Bezier bounds, some region selection, arc placement phases and graphic
  editing/preferences still differ or require AD24.4.1 observation. Full scope
  and remaining work: docs/interface/schematic-active-bar.md and features.json.
- Computer Use is blocked: @oai/sky and cua entry points report a missing
  Windows native pipe, os error 2. Retry, reconnect and tool reset did not
  recover it. No live Altium menu inspection or final Windows window check is
  claimed. A complete T3 Code restart was suggested as a diagnostic step, not
  a confirmed recovery procedure. Existing user windows were preserved.
- Required gates PASS: cargo fmt --all -- --check; cargo clippy --workspace
  --all-targets -- -D warnings; all 595 workspace unit tests. Linux GNU
  all-targets cargo check PASS; runtime unverified. Tests cover actual egui
  short click, hold, right-click, remembered defaults, filters, shape keys and
  validated history; native graphic/owner/power/stack regressions also pass.
  Headless egui mesh rendering produces
  target/routing-check/active-bar-panel.png; this is not a live GUI screenshot.
  Logs are in target/routing-check/active-bar-*.txt. Windows dev build PASS,
  target/debug/pcbcraft.exe updated successfully. No commit/push.
- User-requested Computer Use recheck after plugin update to 26.1027.30824:
  cua.getState still reports missing native pipe, os error 2, and no apps or
  browsers. Fresh node_repl reset followed by @oai/sky initialization fails
  before executing JavaScript: failed to write kernel assets, os error 3.
  Repeated initialization has the same result. Both live inspection paths
  remain blocked; no screenshots or app input were obtained.
- Later user-requested reconnect test succeeded: @oai/sky list_windows returned
  the running Altium 24.4.1 window. Window activation, screenshot capture,
  switching to HarnessGui.SchDoc, right-click opening Nets and Escape closing
  the menu all succeeded without editing the schematic. Live toolbar is about
  476 by 33 logical points at the observed 1707 by 1027 window. Nets popup is
  much narrower than the current PCBCraft popup and has per-command icons;
  matching popup metrics/icons remains required. Launched target/debug/
  pcbcraft.exe and captured its real 1000 by 650 restored PCB window; schematic
  interaction QA and the full Altium comparison remain outstanding. The
  Computer Use connection blocker is resolved.

### 2026-11-01 — live schematic Active Bar continuation (partial)

- Computer Use through @oai/sky worked for live AD24.4.1 and PCBCraft research.
  Corrected the initial screenshot interpretation: Drawing Tools includes all
  ten arc/graphic commands; the fourteenth gold hexagon is Simulation Sources.
  Observed all group menus, filter grid, popup dimensions, owner dialogs and
  native records on the isolated ActiveBarReference-20261101.SchDoc copy.
- Matched the 476 x 33 1/3 point strip, 29 1/3 point popup rows, native group
  widths, separators, icon/text offsets, last-used commands and keyboard menu
  navigation. New original vector icons distinguish menu operations. Fixed the
  8-point schematic toolbar vertical offset and default Projects width to 334.
  Exact raster/icon/font/dialog/preference parity remains incomplete.
- Added typed Off Sheet Connector, Parameter Set, Differential Pair, Blanket,
  Compile Mask, sources/probes/Initial Condition, Graphic and Device Sheet
  reference data. Core commands own placement, masks, copies and history.
  Owned differential-pair parameters determine type independently of the label;
  copied differential probes receive a new shared pair ID. Masks propagate
  exclusions to owned children and collapsing restores compilation membership.
- Added parameter/class/rule and source/probe Properties. Supported schematic
  PCB rule editors reuse PCBCraft's independent rules UI. Simulation metadata
  is authored and preserved; no simulator execution or result data is claimed.
  The existing importer can load installed Simulation Generic Components.IntLib;
  a three-symbol independent fallback is available when it is absent.
- Device Sheet selector reads registered folders and derives entries from child
  ports, with Device Sheet source kind and unique U_<stem> designators. Local
  Design Reuse creates selection-only snippets, places with fresh IDs and
  reserved designators, opens documents and edits metadata. Its local manifest
  and JSON format are PCBCraft-specific; native Altium snippet metadata,
  thumbnails, deletion and full hierarchical device semantics remain open.
  Workspace-dependent cloud/Managed commands remain disabled.
- Expanded preserving SchDoc writing for new objects, owned parameters, source
  components and SIM models, Device Sheet source kind, Additional Blankets and
  image Storage. External image caches load from bounded local files and do not
  alter unchanged native bytes. Native source-body fill requires RECORD=8
  Ellipse; writing it as RECORD=12 Arc rendered an unfilled body in Altium and
  was corrected with a regression. Native moves refuse incomplete inactive
  symbol geometry rather than updating only its represented subset.
- Real Windows GUI QA: Nets/Drawing/Simulation menus; source placement and
  pre/post-placement Properties; one-step Undo/Redo and Save; external image
  visible after reopen; Device Sheet folder registration, port-derived symbol
  placement and Save. Created ActiveBar Supply QA from one selected source,
  verified one component/eight primitives/no native archive or records, placed
  it as V2 and saved again. Latest build reopens the final file, Ctrl+A selects
  23 roots, and nested Device Sheet windows open/close correctly. Fixed child
  windows falling behind a disabled parent, native file-dialog ownership and
  Escape while a field has focus; modal layering has an event regression.
- AD24.4.1 opened the intermediate ActiveBarImplementation-20261101.SchDoc and
  recognized VSRC V1 with DC Magnitude=5, AC Magnitude=1 and AC Phase=0. Final
  ActiveBarFinal-20261101.SchDoc reopens in PCBCraft; its SHA256 is
  redacted-source-sha256.
  Final Altium reopen is NOT verified: its former window disappeared from
  inventory. Refresh found no targetable AD window, and the supported launch
  retry reported 'launched app did not expose a targetable window'. No original
  Altium project was saved for research. Removed only the temporary research
  folder registration in Altium; all files and the original folder entry remain.
- Required gates PASS after the final code changes: cargo fmt --all -- --check;
  cargo clippy --workspace --all-targets -- -D warnings; all 604 workspace unit
  tests (app 127, core 250, geom 1, IO 122, render 104). Linux GNU all-targets
  cargo check and Windows dev build PASS; Linux runtime/DPI remains unverified.
  Logs: target/routing-check/active-bar-{fmt,clippy,full-tests,linux-check,build}.txt.
  Latest separately launched binary: target/routing-check/active-bar-verified/
  pcbcraft.exe. The existing user PCB window with unsaved edits was preserved.
- The first full link exhausted disk space in generated test/example artifacts.
  Cargo clean -p eda-io reclaimed 21.5 GiB, followed by successful full rebuilds;
  no project/source/research files were removed. Remaining limitations are in
  docs/interface/schematic-active-bar.md and schematic.active_bar in features.json.
  No commit or push during this continuation.

### 2026-11-01 — schematic cursor contrast and sheet dimensions

- User report: a white cursor disappeared over the schematic sheet, and sheet
  sizes were inconsistent. The schematic requested the themed Windows Crosshair
  cursor; it now paints a 14 logical px black cross with a white halo in
  eda-render. It follows the actual pointer independently of placement snapping,
  stays clipped to the active canvas, and leaves normal cursors to panels,
  modals and unfocused windows.
- New documents previously combined an A4 label with physical ISO paper size
  297x210 mm, while import and Properties used the observed Altium drawing area
  292.1x193.04 mm. New documents, import and format selection now share native
  drawing dimensions. Standard selection preserves portrait/landscape and marks
  the current format. Existing saved dimensions are preserved until the user
  explicitly selects a standard format; custom sizes are not normalized.
- Regressions cover all 18 formats in both orientations, validated history and
  Undo/Redo, unchanged native bytes and custom sheet dimensions. Render events
  cover light/dark backgrounds, 100/150/200 percent DPI, active placement, panel
  hover, inactive windows and modal dialogs. Initial new-test failures were
  corrected: custom fixture coordinates use 10 mil units; the panel separator
  must not be counted as a cursor stroke. Final 607 workspace unit tests PASS
  (app 127, core 251, geom 1, IO 123, render 105).
- Real Windows QA on the final build: new A4 shows 292.1x193.04 mm; Portrait
  shows 193.04x292.1 mm; choosing A3 retains Portrait and shows 281.94x393.7 mm.
  Undo restores A4 Portrait, Redo restores A3 Portrait. Home fits the tall sheet
  with the expected aspect ratio; black crosshair is clearly visible on the
  light canvas and keeps its size across zoom. Separate portable QA executable:
  target/routing-check/sheet-cursor-qa/pcbcraft.exe. Other open documents were
  not modified during this QA.
- Fresh Altium visual comparison remains unavailable: inventory had no AD
  window and the supported launch reported 'launched app did not expose a
  targetable window'. Native dimensions are grounded in prior AD24.4.1
  observations and SchDoc import checks. Native sheet-property export and
  Linux runtime/DPI visual parity remain outside this fix.
- Required fmt check, strict all-target workspace Clippy, full workspace tests,
  Windows dev build and Linux GNU all-target check PASS. Logs:
  target/routing-check/sheet-cursor-{fmt,clippy,tests,build,linux}.txt.
  No commit or push requested for this fix.

### 2026-11-01 ? schematic File menu and file operations (partial)

- Compared live File/New/Script/Mixed-Signal Simulation/Import/Export in Altium
  Designer Enterprise 24.4.1 through Windows Computer Use. Inspected Preview SCH
  and Smart PDF welcome/target/BOM/additional-settings pages. Both applications
  were maximized on the same desktop DPI for menu comparison. The real Altium
  LastWorkspace DsnWrk confirmed `[ProjectGroup]`, not an assumed workspace
  section. Research did not save or edit the user's Altium project.
- Replaced the File placeholders with the observed order and nested menus,
  independent icon gutter, measured row widths, 22 px rows and native-style
  separator groups. Removed unrelated fabrication/assembly entries from this
  schematic menu. Added local PrjPcb and DsnWrk operations, separate recent
  histories/migration, group dirty protection, Save Copy, project/document
  reference relocation, safe native authored-sheet creation and sheet-property
  writing. Undo survives Save As without reverting the new file name.
- Added bounded ASCII DXF interchange and declarative PCBCraft command scripts.
  Imports and scripts validate before a single history transaction; unsupported
  DXF entities/primitive families refuse conversion. Independent command script
  fixture: fixtures/schematic/set-title.pcbscript.json.
- Added read-only Print preview and Smart PDF using the actual schematic
  renderer, including current unsaved scene snapshots. Print has drawing scope,
  color, media/orientation, fit/scale and offsets; Windows invokes system Print
  and GDI, Linux uses CUPS lp. PDF uses lossless raster pages and component/pin/
  label/port bookmarks with invisible destination links and optional local net
  information/parameters. Output is bounded by page and total memory limits.
- Windows QA: File width initially expanded to 600 px; corrected by measuring
  row content. Nested item spacing and dialog button order were corrected after
  visual inspection. Save Copy As created target/file-menu-qa/gui-copy.SchDoc:
  its SHA-256 matches the unchanged fixture, and the original tab/name stayed
  active. Smart PDF wizard produced gui-smart.pdf; pypdf checks and independent
  PDFium page rendering verified its page, four link annotations and bookmarks.
  Visible annotation note icons discovered in PDF QA were replaced with invisible
  links. Preview SCH reached the real Xerox printer dialog and was cancelled
  without submitting a print job. Parent-window ownership/DPI restoration and
  repaint after the printer dialog are explicit in the implementation.
- Required gates PASS: cargo fmt --all -- --check; strict all-target workspace
  Clippy; 617 workspace tests (app 131, core 252, geom 1, IO 127, render 106).
  Windows dev build and Linux GNU all-targets cargo check PASS. Logs/artifacts:
  target/file-menu-qa/{fmt.log,clippy.log,tests-final.log,linux-check.log}.
  One repeat hit disk exhaustion while linking examples; standard cargo clean
  -p eda-io removed 12.2 GiB of generated artifacts, then the full tests passed.
- Full requested parity is NOT achieved. Disabled New authoring systems and
  legacy/proprietary converters, DWG, Altium Delphi APIs, Excel BOM templates,
  variants/full-project PDF semantics and exact dialog/icon/raster parity remain
  incomplete. Physical printer output and Linux runtime/DPI were not verified.
  Complete implemented/partial/disabled matrix: docs/interface/schematic-file-menu.md;
  feature entry schematic.file_menu is explicitly partial.
- Final executable is target/debug/pcbcraft.exe; a separate portable copy in
  target/file-menu-qa/pcbcraft.exe is launched for review. The original running
  user window was preserved by renaming its old executable to
  target/debug/pcbcraft-before-file-menu-20261101.exe before rebuilding.
  No commit, push or external publication was requested or performed.

### 2026-11-01 — SCH File continuation: NET, graphical DXF, project PDF links

- Computer Use in AD Enterprise 24.4.1 confirmed Export NetList: Export File,
  Project/Document Scope, 20 ordered format choices, OK/Cancel. A separate
  Protel NET under target/file-menu-qa confirmed bracket component records,
  three reserved fields, parenthesis net records, designator-pin endpoints,
  CRLF and CP1251. The original Altium schematic was not edited or saved.
  Export to AutoCAD was inspected and canceled: version, format, units and
  Include Template options are recorded as observed, not implemented parity.
- Enabled Netlist Schematic with a working Protel exporter. eda-core supplies
  validated physical components and compiled connectivity using the existing
  multipart, annotation, mask and project hierarchy rules. eda-io handles
  format/encoding and atomic writes. Current unsaved project models take
  precedence over disk without opening additional tabs; other formats stay
  disabled. Live PCBCraft exported the active TCI sheet to gui-export.NET:
  14 component records, 14 nets, 991 bytes, source tab still clean.
  The observed dialog group positions and outer size were corrected after live
  comparison; repeat GUI NET export produced identical bytes. A newly created,
  not-yet-saved schematic also retains its project scope for export/output and
  Save Project As; the snapshot regression covers this association.
- Extended DXF from the old primitive subset to ordered graphical SOLID faces
  from the shared read-only renderer. Includes component/pin/power-port/port
  graphics, fills, frame and outlined fonts with holes; curves are flattened.
  eda-geom owns document-space faces, eda-render converts screen geometry and
  font contours, eda-io writes R2004/mm true-color DXF. Added planar SOLID
  reimport with fill-only triangles. Images/unknown objects/non-outline fonts
  and drawings above 250,000 faces refuse output before destination replacement.
- Smart PDF now compiles net information once in project context, including
  unchecked sheets, using native project options. Exported pin links cycle
  through visible pins of a net and prefer another selected page. Forward page
  references are valid; unchecked sheets cannot become dangling destinations.
  Invalid link destinations preserve an existing output file.
- Independent QA: ezdxf 1.4.5 read 23,922 faces with zero audit errors/fixes;
  independently rendered geometry/font details were compared with the canvas.
  pypdf verified both directions of a two-page VCC link; PDFium rendered both
  pages. Tests cover current unsaved project NET, native NET syntax/encoding and
  destination preservation, graphic DXF roundtrip, glyph counters, project PDF
  links and selected-page boundaries. Artifacts/logs: target/file-menu-qa.
- Required gates PASS: fmt check, strict workspace/all-target Clippy, 624 tests
  (app 134, core 252, geom 1, IO 130, render 107). Native build and Linux GNU
  all-target check PASS. A full example relink initially exhausted the disk;
  standard cargo clean for eda-app/eda-render/eda-io removed 59.1 GiB of generated
  artifacts, after which the complete workspace test run passed.
- Remaining scope is explicit in docs/features.json and
  docs/interface/schematic-file-menu.md: only Protel among the 20 NET formats;
  DXF graphic faces rather than electrical CAD transfer; native AutoCAD options,
  DWG and other CAD converters, separate authoring systems, Delphi API, BOM
  templates/variants and full dialog/icon/typeface parity remain incomplete.
  PDF drawing text remains rasterized. No claim of full File parity is made.

### 2026-11-01 — SCH Project / Validate

- Observed Project / Validate in Altium Designer Enterprise 24.4.1 through
  Windows Computer Use: project/free-document labels, ordered Project menu,
  Shift+Ctrl+F3/F4, warning-only compile preserving Properties, Messages columns,
  severity squares and bottom Details. ReferenceB produced two missing-driver
  warnings (NetC6_1, NetC13_2), one net-alias warning and Compile successful.
  A later Altium capture failed with `window crop is outside captured monitor`;
  reselecting its returned window did not recover capture. Observation limits
  and assumptions are explicit in docs/interface/schematic-project-validate.md.
- Added read-only eda-core project validation over shared compiled electrical
  islands. Checks annotation/duplicate designators, multipart physical pins,
  unconnected/single-pin nets, missing input drivers, pin electrical conflicts
  and net aliases; honors compile masks and generic No ERC. Native PrjPcb first
  eight ERC matrix rows/columns supply pin-to-pin severity. Validation reports
  aliases and continues; ordinary ECO/netlist compilation still refuses them.
- Current unsaved/new project sheets override disk, closed sheets are read
  without opening tabs, broken/unsupported compilation produces fatal messages.
  Messages has compiler columns, resizable columns/Details, tooltips/Copy and
  object navigation by double-click and wrapping Next/Previous shortcuts.
  Each run replaces its report and explicitly warns about incomplete coverage.
- Live Windows QA used maximized windows at the reference desktop DPI. Fixed
  submenu arrow positions and full-row hit areas, and disabled ScrollArea
  shrinking/minimum-content expansion in Details. Verified Validate, rerun,
  diagnostic double-click, Ctrl+Shift+F3 and warning-only Properties retention.
  On ReferenceB the GUI and read-only schematic_validate harness find the same three
  electrical problems observed in Altium, with natural-order driver pin lists.
  Reference PrjPcb and SchDoc SHA-256 hashes remain unchanged. Existing user
  PCBCraft/Altium windows were preserved; QA used its own portable executable.
- Required final gates PASS: cargo fmt --all -- --check; strict workspace /
  all-target Clippy; cargo test --workspace, 637 tests (app 138, core 260,
  geom 1, IO 131, render 107). Windows dev build and Linux GNU all-target check
  PASS. Artifacts/logs: target/validate-qa/{final-gates.log,ReferenceB-diagnostics.txt,
  reference-hashes-before.json}. Final executable: target/debug/pcbcraft.exe;
  a byte-identical target/validate-qa/pcbcraft.exe is used for the live review.
- Full Altium parity is NOT achieved. Native TypeN reporting levels, extended
  matrix categories, the remaining ERC rules, rule-specific No ERC, hidden-pin /
  repeated-channel compiler semantics, variants, live markers, full Messages
  context menus/filter/sort/export, asynchronous/cancellable compilation and
  exact messages/icons/fonts/pixels remain incomplete. Linux runtime/DPI was
  not visually verified. docs/features.json marks schematic.project_validate
  partial. No commit, push or publication was requested or performed.

### 2026-11-01 — SCH Validate continuation

- Continued native Windows Computer Use observation on a separate project copy.
  Confirmed Compiler source, exact success text, full alias object descriptions
  and repeated power-object counts, short Details pin names, Messages context
  menu order, Error Reporting groups and all 17 Connection Matrix categories.
- Extended core and PrjPcb matrix import to 17x17 (pins, ports, sheet entries,
  Unconnected); passive unconnected pins honor No Report. Added port conflict /
  No ERC / unconnected tests. ReferenceB's three electrical diagnostic strings now
  match the observed native strings, including Power Object GND (23).
- Messages supports multiple selection, column sorting, grouping, Copy,
  Save/Save Selection TSV and Clear All/Selected/Class. Unsupported specific
  No ERC, Export To Report and Print are disabled. Later application messages
  append with independent timestamps/source and preserve the compiler report.
  Details uses short natural-order pin names and the observed red error icon.
  Native grouping/export format and full object-tree parity remain unverified.
- Native TypeN mapping probes were inconclusive: Altium raised an index error
  loading controlled copies, then runtime error 216 and process exit. Original
  PrjPcb/SchDoc SHA-256 hashes were checked unchanged. User unsaved native
  session recovery was not verified. Computer Use restart left X2 running
  without a targetable window; capture recovery also reported monitor-crop and
  foreground-process-id errors. No per-rule TypeN mapping is claimed.
- Final required gates PASS: fmt check, strict all-target workspace Clippy,
  workspace tests (639: app 139, core 261, geom 1, IO 131, render 107), Windows
  dev build and Linux GNU all-target check. Test output is in
  target/validate-qa/continuation-tests.log. The QA executable matches the
  target/debug executable SHA-256. Native complete parity remains unfinished;
  docs/features.json keeps partial status and lists outstanding behavior.
- Live continuation QA verified Validate, short Details pin labels and red
  summary icon, ordered context menu, ascending/descending No. sorting and
  Save producing target/validate-qa/Messages-continuation.txt with five rows
  (three native-matching diagnostics, explicit coverage warning, success).
  Corrected Save to reuse the parented application FileDialog. Error summaries
  never claim no errors. Final gate log: continuation-final-gates.log.

### 2026-11-01 — Validate reporting levels; user stop checkpoint

- User restarted Altium and authorized continuation. Corrected the ignored QA
  generator to modify only Electrical Rules Check; earlier copies also changed
  Modification/Difference TypeN values. Corrected copies load and validate.
- Four native bit-coded probes and two independent inverse checks confirm
  Type10=no driving source and Type14=multiple names, with 0/1/2/3 mapping to
  No Report/Warning/Error/Fatal Error. Native Error/Fatal runs omit a summary.
  Observations: target/validate-qa/native-rule-observations.json.
- Implemented scoped TypeN import, unknown-level retention, invalid-level
  rejection, and the two confirmed rule mappings. Added section isolation and
  severity/suppression regression checks. Updated Error color and omitted
  synthetic error summaries. These newest UI changes have not had another live
  PCBCraft review; previous Windows GUI checks remain documented above.
- User explicitly requested stopping and committing all work. Native UI input
  and further feature work stopped; specific No ERC investigation remains
  unfinished. Full Altium parity remains partial. Commit preparation runs the
  required fmt/Clippy/workspace tests, plus native build/Linux check, recorded in
  target/validate-qa/stop-commit-gates.log.
- Commit checkpoint completed 2026-11-02: fmt check, strict workspace/all-target
  Clippy, all 640 tests (139 app, 261 core, 1 geom, 132 IO, 107 render), Windows
  build and Linux GNU all-target check PASS. Original user PrjPcb/SchDoc hashes
  checked unchanged. Full feature parity remains explicitly unfinished.

### 2026-10-28 — Blanket, Compile Mask and Parameter Set directives

- Observed AD Enterprise 24.4.1 live Properties: Blanket Border Width, Line Style,
  Border/Fill Color and Collapsed and Disabled; Parameter Set Location, Label,
  Large/Small style and All/Parameters/Rules/Classes; native Width rule dialog.
  Official reference: https://www.altium.com/documentation/altium-designer/schematic/directives
- Implemented typed core objects and validated edit/history lifecycle. Blankets
  support rectangular/polygonal outlines, concave containment, nonintersection,
  corner/edge resizing and perimeter-attached directives. Masks exclude complete
  objects and descendant sheets of masked Sheet Symbols. Collapse restores scope.
  The legacy JSON compile_masks field remains readable and follows typed edits.
- Added editor placement, preview, repeat placement, Enter/Backspace, border-only
  picking, collapse toggles, handles and pre-placement appearance/parameter drafts.
  Properties shows area-specific controls and Parameter Set filters/add/edit/remove;
  supported schematic constraints share the PCB constraint widgets. Placed masks
  fade excluded objects; an unfilled sheet border no longer intercepts area picks.
- Blanket net/component classes, component parameters, differential pairs and
  supported PCB rule families enter ECO. Multipart sections map to one physical
  component class member. Width rules affect route checks. Class membership is
  cleared when its Blanket is disabled; scope is determined from the same Blanket.
- Native SCH: 225 Additional, 211 FileHeader, 43 Parameter Set and hidden 41 rule
  children using the native broken-bar separator. ASCII/CFB create/edit/delete,
  coordinate quantization and owner remaps are verified. Additional WEIGHT must
  include newly authored Blankets; creating the stream requires its own header.
  AD reopening caught the missing-count issue that self-roundtrip did not detect.
- Live native research found legacy Blanket border width stored as LineStyle + 1,
  modern width as LineWidth (0..3), and LineStyleExt directly 0=Solid/1=Dashed/2=Dotted.
  Added regression coverage for all width/style choices; missing style stays Solid.
  Native PCB Classes6/DifferentialPairs6 preserve unchanged/opaque fields. Rules6
  creation uses only observed AD24 family tags; unknown families require a template.
- Real ReferenceG read-only comparison after multipart fix: 4 existing unavailable baseline
  rows, 0 enabled changes, about 2.4 s to compare. Logs:
  target/directives-ReferenceG-profile-final.txt. QA mask excludes C8, R17, R20, R21, R22,
  R24, TP168, VD3, VD4 and corresponding UART_PROG_MCU class members. AD24 live ECO
  showed the same 9 removals, 19 pin removals and class changes. No Execute Changes.
  Research copies live only under target; all 11 original source SHA256 hashes match.
- Native AD reopened target/directives-pcbcraft-qa2.SchDoc, displaying the authored
  polygon after Additional count correction; observed Dashed exposed and fixed an
  off-by-one in LineStyleExt. Final generated copy is directives-pcbcraft-qa3.SchDoc.
- Full parity remains open: X/Y reflection/protected locks, shared multi-selection
  Properties, complete ERC/No ERC filtering, bus/harness/multichannel scope,
  duplicate parameter names/diff-pair classes, class-generation/comparator options,
  removal reconciliation for all generated rules/pairs, exact visuals and Linux QA.
  Native ASCII PCB class/pair modifications and new rule creation remain refused.
  These are recorded as partial, not as completed commands or verified parity.
- Validation logs: target/directives-{workspace-current,core-verified,io-current,
  clippy-current,build-current,fmt-final}.txt. Latest full run before the final
  style regression: 568 pass, one pre-existing GPU routing dither pixel failure
  (frame 13, Rgba8Unorm, difference 4 versus tolerance 3). New IO style suite: 115 pass.
  Strict workspace/all-target Clippy and fmt check pass; final rerun follows below.

- Final style/Additional regression suite: 115 IO PASS. Final workspace run:
  569 PASS and the same pre-existing GPU pixel mismatch only. After the live Width
  dialog exposed clipped OK/Cancel controls, constrained the shared schematic
  constraint region and added an all-supported-families bounds regression.
  Final app suite: 116 PASS, strict all-target Clippy and fmt PASS. Build:
  target/debug/pcbcraft.exe. Logs: directives-{workspace-release,io-current,
  rule-layout,app-ui-verified,clippy-ui-verified,build-ui-verified,fmt-final}.txt.
  Live PCBCraft confirmed area Properties, mask collapse/unmask and exact Undo to
  clean state, Parameter Set values and opening the Width constraint editor.

- Follow-up real-window check: Width Constraint shows both OK and Cancel below
  the table; Cancel returns to the clean document without applying an edit.
- Added document-coordinate X/Y reflection of Blanket/Compile Mask while moving
  or dragging, composing with Space/Shift+Space around the cursor. Covered
  circuitry is never transformed. Preview and commit share a core transformation;
  one History transaction updates mask exclusions and Undo restores the old scope.
  Parameter Set reflection and the other parity gaps listed above remain open.
- Differential Pair ECO now preserves actual positive/negative net spelling and
  updates existing pair names case-insensitively while retaining opaque fields.

- Latest complete validation after reflection/pair-case fixes: 573 PASS, 1 existing
  GPU routing pixel failure (app 117, core 237, geom 1, IO 115, render 103+1).
  fmt and strict workspace/all-target Clippy PASS; debug build PASS.
  Logs: target/directives-{workspace-latest,clippy-latest,fmt-latest,build-latest}.txt.
- Final real Windows QA: Altium opened directives-pcbcraft-qa3.SchDoc and selected
  the authored polygon with Medium/Solid matching PCBCraft. New PCBCraft instance
  demonstrated M,M then X reflected preview, click commit and one Ctrl+Z returning
  the document to its clean state. Source circuit did not move. Exactly one live
  PCBCraft window left running on the QA copy. All 11 original ReferenceG hashes still match.


### 2026-11-02 — Synchronize local directive work with origin/main

- User requested fetching upstream updates, merging as necessary, committing all
  current work and pushing. Preserved the original local work in a temporary Git
  stash and fast-forwarded main from 91a890f to ca2a9f0 (six upstream commits).
- Reconciled area directives with upstream harnesses, Active Bar placement,
  simulation probes, File workflows and Project Validate. Kept both work logs and
  feature entries; unsupported/full-parity limitations remain explicit.
- Unified Parameter Set data with retained native child presentation. Child edits
  update typed parameters/rules atomically; typed edits retain/update children,
  native saves avoid duplicate parameters, and old JSON label fields are accepted.
  Regression coverage checks child editing/Undo and native field/rule roundtrips.
- Verification artifacts are under target/sync-qa. Final gate and real-window
  results are recorded below after the completed checks.
- Windows dev build, final fmt check and strict workspace/all-target Clippy PASS.
  Real-window QA was attempted through the Computer Use skill: launch_app twice
  returned `accessibility window-opened handler did not become ready`, and no
  PCBCraft window was returned afterward. No GUI parity verification is claimed
  for this merge. The existing Altium window was not manipulated.
- Final cargo test --workspace: 659 PASS, one previously documented GPU routing
  pixel failure: Rgba8Unorm, frame 13, dither=true, difference 4 versus tolerance
  3 (eda-render/src/tests/pcb_route_gpu_tests.rs:235). App 142, core 271, geom 1
  and IO 139 all PASS; render 106 PASS / 1 FAIL. All merge regression checks pass.
  No PCB GPU implementation/test files were changed by this merge. Full log:
  target/sync-qa/workspace-tests.log. Final strict checks: fmt-final.log and
  clippy-final.log; successful Windows build: build.log.


### 2026-11-02 ? SCH/PCB selection and navigation correspondence

- User requested native Altium controls, especially default left-button rectangle,
  right-button pan, component/wire/net selection and Alt/Shift variants.
- Observed AD Enterprise 24.4.1 using an authored fixture copy: component toggle,
  wire then segment refinement, directional frame/partial child text selection;
  inspected PCB Select menu without editing/saving user designs. Official rules
  and unverified native modifier combinations are distinguished in
  docs/interface/editor-controls.md.
- Shared SCH/PCB camera navigation, robust schematic press/release cancellation,
  default/additive frame, child text selection, electrical-island net highlights,
  memory and nudge/Undo implemented. PCB adds Ctrl-pad frame, Alt airwire choice,
  exact individual-copper crossing, overlap click cycle and physical/Tab expansion.
- Source/GUI validation is in progress; final results follow below. Full native
  preference/menu/marker/edge-pan/shove parity is not claimed; features.json keeps
  both controls features partial and enumerates remaining differences.

- Read native System/General and Mouse Wheel Configuration without changing
  settings. Left/Right selection is enabled; Ctrl-wheel zoom, plain/Shift scroll
  and Ctrl+Shift-wheel layer/channel mappings are confirmed in the installed UI.
- Restored native Ctrl-click PCB highlight and Ctrl+Shift-click cumulative nets;
  Ctrl-click no longer implicitly starts Cross Probe. T,C retains explicit
  continuous/jump modes. Ctrl+Shift-wheel now requests a layer change and does not
  leak an egui smoothed camera zoom. Schematic compiled channels remain absent.

- Real portable GUI: SCH default left rectangle selected four authored objects;
  S popup opened; Escape preserved the selection; source stayed clean. Maximized
  PCBCraft client 1920x1032 and AD client 1918x1030 use the same desktop DPI,
  but dock widths and exact marker/Fit metrics still differ.
- PCB GUI exposed an existing invisible-polygon hit: a drag through empty,
  unpoured polygon interior moved its definition instead of making a rectangle.
  Undid the authored-copy movement and closed without saving. Picking now uses
  polygon boundaries or actually drawn copper; crossing frames test boundaries.
  Added renderer/controller regressions, including unpoured/non-stale state and
  both directions. Final fresh GUI check and verification results follow below.

- Fresh extracted portable PCB check: left-to-right containing frame and
  right-to-left crossing frame select R1 without editing the model; empty
  unpoured polygon interiors no longer capture the gesture. Track click followed
  by Tab selects the two physically connected copper objects (track and pad).
  S menu layout is readable in the real window. No authored fixtures were saved.
- GUI also exposed egui's pending Tab focus traversal stealing subsequent editor
  shortcuts. Selection Tab/Shift+Tab now resolve that traversal before property
  fields register; a multi-frame real-egui regression checks the following S key
  and confirms that property text receives no input. Routing Tab retains its
  deliberate property editing mode.
- Final source checks: cargo fmt --all -- --check PASS; cargo clippy --workspace
  --all-targets -- -D warnings PASS. cargo test --workspace --no-fail-fast:
  677 PASS, one previously documented GPU pixel comparison FAIL. App 153, core
  273, geom 1, IO 139 PASS; render 111 PASS / 1 FAIL. The unchanged failure is
  Rgba8Unorm frame 13 dither=true, difference 4 versus tolerance 3, at
  crates/eda-render/src/tests/pcb_route_gpu_tests.rs:235. No test was disabled or
  weakened. Full log: .tools/interaction-tests-final.log.
- Final portable (release, locked MSVC x64) re-extracted and launched from TEMP
  with only Windows on PATH: Track -> Tab -> S opens Select immediately, Escape
  preserves two selected objects, Shift+S changes layer display without opening
  Select. Document title stays clean; both authored reference files remain byte
  identical to the fixtures. Data/app.ron is written next to the EXE. ZIP CRC,
  executable manifest hash and absence of smoke-test state in the archive PASS.
- Delivery: dist/PCBCraft-0.1.0-windows-x64-portable-2026-11-02-controls.zip,
  13,806,587 bytes, SHA256
  redacted-source-sha256.
  Sibling controls-verification.json and windows-portable-2026-11-02-controls-tests.log
  record the checks and known failure. Original dated portable archive preserved.

### 2026-11-02 — imported PCB query failures blocking routing

- Audited enabled scope expressions in 21 native PCB files available on this
  machine, read-only. Reproduced RouteSession startup failures on ReferenceD
  (InDifferentialPair), ReferenceE (OnTopLayer), and ReferenceC (HasFootprintPad). Native
  original file hashes are retained in .tools/query-routes-before.json.
- Added exact owned-pad footprint/name matching, differential pair membership
  from POSITIVENETNAME/NEGATIVENETNAME and native class KIND=6, explicit copper
  layer predicates, side/silkscreen/mid-layer predicates, hole shape/plating and
  DrillType scopes, Name Like/inequality, boolean-string comparisons, function
  whitespace, symbolic logical operators, XOR and flat long boolean chains.
  Existing OnTop/OnBottom now include their associated overlay/paste/solder
  layers, as documented by Altium. Unknown predicates remain errors.
- Rule resolution errors now identify rule name, kind and SCOPE1/2EXPRESSION.
  No rules are disabled, no generic fallback replaces an unknown scope, and
  imported native records are not rewritten by evaluation. Builder exposes
  the newly supported differential-pair/class choices.
- Six regressions cover scopes, native legacy literal pad identifiers, strict
  unknown rejection, layer/side distinctions and route preview/place/finish plus
  exact History Undo. The read-only profile tools gain --audit-queries, --pairs
  and --net. Source byte equality replaces a reimport equality check in the
  routing probe; exact loaded-model Undo remains independently asserted.
- Initial repeated audits: routing-related query errors removed from ReferenceD, ReferenceE,
  ReferenceC and ReferenceF. ReferenceG retains the ambiguous numeric-OR annular scope
  ((PadXSize_AllLayers or PadYSize_AllLayers)> HoleDiameter); no guessed numeric
  semantics are installed. Height/CompPinCount/component package predicates and
  unsupported constraint families remain outside the implemented evaluator.
- Final workspace, native routing and Windows portable GUI results follow.
- User clarified that the reported project is on another PC and unavailable
  here. Local reproduction is evidence for the exercised scopes, not a claim
  that the absent board has been tested.
- Final native probes PASS on ReferenceD (29,362 items, D_P), ReferenceE (41,409 items, GND)
  and ReferenceC (27,202 items, ExampleOutput): startup, nonzero preview, place, finish,
  exact Undo and unchanged source bytes/hashes. Existing STEP-import warnings
  remain on ReferenceD/ReferenceE; they do not originate in query evaluation. Results:
  .tools/query-routes-final.json. Audit before/after:
  .tools/query-audit-before.json and .tools/query-audit-after.json.
- Windows strict Clippy/all-targets PASS. Final workspace tests: 683 PASS,
  one unchanged pre-existing GPU dithering pixel comparison FAIL (Rgba8Unorm,
  frame 13, dither=true, difference 4 vs tolerance 3). App 153, core 279, geom 1,
  IO 139 all PASS; renderer 111 PASS / 1 FAIL. No tolerance/test was weakened.
  Full log: .tools/query-compat-tests-final.log. Portable GUI results follow.
- Final release portable GUI PASS on an authored byte-identical ReferenceE copy:
  Fit Board, select/zoom CAN1-H area, Ctrl+W, GND via, nonzero route preview,
  place, Escape finish. GND length changes 435.467 -> 436.452 mm. One Ctrl+Z
  removes the branch and restores the clean document title. Closed without
  saving; original and research-copy SHA256 remain unchanged. An automatic
  repour status appeared briefly after finish; Undo restored the original view.
- Portable launched from TEMP with Windows-only PATH, writes Data/app.ron,
  and passes ZIP CRC/executable-manifest/no-test-state checks. Delivery:
  dist/PCBCraft-0.1.0-windows-x64-portable-2026-11-02-query-fix.zip,
  13,817,186 bytes, SHA256
  redacted-source-sha256.
  Sibling query-fix-verification.json and windows-portable-2026-11-02-query-fix-tests.log
  record the results. The controls portable archive is preserved separately.

### 2026-11-02 - slow batch and online DRC

- Reproduced severe latency on read-only ReferenceE (41,409 items), ReferenceD (29,362),
  ReferenceC (27,202), and ReferenceA (982). Kept a separately built pre-optimization checker
  using the same query evaluator and stable serialized IDs for comparison.
  Initial unbuffered JSON loading was separated from measured DRC time; later
  measurements use direct buffered snapshots and time only checking.
- Prepared priority/scope/matrix caches, scoped per-item broad-phase gaps,
  cached bounds and complex-pour segment indexes, prepared board edges and
  identical per-item/layer annular reuse. Removed per-item full-board cutout scans.
  Spatial mask merging retains original merge/source order; exposed pad clipping
  is cached and far silk pairs do not consume the check budget. Unknown scopes
  keep conservative diagnostic evaluation. Connectivity skips already-joined nodes.
- Honor configured finding limits with ordered-prefix results and explicit
  truncation; exactly-N complete checks retain complete status. Later diagnostic
  phases can be unvisited only when the report is explicitly incomplete.
- Online checks now debounce the last edit/preview/settings change, cancel stale
  work/manual preemption/document closure, never overlap two jobs for a document,
  discard cancelled/racing results, avoid double cloning, and stop idle polling.
- Regressions cover priority/scope/error equivalence, duplicate IDs/probes,
  exact all-pairs copper/matrix/footprint behavior and report prefixes, exact
  segment-index distances with holes/islands, exhaustive silk mask/exposed copper,
  distant-pair budget preservation, concave/convex outlines, rare large-gap scopes,
  mid-check cancellation, stable online revisions and manual preemption/closure.
- Final timing/equivalence, workspace and portable GUI results follow.
- Optimization checkpoint before the subsequent Altium correctness comparison.
  These counts/timings and the following portable hash describe that checkpoint.
- Uncontended dev-optimized timings, excluding load, median of three
  at limit 500: ReferenceA batch 0.154 s / online 0.134 s; ReferenceE batch 0.381 s / online 0.387 s; ReferenceD batch 1.280 s / online 1.265 s; ReferenceC batch 0.594 s / online 0.586 s.
- Complete single passes at limit 50,000 all finish without truncation: ReferenceA batch 0.146 s / online 0.126 s; ReferenceE batch 8.802 s / online 6.655 s; ReferenceD batch 12.897 s / online 8.240 s; ReferenceC batch 8.137 s / online 6.707 s.
- Full original ReferenceA report is identical (277 batch / 226 online); original
  1.105/1.060 s vs new median 0.154/0.134 s. Exact prior-engine comparison
  also passes on 1/16 native ReferenceE/ReferenceD/ReferenceC samples in both modes. New full
  ReferenceD (30,847 findings) and ReferenceC (5,055) exactly match the intermediate
  exhaustive-geometry reports; original large-board jobs exceeded 180 s
  except ReferenceC online at 129.086 s (old work continued after report cap).
- Final Windows workspace: app 155, core 288, geom 1, IO 139 and renderer
  111 PASS; one unchanged renderer GPU dithering comparison FAIL (frame 13,
  Rgba8Unorm, 4 vs tolerance 3). Total 694 PASS / 1 FAIL. Fmt/strict Clippy PASS.
- Extracted release portable GUI PASS on the authored unchanged ReferenceE copy:
  Windows-only PATH/TEMP launch, Fit Board, online markers, manual 500 findings
  (43 unchecked, incomplete) and full 10,871 findings (44 unchecked, complete).
  Title stays clean; no native Save. Closed the test instance. ZIP CRC, EXE
  manifest hash and absent archived smoke state PASS; Data/app.ron is created.
- Delivery: dist/PCBCraft-0.1.0-windows-x64-portable-2026-11-02-drc-fix.zip,
  13,859,894 bytes, SHA256 redacted-source-sha256.
  Prior query/controls archives preserved. Dated verification JSON and interactive
  timing HTML are included separately in dist/. The reference checker retained
  the shared connectivity root-skip optimization; measured DRC-loop comparisons
  are conservative for that part. Linux GUI/performance was not exercised.

### 2026-11-02 - Altium comparison and electrical boundary checkpoint

- The optimization checkpoint above predates intentional correctness changes.
  On the byte-identical ReferenceE research copy, Altium 24.4.1 completes with zero
  violations twice (10 s and 8 s, Stop when 50,000). Source SHA256 remains
  redacted-source-sha256.
- Corrected circular annular-ring measurement, effective polygon nets including
  read-only outlines, filled-land electrical connectivity, rounded mask offsets
  and owned Net Tie copper. Poor-contact CHECKBADCONNECTIONS is explicitly
  unchecked rather than treated as satisfied by island connectivity.
- Independent native probes: complete combined report has 1368 findings,
  including unrelated ReferenceA-copy findings. The isolated copper pairs accept
  48.26 nm deficit and report 50.8 nm. Electrical copper-pair DRC now uses a
  measured 50 nm margin. Mask probes report a 2.54 nm deficit; mask, annular,
  outline/hole comparisons and routing rule values were not relaxed.
- Same-board ReferenceE findings fall from 10871 to 76, without truncation. Remaining
  differences: 75 mask slivers at 3-5 nm deficit, one 0.15 mm internal same-net
  via-pad pair against a 0.2 mm rule. Altium has zero; parity remains incomplete.
  A separate drc-reference-e-mask-isolated.PcbDoc contains eight translated ReferenceE mask
  pairs. Its native run is awaiting user assistance: capture became black and
  refreshed activation returned "failed to activate captured window". The
  preceding combined full run was launched successfully by Computer Use.
- Fresh idle dev-optimized full limit-50,000 batch/online DRC times, excluding
  import/deserialization and scheduling delay, single sequential passes:
  ReferenceA 0.136/0.124 s (93/42 findings, 21 unchecked);
  ReferenceE 7.781/6.029 s (76/76, 45/44 unchecked);
  ReferenceD 11.070/7.244 s (15525/15514, 17 unchecked);
  ReferenceC 7.569/6.237 s (61/61, 27/26 unchecked). No report is truncated.
  Other boards have not been compared with native Altium DRC; these counts
  establish timing only, not correctness of every finding. Native source hashes
  and the ReferenceE copy all remain unchanged.
- Fmt and strict workspace/all-target Clippy PASS. Workspace: app 155, core 295,
  geom 1, IO 139, renderer 111 PASS; same existing GPU frame-13 dithering FAIL
  (Rgba8Unorm, 4 differing pixels vs tolerance 3). Total 701 PASS / 1 FAIL.
- Updated release portable ZIP/import/dependency/license/CRC/manifest checks
  PASS. Archive has empty Data and no smoke state. Current binary has not been
  rechecked in a visible GUI while Computer Use activation is unavailable; the
  earlier scheduler/UI smoke observation above is historical, not this binary.
  dist/PCBCraft-0.1.0-windows-x64-portable-2026-11-02-drc-fix.zip,
  13,856,945 bytes, SHA256
  redacted-source-sha256.
  This is a reviewable checkpoint, not completion of the Altium parity stage.

### 2026-11-02 - project assembly variants checkpoint (user stopped)

- User requested full variant operation and schematic display, then explicitly
  requested stopping further implementation and committing/pushing the result.
  This checkpoint does not complete the full variant stage.
- Observed live AD Enterprise 24.4.1 Variant Management on the existing research
  project: component and parameter grids, variant columns, bottom actions,
  Projects selection and Editor/physical sheet tabs. Native stale VD7/VD9 link
  warning observed, not repaired. No native project/schematic Save performed.
- Corrected the actual ProjectVariantN import (legacy VariantN retained), native
  Kind 0/1/2, UniqueId hierarchy, ParamVariation/ParamDesignator and read-only
  .PrjPcbVariants symbol decoding. Native Desktop sources were read only.
- Added GUI-independent variant/variation/alternate models, validation, history
  and read-only assembly projections; not-fitted marks, varied parameter italics,
  special strings, pin-compatible alternate geometry and stable pin identity.
  Added manager CRUD/parameter/reset/history controls, project variant selection,
  physical sheet view/toolbar, independent project dirty/save handling, atomic
  external-change guarded Save and current-definition Save Project As. Print/PDF
  snapshots use the physical view within the documented channel restrictions.
- Core/IO/app regressions cover unchanged base documents/pins/wires/history,
  invalid-history atomicity, scoped UIDs, alternate pin mismatch, parameter
  precedence, native metadata/unknown-field roundtrip, Unicode Save As, external
  modifications, view cache/read-only guard, save/reopen and project Undo.
- Validation: cargo fmt --all -- --check PASS; strict workspace all-target
  Clippy PASS after fixing a new test initializer warning; dev app build PASS.
  cargo test --workspace: app 157, core 299, geom 1, IO 141, renderer 111 PASS:
  total 709 PASS / 1 existing FAIL. The unchanged renderer routing pixel test
  still reports Rgba8Unorm frame 13, dither=true, 4 pixels vs tolerance 3.
- Attempted a real-window smoke launch of an isolated newly built executable
  with its own portable state. Computer Use returned "accessibility window-opened
  handler did not become ready"; refreshed window inventory had no PCBCraft
  window. Final Windows UI behavior/pixel QA remains unverified; no success claim.
- Remaining work is explicit in docs/interface/schematic-variants.md and
  docs/features.json. In particular, newly authored alternate geometry is stored
  as PCBCraftAlternateDataN extension fields; native Altium alternate-library
  writing, complete multipart/Repeat/channel behavior, UID repair, reports,
  other assembly outputs and exact UI parity remain unfinished. No new portable
  release was requested or built for this checkpoint.

### 2026-11-03 - schematic editing latency

- User reported severe lag during schematic editing. Initial read-only CPU
  diagnostic on ReferenceCControl.SchDoc (125 components, 2162 scene
  primitives, 3476 parameter objects) measured Move/Drag/wire previews at
  112.638/171.288/172.268 ms. These are diagnostic means of five operations,
  not native window latency or a release benchmark.
- Removed repeated document-wide identity searches from preview differences,
  patch application, owner validation, parameter expansion and hierarchy/
  directive reconciliation. Added exact-integer R-tree candidate pruning for
  derived junctions and the changed-wire overlap closure. Existing exact
  geometric predicates, stable wire colour/order and unrelated native overlaps
  remain authoritative.
- A wider probe exposed remaining group Move latency (35/56 ms on Con/Pow).
  Added indexed selection roots and grouped extraction, shared with connected
  Drag. Selection/child ordering, hidden parameters, native text roles and image
  filtering are checked against independent single-object extraction results.
- All-component connected Drag then exposed a separate 171/220 ms Con/Pow
  path. Added spatial branch/moving-point/anchor queries, shared attachment
  collection and indexed stem-ID checks. Cleanup now rejects unrelated tips
  before cloning paths/styles. One-vertex imported wire branch semantics have
  a dedicated regression; existing exact overlap/topology tests remain active.
- Added a bounded revision/document/command preview cache. Unchanged previews
  stop scheduling frames; eventless follow-up paints no longer recreate UUIDs
  for placement tools. Core validation, atomic commits and Undo/Redo remain in
  the edit path. No Altium project/source file was changed.
- Added exhaustive junction-reference, transitive overlap/history, cached
  Undo/Redo/document isolation and real egui stationary Move/Place scheduling
  regressions. The read-only schematic_edit_profile example covers common
  editing operations and CPU frame tessellation; methodology and limitations
  are in docs/schematic-performance.md.
- Real-window verification attempted with the bundled Computer Use skill.
  Both list_windows and refreshed list_apps returned "Computer Use native pipe
  is unavailable: failed to connect native pipe ... (os error 2)". No current
  Windows visual/interaction success is claimed; Linux runtime is unverified.
- User interrupted after the parallel rebuild exhausted memory. No compiler or
  linker processes remained on inspection. PowerShell cleanup was rejected by
  automatic policy; standard `cargo clean --profile dev` succeeded and removed
  29.1 GiB of generated artifacts. Source projects and release outputs retained.
  Added `.cargo/config.toml` with one build job to bound compiler/linker
  concurrency on this 16 GB machine; subsequent verification runs serially.
- Serial dev app/probe build PASS after cleanup (cold dependencies: 32m 07s).
  Repeated read-only CPU corpus with no competing compiler: Con Move/Drag/wire
  11.030/12.224/8.633 ms; all-component group Move/Drag 21.683/39.271 ms.
  Pow (211 components, 5643 parameters) Move/Drag/wire 16.127/18.592/15.485 ms;
  all-component group Move/Drag 32.721/57.935 ms. Large connected group edits
  remain more expensive than single edits; no universal 60 fps claim. Updated
  raw report includes all operations; all four input SHA-256 values unchanged.
- Final validation: cargo fmt --all -- --check PASS; strict workspace/all-target
  Clippy PASS on the final Rust sources (before the interrupted parallel test
  build; subsequent changes only configure build jobs/document results).
  Serial cargo test --workspace --jobs 1 -- --test-threads=1: app 160, core 303,
  geom 1, IO 141, renderer 111 PASS; total 716 PASS / 1 existing FAIL. All new
  schematic regressions PASS, including the one-vertex wire branch case.
  Unchanged PCB GPU routing test still reports Rgba8Unorm frame 13, dither=true,
  4 pixels different vs tolerance 3 at pcb_route_gpu_tests.rs:235. Full command
  therefore exits 101; no tolerance relaxed or unrelated renderer code changed.
  JSON/feature-ID validation and git diff --check PASS. New executable is
  target/debug/pcbcraft.exe; 42.72 GiB free disk after the serial rebuild/tests.

### 2026-11-03 - hidden Parameter Set fields and text rotation

- User clarified that previously hidden data is appearing around Parameter Sets,
  and that text/parameter rotation seems reversed. Read the current PCBCraft
  ReferenceJ window without editing its unsaved document. Read-only native source
  audit found Small STYLE=1 Parameter Sets with ISHIDDEN=T ClassName=ExamplePowerClass.
- Follow-up clarification isolated the unwanted output to the literal parent
  label "Parameter Set", not the hidden ClassName values. The Small marker
  label fix directly addresses that reported case; child visibility is retained.
- The importer already retains the hidden child flag. The renderer nevertheless
  emitted the parent's stored "Parameter Set" label for Small markers. Small
  markers now omit that label; Large markers retain it. A primitive-level
  visibility guard prevents hidden fields from generating ink, bounds or hits.
- Read-only local Altium 24 configuration audit found DisplayStringAsRotated=1.
  PCBCraft previously forced orientations 180/270 back to right-reading 0/90
  and inverted justification. Text now uses all four document angles with the
  same local anchor; screen-space angles negate document angles because Y is
  downward. Large directive labels use the full orientation too.
- Removed the corresponding importer workaround which substituted Top Right
  for an omitted JUSTIFICATION at 180/270. Missing native enums now retain the
  zero Bottom Left anchor independently of the angle; explicit anchors survive.
- Added tessellated-glyph rotation checks against independent document-space
  rotation for all four angles and nine anchors; marker/hidden-field paint and
  hit checks; ASCII/CFB hidden-child rotation, copy, save/reopen and History
  coverage. Official references: Altium schematic/directives and
  preferences/schematic (Display Strings As Rotated). Exact new AD24 window
  comparison remains unverified: launch did not expose a targetable window.
  The two background X2 processes created by these failed attempts were stopped
  to release memory. Existing user PCBCraft session was left running.
- Validation: fmt PASS; strict workspace/all-target Clippy PASS; serial workspace
  tests 720 PASS / 1 existing PCB GPU routing FAIL (Rgba8Unorm, frame 13,
  dither=true, 4 pixels vs tolerance 3). All four new regressions PASS.
  Dev app build PASS; updated target/debug/pcbcraft.exe. To build without closing
  the user's dirty ReferenceJ session, its running executable/hardlink were retained
  as pcbcraft-before-parameter-text.exe; the standard executable path now holds
  the new build. No user document was closed or saved.
- Launched a separate portable QA executable under .tools/parameter-text-qa.
  First capture was black, activation reported "failed to activate captured
  window"; fresh selection/capture showed the Windows lock screen. Stopped UI
  input as required by Computer Use and requested desktop unlock. Current new
  window behavior remains unverified until the desktop is available. No current
  AD24 visual parity or Linux GUI success is claimed.


## 2026-11-04 - Run DRC / live AD24 continuation

- User requested continued Run DRC implementation with Computer Use comparison.
  Altium Enterprise 24.4.1.13 eventually exposed its main window after the user
  started it. Original projects and their unsaved documents were left intact;
  native experiments used ignored ReferenceB copies and independent probes.
- Full observations and rejected hypotheses: docs/pcb-update/DRC_COMPARISON.md.
  Native baseline is two violations versus initial 26 PCBCraft findings. Native
  mask-8/mask-17, Standard-component, Any Net, polygon-gap, collinear-midpoint and
  straight-edge ordinary-region controls were actually opened and run. Stop
  limit 10000 was not reached. A global 1 micrometre edge tolerance is disproved.
- Implemented shared exclusion of retained compatibility/body records from DRC,
  object-specific keepout clearance, native Track/Arc/Fill restriction masks and
  the corrected Arc modern-layer offset. Different Nets now excludes two No Net
  copper objects, while physical routing/pour obstacles keep their scoped gaps.
  Mechanical cutouts and keepouts continue to constrain unassigned copper.
- Text glyph/board-edge geometry is explicitly unchecked. Native extended text
  location and rectangle fields were investigated; no guessed font/baseline
  formula or blanket polygon tolerance was added to force equal result counts.
- Checker uses the observed 771 x 700 outer dimensions and a shorter body on
  small viewports; configuration-only rules are excluded, Return Path remains
  disabled, headers sort, Online/Batch have independent Used On / All On / All Off
  actions. Live QA identified label interception of the context menu; the menu
  is now attached to label responses, checkbox alignment and tree gutters fixed.
- Built a separate portable QA executable and opened the same source copy.
  Real window: native-sized checker and successful Run DRC, three violations /
  29 unchecked conditions. Matching keepout gap is 0.172657 mm. The two polygon
  edge findings and missing native text finding remain unresolved; full matching
  errors, native docked report/Message presentation, typography/icons and Linux
  GUI parity are not claimed.
- Existing live PCBCraft session was retained without closing/saving documents;
  its executable was retained under target/debug before rebuilding the standard
  path. QA data and captures remain ignored. No private project enters Git.
- Validation in progress: the first complete workspace attempt found two routing
  regressions from No Net rule selection and an obsolete exhaustive-test unwrap.
  Physical obstacle resolution and the independent reference loop were corrected;
  final checks and latest-menu GUI verification follow below.
- Final portable QA build: confirmed the checker at 771 x 700 in the real window,
  corrected tree gutters, centered checkboxes and the six-entry context menu.
  Online All Off clears Online while leaving Batch intact; Online Used On restores
  the checks with enabled definitions. Running Batch again completes with three
  violations / 29 unchecked conditions. The source document remains clean.
- Re-ran native DRC on the unchanged source copy: two violations, zero warnings.
  Original and local source copy remain byte-identical after all observations.
- Privacy follow-up: two newly generated local orchestration checkpoint commits
  had non-anonymous author metadata. Anonymized those local refs, expired obsolete
  reflogs/objects; reachable-history scan passes again. Main history is unchanged.
- Final validation: cargo fmt --all -- --check PASS; cargo clippy --workspace
  --all-targets -- -D warnings PASS (38.48 s); cargo test --workspace PASS,
  727 tests / zero failures (160 app, 308 core, 1 geometry, 144 IO, 114 render),
  including the previously intermittent GPU routing test. Full test build/run
  completed in approximately six minutes. Dev application build PASS (5m 47s).
  git diff --check and privacy scans, including reachable history and the new
  comparison document, PASS. Native text/polygon parity and unsupported rule
  coverage remain open; this validation does not establish complete DRC parity.

## 2026-11-04 - PCB floating Active Bar

- User clarified the requested upper PCB toolbar is the floating Active Bar.
  Used the Computer Use skill to inspect all fourteen groups in the live
  Altium Enterprise 24.4.1.13 window, including both filter palettes and all
  command menus. Offset, Rotation Angle and Align Objects dialogs were opened
  and cancelled. Original projects/documents were not saved or edited for this
  research; native probes used an ignored research copy.
- Observation, implementation, remaining limitations and primary documentation
  links: docs/interface/pcb-active-bar.md. Catalog has 92 commands in twelve
  command groups plus Selection Filter and Objects for snapping. Native row,
  separator, menu and strip dimensions replace the placeholder PCB toolbar;
  corrected title-strip height puts Active Bar at the observed y=82.
- Implemented 53 available command workflows. The other 39 remain visibly
  disabled, including advanced routing/tuning, dimensions and text frames.
  This is a partial implementation, not completion of the entire target scope.
  Original vector icons do not establish exact native raster parity. Linux,
  3D/board-planning variants and equal-size/DPI acceptance remain unverified.
- Added one-second hold/right-click menus, short-click last defaults saved in
  settings, keyboard navigation/dismissal, filter and snap state shared with
  Properties, and supported Custom / All On / All Off / Custom snap presets.
  Unsupported snap categories, Guides/Axes and Rooms/Other remain disabled.
- Pure core operations implement geometry-based region/lasso/touching selection,
  primitive/component arrangement, equal edge-gap distribution, four-track
  rectangles and center/edge/any-angle arc geometry. Keepout placements carry
  real restrictions and no net. Polygon cutout placement creates a standalone
  cutout region. All mutations validate and use document history.
- Offset uses unit-aware fields and two-point measurement. Rotation asks for
  the canvas pivot after angle acceptance. Align combines horizontal and vertical
  choices in one Undo entry. Modal workflows reject stale revision snapshots.
  Arrangement reuses the core transform on a private copy, avoiding cloning and
  validating the entire board once per selected object.
- Separate portable Windows QA build: observed filter 363 x 134 and snap palette
  425 x 177. Pads switches both popup and Properties; the supported snap preset
  cycle restores the original custom pattern. Ctrl+A selects nine demo objects;
  Align Left + Top applies together, and one Ctrl+Z restores the whole board.
  Native child clicks in filter/snap palettes through the capture helper dismissed
  their popup; native switch/preset interaction was not accepted as verified.
- Real-window QA found two layout issues: centered radio labels caused staggered
  alignment controls, and a long command menu inherited a shorter palette's cached
  height. Corrected left alignment/spacing and explicit per-menu dimensions with
  separate scroll state. Added regressions before the final rebuild.
- Initial strict all-target Clippy and fmt PASS. First Active Bar event suite
  (four tests) PASS. A new regression covers combined Align history, stale dialogs
  and native dialog dimensions; existing tests now cover settings-backed defaults
  and palette-to-taller-menu growth. Final workspace gates/build/GUI follow below.
- Privacy: scanned all seven new source/document files, zero findings. A newly
  generated local orchestration checkpoint had non-anonymous author metadata;
  anonymized that checkpoint ref and expired obsolete objects. Reachable-history
  privacy scan passes. No personal project, local installation path or capture
  is added to Git.
- Final regression found the close control expanding the frame by seven pixels.
  Title close now uses a margin-local interaction without changing content
  layout. All three observed dialog dimensions pass exact-size checks. Implemented
  Altium's undimmed modal background; inspected straight radio columns in the
  previous build. Final background/radio-inset adjustments have automated checks
  but were not recaptured after the user requested closing extra app instances.
- Final full workspace tests PASS: 735 tests, zero failures (166 app, 310 core,
  1 geometry, 144 IO, 114 render). Six Active Bar event/workflow regressions pass,
  including measured offsets before acceptance and after one-step Undo. Initial
  full compilation took 12m 09s; latest incremental test compilation 1m 13s.
  Strict workspace/all-target Clippy PASS (7.68 s); fmt check PASS.
- Latest real-window QA confirms Drawing Tools grows after the shorter snap
  palette and shows all eight rows, while Keepout shrinks correctly afterward.
  Rectangle inserts four tracks and one Undo removes the whole outline. Keepout
  Fill displays five restrictions and disabled No Net, places and undoes. Offset
  measures +8 / -4 mm from two canvas points, applies the shift only after OK,
  and one Undo restores all nine demo objects. Align's corrected 310 x 240 frame
  and straight radio columns were inspected. No user document was closed/saved.
- Final dev build PASS (1m 10s). On user instruction, closed all five test GUI
  instances, discarding only agent-created demo documents. Window inventory now
  contains one PCBCraft instance: the original user session, retained with its
  unsaved documents. Latest build remains on disk; the old user session was not
  restarted. Recorded the single-GUI-instance rule in AGENTS.md. Future GUI QA
  must safely replace that session rather than launch alongside it.


## Active Bar continuation: reference alignment and connection selection

- Observed through Computer Use in AD24 on independently authored fixtures:
  Center H/V asks for a selected primitive, Center/Center asks separately for
  both axes, cancelling the second keeps the first, and one Undo restores the
  completed operation. Each Align dialog opens with No Change on both axes.
  Increase Horizontal Spacing at a 5 mil grid leaves the leftmost fill fixed
  and shifts the following fills by one/two steps. User projects were untouched.
- Added four spacing commands and four physical/component selection commands
  (61/92 available, 31 explicitly disabled). Centers now use actual chosen
  references rather than the total bounding-box center. Confirmed axes survive
  partial cancellation and commit together in one validated history operation.
  Locked selected objects stay stationary; Move All Components Origin To Grid
  excludes components with off-grid pad centers. Native HTML reports, independent
  grid Step X/Y and maintain-spacing clearance solving remain unsupported.
- Physical selection stops at component pads; free pads remain traversable.
  Native synthetic probe reported five selected objects and highlighted the
  entire route through an intermediate free pad; the per-object list was not
  inspected. Single-layer and component selection result sets are supported by
  official docs and core/controller tests, with full native comparison pending.
  New selection pick modes remain active until cancellation; they do not edit
  the board or history. Shorted-net and selection preference parity is unverified.
- Added repeatable synthetic fixture author example. Source-free native binary
  output did not load in AD24; preserving a native-created empty source container
  while injecting our own fixture geometry worked. All native files/templates
  remain ignored research artifacts; no private board data added to source.
- Native capture failed twice with `window crop is outside captured monitor`
  while opening Panels. Stopped native input after failed window recovery.
- On explicit user instruction, closed the sole original PCBCraft session with
  Discard Changes. Window inventory confirmed no PCBCraft instances remain.
  The next GUI launch will therefore be the single new build, not a second copy.
- Strict all-target Clippy passed before the final controller regression.
  Core physical-boundary regression passed; full workspace gates are in progress.
  Privacy scan passes for all eight new files and all 1851 reachable files/objects.
  Anonymized one newly generated local checkpoint author/committer; no branch
  commits were rewritten and no user project/capture was added to Git.

- Recovered native capture by reading the open menu without images and
  dismissing it with Escape. Single Layer selects one Top track; selecting the
  Bottom track next replaces it with two Bottom tracks and the intervening free
  pad, while Top remains active. Corrected seed-layer choice and free-pad results.
  Component Connections prompts Choose component; the synthetic component pad
  did not resolve its owner and opened Component Designator. Result-set parity
  for component modes remains unverified; blank-canvas lookup is unsupported.
- First complete workspace run passed (740 tests: 168 app, 313 core, 1 geom,
  144 IO, 114 render). New controller regression and native single-layer findings
  are included in the final serialized fmt/Clippy/tests/build run now in progress.

- Native per-object verification completed through View > Panels > PCB List
  (avoiding the capture failure from the bottom Panels menu). Selected-only
  view confirms Pad/Track/Track on Bottom for Single Layer and Pad/Via/Track/
  Track/Track for full Physical Connection. The earlier free-pad interpretation
  is now verified by object kind and layer rather than selection count alone.
  Closed the research panel and exited the interactive command afterward.

- The final connection controller regression caught termination after a valid
  seed and incorrect right-click cancellation. Corrected the two branches:
  four new pick modes repeat after success, right-click always clears them;
  existing Net/Copper commands retain their prior one-shot behavior. No model
  changes were involved. Repeating the required complete gates after the fix.

- Final automated gates after the controller fix: fmt check PASS, strict
  workspace/all-target Clippy PASS (9.66 s), full workspace tests PASS: 741 tests
  (169 app, 313 core, 1 geometry, 144 IO, 114 render), zero failures. Nine Active
  Bar event/controller regressions pass, including repeat/cancel selection and
  two-reference alignment. Incremental test compilation took 1m 51s.

- First latest-build Windows QA: all Align spacing entries enabled, four maintain-
  spacing entries disabled, 310 x 240 dialog with independent radio columns and
  undimmed background. Confirmed first axis survives right-click cancellation
  of the second and one Undo restores the three original fills.
- Actual pointer QA exposed premature second-axis hover movement immediately
  after the first reference click. Native reference alignment moves on click,
  so removed the alignment hover transform and added an assertion that Y remains
  unchanged while hovering the second reference. Closed this sole QA instance
  before rebuilding; repeating gates and real-window reference picking.

- Post-hover-fix gates PASS again: fmt, strict workspace/all-target Clippy
  (9.12 s), all 741 tests (169/313/1/144/114). Test compile took 2m 47s.
  Alignment reference hovering now leaves the unconfirmed axis stationary.
  Existing Undo selection-clearing differs from native selection retention and
  is explicitly recorded; this phase does not claim full interaction parity.

- Final dev build PASS (2m 24s). Final real-window QA after the hover correction:
  first reference leaves three fills vertically separated, second reference
  applies only its axis and ends the command; one Undo restores all original
  geometry. Increase Horizontal Spacing shifts the second/third fills by one/
  two 0.127 mm grid steps and undoes as one operation. Short click successfully
  repeats the settings-backed last Align group command after restart.
- Final same-fixture Windows selection QA matches native result types:
  Physical Connection reports Track 3 / Pad 1 / Via 1, remains active, then
  selecting the second net replaces it with one Track. Right-click ends picking.
  Single Layer on a Bottom track while Top is active reports Track 2 / Pad 1,
  with no Via; right-click ends picking. No native PCB research file was saved
  or modified on disk. The sole final PCBCraft GUI remains open on the synthetic
  selection fixture; other test GUI instances were closed before replacement.

## 2026-11-04 — File-based component and library authoring

- Continued the original full library scope; this remains a partial checkpoint.
  Implemented checked library member/variant, metadata, pin and model commands
  in eda-core, independent of GUI/GPU. Active definitions reuse existing drawing
  and footprint editors, atomic validation and bounded Undo/Redo. Navigation is
  not a content edit. New geometry acquires its definition owner.
- Added SchLib/PcbLib tabs, File > New > Library, SCH/PCB Library panels, schematic
  model table, footprint primitive list/preview, pin placement, metadata/parameter
  properties, explicit pin/pad links, Make Available, placement and extraction
  from documents. Native-only unsupported commands remain disabled. Properties
  flush before Save and library navigation; no-op inspection remains clean.
- Independent native SchLib/PcbLib writers, safe backups and IntLib compilation
  now exist. Original native containers travel in JSON; unchanged library output
  is byte-identical. Modified unsupported auxiliary/reference data refuses native
  save before destination replacement. Scope/remaining gaps are recorded in
  docs/interface/library-authoring.md and docs/features.json.
- Computer Use reference: new synthetic SchLib/PcbLib only. Observed New Library,
  library panels, long-name SectionKeys, Model Map records 46/47 and footprint
  PrimitiveParameters namespaces. No original user project was edited or saved.
- Windows GUI: sole previous user session was closed without saving Controller
  under the user's explicit permission. Launched a single test build. Created a
  SchLib, placed a pin, checked Undo/Redo and saved GuiCreated.SchLib. Altium opened
  it and recognized the symbol/pin/electrical type. Current Rust native PcbLib
  opened in Altium with both pads, names and 2 mm sizes. Closed that test PCBCraft
  instance before replacing its executable; no second GUI was launched.
- Native interop investigation: missing Library/Data KIND/VERSION/BOARDVERSION
  caused Altium to reject PcbLib. Source-only IntLib extracted an empty package.
  Added Version.Txt, LibCrossRef.Txt and Parameters   .bin; native compilation of
  an isolated synthetic LibPkg established local Windows source identities and
  outer CFB V3 as further requirements. Corrected diagnostic V3 container extracted
  both source libraries successfully. Final production export verification is
  pending the current clean rebuild.
- First workspace run after the authoring additions compiled all targets/examples
  in 15m 23s. App/core tests passed; one new IO assertion failed because native PCB
  coordinates round to 2.54 nm and uniform pad stacks import as explicit layers.
  Replaced exact structural equality with bounded coordinate/stack semantics.
  Strict all-target Clippy passed before the latest UI persistence fixes. Final
  serialized gates and production GUI/native verification are still in progress.

- Final native production check PASS: generated production-final/Authoring.IntLib
  directly from the current Rust writer, without binary patches. Altium Extract
  created a package containing both source libraries; opening its SchLib shows
  PCBCraft_Test, U?, pins 1/2 and a resolved TEST_2PAD model with footprint preview.
  Shared Designator/Comment omit OwnerPartDisplayMode (its native byte field does
  not accept -1). The final writer now matches that observed representation.
- GUI PCB authoring PASS: New Library dialog is centered at approximately
  700 x 550, with the active bar hidden behind it. Created a PcbLib, placed a pad,
  inspected counts/primitive preview, ended placement and saved GuiFootprint.PcbLib.
  Corrected empty-list shrinking and the preview background after this inspection.
  Added automatic numeric pad numbering and a controller ownership/Undo regression.
- Final automated gates PASS: fmt check, strict workspace/all-target Clippy, all
  753 workspace tests (172 app, 318 core, 1 geometry, 148 IO, 114 render), zero
  failures. Last incremental test compilation took 2m 24s. Final build and Linux
  cross-check are running; latest executable window verification follows them.
- Final executable build PASS (2m 20s); final Linux GNU all-target type check
  PASS (5m 39s). Linux GUI execution and visual comparison remain unverified.
  Computer Use on the final executable: both native libraries open cleanly;
  PCB list retains its reserved height, lists two numbered pads and shows their
  preview; SchLib displays the symbol and persisted TEST_2PAD/Authoring.PcbLib
  link. Edit Model exposes the open footprint in Browse and the pin-map field;
  closing the dialog leaves the document clean. One PCBCraft GUI remains open
  on the synthetic SchLib/PcbLib pair; the generated Controller example was
  discarded under the user's permission. Full Altium parity remains incomplete.


## 2026-11-07 ? Project Git panel and semantic revision review

- User-requested independent Panels > Git left dock, outside the existing
  Altium parity scope. Consulted GPT-6 Astra with high reasoning and default
  service tier for dock width/chrome, history workflow and review semantics.
- Implemented bounded shell-free Git access in eda-io, semantic document
  comparisons in eda-core, and geometry/camera overlays in eda-render. Git tree
  filters supported native files by the explicitly selected project directory.
  History supports refs/tags, pagination, roots, merge parents and file history.
- Working tree reviews compare HEAD with saved uncommitted files. Added an
  explicit Include unsaved editor edits option using an immutable model snapshot;
  commit continues to require saved files. Revision reviews are read-only and
  cannot execute working-document Save/Undo/edit commands.
- Selected-file commit uses a private index, rejects stale HEAD/conflicts/active
  integration operations and preserves other staging. Push names the upstream,
  sends complete commits, supports first upstream setup and never forces.
- Added synthetic native-project/local-remote generator and regressions for
  selected commits, roots/renames/deletions, scoped paths, local push, native
  index/UUID changes, four semantic colors, library member unions and read-only
  document preservation. Initial workspace run compiled in 18m 54s; app/core/IO
  gates passed, one new render test lacked its named-font test setup. Corrected
  the setup and an egui corner-method typo in the new dashed-contour code.
- Strict workspace/all-target Clippy passed on the integrated implementation.
  Final rebuilt executable, full regression rerun and actual-window QA follow;
  results below supersede this intermediate checkpoint. Scope/limits are in
  docs/interface/git-panel.md and docs/features.json.

- Final automated gates PASS: cargo fmt --all -- --check; strict workspace,
  all-target Clippy; 767 workspace tests (175 app / 322 core / 1 geom / 154 IO /
  115 render), no failures; Linux GNU all-target type check (1m 51s). The last
  full test compilation took 4m 27s; the final GUI build took 3m 06s.
- GPT-6 Astra high/default inspected the actual 1440x900 Windows window read-only.
  Corrected all four findings: revision-aware active-file highlight, wrapped/
  collapsible/copyable property values, distinct empty/clean/search states and
  clearer commit subject/metadata hierarchy with reveal-on-selection. Further
  real-window corrections left-align file rows, keep loading in the fixed header
  and show physical property values in mm instead of raw model nanometres.
- Windows Computer Use PASS: independent Panels > Git; project-directory and
  PrjPcb-path selection; history and changed files; SCH and PCB overlay each show
  exactly one removal (red), translation (blue), parameter edit (orange), and
  addition (green). Old positions have dashed bounds. Selecting the SCH move
  fits both positions and draws its arrow; Before/After retain that camera.
- Working-tree QA PASS: a saved PCB width edit is orange against HEAD. Toggled
  Visible Grid in the synthetic open SchDoc without saving; Include unsaved
  editor edits exposed the buffer change with an explicit Editor buffer banner.
  Return + Undo restored the clean original model; native SchDoc stayed unchanged
  on disk. A guarded native-writer refusal in the first fixture generator was
  avoided by using independently authored ASCII 5.0 synthetic records. The final
  Rust fixture generator runs successfully without patches or user inputs.
- Final executable GUI commit/push PASS on the isolated synthetic repository:
  selected only Review.PcbDoc, entered a message and committed through the panel.
  The working diff closed, native change count became zero and outgoing count
  became one. Only Review.PcbDoc entered the new commit; an unrelated staged text
  file stayed staged. Push showed main -> origin/main and sent to a local bare
  remote; local/remote hashes match and outgoing/incoming counts return to zero.
  No source-repository commit/push or original Altium project changes occurred.
- The native folder picker eventually appeared after an initial capture delay;
  retained explicit path entry as a convenient way to choose a project. One final
  PCBCraft QA GUI remains open. External-remote authentication and Linux GUI
  runtime are unverified; semantic/native-format limits remain explicit in the
  feature registry and Git-panel guide. This does not claim full Altium parity.
### Git panel visual revision — 2026-11-07

- User rejected the initial Git interface. Reworked it directly with actual-window
  Computer Use inspection: native dock headers, compact repository toolbar,
  Changes/History tabs, flat timeline rows, file icons/status columns, bottom
  commit form, a separate searchable Changes/Properties dock, and compact
  comparison controls. Long property values expand and offer Copy by context menu.
- Removed persistent bounding boxes around every changed object. Selection alone
  shows contours and movement arrows. Unchanged artwork is repainted neutral gray
  to avoid confusing native red PCB layer colors with deletions.
- Windows GUI regression: history and file selection, SCH/PCB four change classes,
  movement focus, before/after values with mm units, independent commit checkboxes,
  saved working-tree diff, and selected-file commit. The local synthetic QA commit
  `80135d9c48d4bb2d27958f8e981a85aaa3f5ec64` contains only Review.PcbDoc;
  the pre-existing unrelated.txt staging remains intact. No production remote push.
- User supplied Bip_adaptive as the PDU project. Created an isolated copy at
  `local/PDU-git-review`; all seven source file SHA-256 hashes remain unchanged.
  Baseline `05d76c0239afbefd1bbbbf3958786feeddf8b95d` contains the original files.
  Authored native SCH edits through validated commands: R1 +5 mm, C1 220nF,
  R11 removal, C99 insertion, with connected wires updated. Board edits were
  authored in Altium on the copy: ECALL_SW width 0.3 to 0.4 mm and version text
  +2 mm. PCBCraft's guarded native PCB writer refuses this board's repour/reindex
  streams, so no writer guard was bypassed. Altium also changed dimension text
  locale, tiny coordinates, two regions and board metadata during save; these
  additional native changes remain visible, not silently discarded.
- Found and fixed native 1 nm relative-coordinate rounding that classified a
  translated SCH component as Parameters. A regression covers translation
  rounding while retaining an exact 1 nm width change as Parameters.
- Verified saved working-tree diffs in the real GUI: SCH seven semantic changes
  with all four colors; PCB twenty changes, including orange ECALL_SW width and
  blue translated label. Selected both files and created the second commit via
  the panel: `f0f9357136f0e2d481ee076766a2e3db718c7f06`. Only the two intended
  native documents entered the commit; Altium History backups stayed untracked.
- Committed SCH history comparison has the same seven changes as working-tree
  review; post-commit HEAD/worktree comparison reports zero for both documents.
  Fixed oversized SCH selection frames caused by hidden parameter geometry;
  hidden metadata remains compared. Added a focus-bounds regression and readable
  PCB net/text labels in the change list.
- Final Windows GUI validation on rebuilt binary: hidden-parameter frame fixed,
  SCH Before/After preserves camera, committed PCB history shows ECALL_SW width
  and version-text translation with readable net/text labels and property values.
  One QA GUI remains open on the PDU committed SCH overlay.
- Final gates PASS: cargo fmt --all -- --check; cargo clippy --workspace
  --all-targets -- -D warnings; cargo test --workspace (769 tests: 175 app,
  324 core, 1 geom, 154 IO, 115 render); Linux GNU all-target type check.
  Source repository changes remain uncommitted; no production remote push.

### Git revision document tabs — 2026-11-07

- Replaced the separate Git review workspace with ordinary read-only document
  tabs. Working documents and multiple SCH/PCB revisions coexist; matching
  immutable comparisons reuse a tab, and reopening a working-tree comparison
  refreshes its captured snapshot. Closing a tab removes only that comparison.
- Each revision uses the normal document View state, PCB layer strip, View
  Configuration, grid settings, mirror/single-layer controls and 2D/3D cameras.
  Common Ctrl+Tab / Ctrl+Shift+Tab cycling includes working and review documents.
  PCB view shortcuts are shared with the ordinary editor. Ctrl+Home fits the
  selected semantic change, including its former position.
- Layer/object visibility, transparency and dimming now affect the colored PCB
  overlay. Native 3D displays Before/After snapshots; semantic coloring remains
  a 2D feature. SCH grid toggling changes view state rather than snapshot content.
- Review tabs reject edit/save commands, use disabled placement toolbars and
  read-only Properties, and never participate in dirty-document saves. Existing
  tabs remain explicit captured snapshots after a Git commit.
- Added regressions for independent tab views, comparison identity/reuse,
  individual close, revision switching, working-document preservation and
  hidden PCB layers. Updated the Git-panel guide; final checks recorded below.

- Final Windows Computer Use verification at 1707x1019 on the PDU two-commit
  fixture: simultaneous working SCH, committed SCH diff and PCB diff tabs;
  Ctrl+Tab and Ctrl+Shift+Tab with search focus; individual Ctrl+W and middle-click
  close; reopen; independent camera and hidden Top Layer retained across tabs;
  shared View Configuration and layer strip; SCH G grid toggle; Ctrl+PageDown,
  Ctrl+Home, View > Fit Board; native 3D After and return to 2D overlay. The
  comparison toolbar fills the editor width. One QA GUI remains open with both
  revision tabs and the PCB layer controls visible.
- Final gates PASS: cargo fmt --all -- --check; cargo clippy --workspace
  --all-targets -- -D warnings; cargo test --workspace (772 tests: 177 app,
  324 core, 1 geom, 154 IO, 116 render); Linux GNU all-target type check.
  All seven original PDU file hashes are unchanged; test-copy tracked files
  remain clean. No source commit or remote push was performed in this stage.

### Persistent panels and working-file navigation — 2026-11-07

- Replaced document-dependent panel priority and independent SidePanels with a
  workspace-owned registry and one host on each side. Git, Projects, Library and
  View Configuration remain open as dock tabs; Properties, Components, Messages
  and Changes share the right host. Switching SCH/PCB/revision/Home changes panel
  contents, not membership, selection or width. Incompatible contexts have an
  explicit empty state. Layer Stack Manager also uses the common right host.
- Panels and View open or activate a panel. Re-selecting it does not toggle it
  off. Close, middle-click and tab context menus consistently close one panel.
  Removed dummy Navigator, SCH Filter, PCB Filter and PCB tabs and the inactive
  pin control. Restore Default Layout resets the common registry.
- Added Open working file to Git file context menus, revision-tab context menus
  and the review toolbar. Unchanged working-tree rows open their native editor.
  Working files and historical snapshots coexist; reopening a working file reuses
  its editor without losing unsaved changes. Review snapshots no longer appear
  as ordinary Free Documents. Changing repository preserves existing reviews.
- Added regressions for persistent dock selection/membership/geometry across
  document kinds, working-file identity and unsaved preservation, repository
  switching with open snapshots, and panel commands with an unapplied stack draft.
- Actual-window Windows checks at 1707x1019 on the existing two-commit PDU copy:
  simultaneous working SCH/PCB and committed SCH/PCB diff tabs; Open working file
  creates PCB or reuses SCH; Git history remains selected; View Configuration
  leaves the Git tab available; SCH/PCB/Home preserve both docks; explicit Git
  close/reopen retains state; selecting an already open Git in Panels keeps it
  open. Restart retains ordinary dock membership and selection.
- GUI testing found Layer Stack Manager rejecting panel commands. Moved dock
  navigation ahead of document command guards, preserving pending drafts, and
  gave its View menu the shared panel catalog. Verified Components opens through
  that menu, Messages remains present, and Properties returns to stack controls.
  Opening a file or comparison hides the manager without discarding its draft;
  asynchronous document imports can finish while that manager is in a background
  tab. Final build and verification results follow below.
- Final rebuilt GUI: Stackup -> existing working SCH -> retained Stackup ->
  committed PCB diff works without losing dock tabs. Revision-tab Open working
  file reuses the ordinary PCB; Git file-context Open diff opens SCH alongside
  both working files and the PCB comparison. One final QA GUI remains open on
  the PDU SCH overlay. No native design files were saved during this regression.
- Final gates PASS: cargo fmt --all -- --check; cargo clippy --workspace
  --all-targets -- -D warnings; cargo test --workspace (777 tests: 182 app,
  324 core, 1 geom, 154 IO, 116 render); Linux GNU all-target type check; GUI
  build; git diff --check. Feature registry JSON parses successfully.
  All seven original PDU hashes remain unchanged; disposable tracked files
  remain clean. Source changes are uncommitted; no remote push was performed.

### Assembly variant continuation ? 2026-11-07

- Observed installed AD Enterprise 24.4.1 Variant Management, Edit Project Variant,
  Parameter Properties, Edit Component Variation and Drawing Style at the same
  1707-pixel desktop width. The native project copy in local/variants-qa was the
  only project authored/saved for this research. Original SCH/PCB/IntLib hashes
  match the source copy; existing unsaved native Harness documents were retained.
- Added independent validated bulk commands, exact UID precedence, insensitive
  parameter names, explicit stale-link repair, bounded local history and active
  selection normalization after Undo. Unresolved links refuse assembly viewing,
  project output and BOM generation rather than applying ambiguous values.
- Reworked the manager around the two all-variant grids and six native project
  columns. Added child variant/component/parameter dialogs, unique cloning,
  Ctrl/Shift row selections, bulk states/parameters/reset, native column grouping,
  reports and local drawing-style drafts. Cancel preserves project and style.
  Parameter rename retains opaque native properties.
- Native evidence established Parameter2_1 for the first variant's global
  parameter and an ordinary SchLib companion .PrjPcbVariants for alternate parts.
  Implemented both, including Save As, existing-symbol/container preservation,
  conservative unsupported-data refusal, companion external-change protection
  and rollback if the project write fails. Private exact-geometry/UID fields
  remain alongside native content.
- The native roundtrip example generated NativeRoundtrip.PrjPcb and its companion.
  Live Altium exposed two interoperability defects: Win32 verbatim project paths
  and parameter records emitted before their component records. Fixed Save As
  path normalization and native record ordering/interleaved parameter pairs.
  A separate NativeOrdered project with normalized/copied document references
  and the exact generated library was opened in Altium: global AssemblyTitle
  retained; C1 alternate 1404 and parameter values 4-11-00008 / 10nF confirmed.
  Legacy ANSI tie-breaking now recognizes Cyrillic text without Cyrillic paths.
- Added configurable cross/mesh/masking/not-fitted text and changed-parameter
  color, bold, italic and suffix projections; detailed CSV and assembly BOM CSV.
  BOM refuses unsupported multipart folding and excludes Not Fitted/No BOM items.
- Full parity remains open: exact UI/DPI/icons/grid/clipboard, project-native
  drawing preferences, full multipart/Repeat/channels, placement, PCB 3D/assembly,
  fabrication/paste-mask, OutJob and ActiveBOM execution. See feature registry
  and interface/schematic-variants.md; no claim of full Altium equivalence.
- The user authorized closing/restarting PCBCraft. Saved the original unsaved
  group separately as local/variants-qa/SessionSnapshot.DsnWrk before closing.
  Built a separate review executable, closed each prior GUI before replacement,
  and installed the checked executable at target/debug/pcbcraft.exe.
- Additional native confirmation: the ordered QA copy displays C1 alternate
  1404, 4-11-00008, 10nF and its remaining parameters; selecting the variant and
  physical sheet displays the changed value in green. The unordered copy had
  blank parameter cells, so ordering is a verified interoperability requirement.
  Source-data/code QA copies and imported native observations remain distinct.
- Final Windows PCBCraft 1707x1019 QA: corrected shared component/parameter grid
  sizing, imported Cyrillic/alternate metadata, component child dialog, C2 Not
  Fitted, Undo/Redo, Detailed Report and GUI CSV export. GuiVariantBOM.csv contains
  37 fitted rows, C1 library 1404 / resolved 10 nF, and excludes C2. Manager OK and
  Ctrl+S saved the QA project and ordinary companion without error; native text
  contains C2 Kind=1. Physical view shows its red cross and C1 green modified
  value. SCH/PCB/IntLib hashes still match local/PDU-git-review. One updated GUI
  remains open on the QA copy; original design documents were not saved.
- Final gates PASS after the last source change: cargo fmt --all -- --check;
  cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace
  (789 tests: 184 app / 328 core / 1 geom / 160 IO / 116 render); Linux GNU
  workspace/all-target check. GUI review build succeeded. Source changes remain
  uncommitted and full Altium variant parity remains incomplete as listed above.

### Schematic variant appearance correction — 2026-11-07

- User reported that physical variant appearance does not resemble Altium. The
  previous functional QA did not establish a visual match. Re-observed installed
  AD Enterprise 24.4.1 with the disposable variant project, including No Variations,
  named assembly, red crosses on C2 and temporarily R9, Drawing Style preview,
  Gray Box and Mesh, native changed-parameter font dialog and Active Bar tooltips.
  Original design and existing unsaved Harness documents were not saved/closed.
- Corrected electrical-context masking; No Variations parameter masking; automatic
  not-fitted masking independent of graphics; cross bounds excluding all parameter
  strings; gray bidirectional thin mesh; changed Times New Roman 10 bold italic
  parameters. Alternate symbols retain sheet annotation geometry/visibility and
  compare against sheet text, rather than inheriting library annotation placement.
- Replaced the invented text strip with the observed five-slot 176px physical
  Active Bar. Commands pick a component on the physical sheet; Escape/right-click
  cancel picking. Added filter/select menus and regressions for variant-only picks.
  Choosing/editing an alternate reuses the transactional manager child dialogs.
- Found verbatim Win32 prefixes in companion-library identifiers after a normal
  save. Normalized library references through the same helper as document paths;
  extended the native roundtrip assertion. The separate VisualReference project
  probes the reference normalization without touching the already open QA file.
- Added logical/assembly scene JSON diagnostics (variant_scene example), annotation
  layout/font and cross-bounds regressions. A first test run found a stale raw
  expression in the long-label test fixture; corrected that fixture. Final gates
  and live-window verification follow below. Pixel parity remains an observation
  target; the implementation list alone does not establish it.
- Follow-up live QA found the original four-field stored style with dim=false;
  migration now recognizes that untouched legacy form too. Customized colors,
  fonts, suffixes and other style changes are retained. Also removed the 26px
  schematic top spacer: native Active Bar overlays the canvas, and the spacer
  shifted the page center. Fit Document in AD24 confirms the full working area.
- Windows QA of the rebuilt executable: No Variations masks context/parameters;
  named assembly shows C2 cross, masked context and C1 green changed value.
  The five-slot bar starts component picking; R9 toggles Not Fitted, Escape exits
  and Undo restores the original variant without dirtying the schematic. QA
  project changes were undone before restarting. Only the generated demo/group
  were discarded. Final executable remains open on VisualComparison.PrjPcb.
- Native reference observations remain separate: C2/R9 cross and mesh were
  observed on the QA project; C1 green value was re-confirmed on NativeOrdered.
  New shared-sheet project/cache probes are not additional evidence of complete
  native interoperability. Exact rasterization at all zooms/DPI and every
  selection-menu gesture remain unverified; the feature stays partial.
- Final gates after the viewport change PASS: fmt check, strict workspace and
  all-target Clippy, 792 workspace tests (185 app / 330 core / 1 geom / 160 IO /
  116 render), Linux GNU all-target check and native GUI build. SCH/PCB/IntLib
  hashes still match the source copies. Changes are uncommitted.
- Final post-restart Drawing Style inspection confirmed migration without manual
  edits: graphics width 0.508mm, Times New Roman 10, Bold and Italic checked.

### Git follows the active document — 2026-11-07

- Git now resolves the active document's loaded owning project before its source
  directory. Project-owned untitled documents use their new-project association;
  free untitled documents clear automatic context. Home retains the last browser.
  Opening Git explicitly rebinds it to the current document rather than the
  persisted or manually selected folder.
- Every Git review captures its original project directory in addition to the
  repository root. Switching to a historical or deleted-file review restores the
  original scope even when two projects share a repository or file names.
- Browser state is cached per directory, including history, search, commit draft
  and in-flight jobs. Background results stay in their originating browser;
  finishing a comparison in another project does not steal the active tab.
- Added regressions for nested project sheets/source paths, unsaved ownership,
  stale preferences, manual-folder reset, deleted scoped reviews, preservation
  of drafts and jobs, and delayed comparison completion after a project switch.
  Verification results follow below.
- Windows Computer Use on the rebuilt GUI at 1707x1019: Git opens on the active
  git-panel-qa-v2 project despite a persisted variants-qa folder; saved working
  PCB diff stays in v2. Opening git-panel-qa-v3 switches the panel automatically.
  Its committed PCB diff and v2's working diff coexist with identically named
  files; switching either diff restores the correct project and browser mode.
  The selected v3 commit remains selected after returning. Open working file
  creates the ordinary v3 PCB beside both diffs; selecting the v2 SCH returns
  Git to v2. No native documents or Git commits were changed during this QA.
- Final gates PASS: fmt check, strict workspace/all-target Clippy, 795 workspace
  tests (188 app / 330 core / 1 geom / 160 IO / 116 render), Linux GNU all-target
  check and native GUI build. Source changes remain uncommitted.

### Large Git diff rendering — 2026-11-07

- Observed the reported root PCB comparison open in the GUI: Empty -> 05d76c02,
  15,583 additions on the PDU board. Found a complete board scan for every change
  plus a separate egui interaction widget per change on every frame.
- Added immutable review artwork caching for both PCB and SCH, with a private
  font atlas and resident GPU meshes (software mesh fallback). Navigation reuses
  artwork; settled zoom refines text/strokes. Content, layer/display settings,
  filters, mirror, fonts and DPI invalidate the cache. Non-font image textures
  remain on the ordinary texture path. PCB outline/grid and SCH sheet/grid remain
  native, without painting and tessellating the whole model beneath the overlay.
- Visible change bounds are cached with linear ID-set visibility lookup. One
  canvas interaction picks changes, preserving hover/selection and avoiding stale
  hit boxes after pan. Unfiltered Changes lists no longer lowercase every label.
- Added the read-only diff_performance example, warm-frame root-comparison probes
  including egui tessellation and excluding GPU/presentation. On the saved PDU
  board (15,581 additions), baseline median was 171.92 ms vs native After 6.45 ms;
  initial optimized probe was 6.32 ms vs After 6.48 ms. PDU SCH (268 changes) was
  2.39 ms baseline and 0.21 ms optimized. Final probes and GUI checks follow below.
- Final warm-frame software probe: PDU PCB overlay median 7.54 ms (max 8.03 ms),
  native After 8.42 ms; PDU SCH overlay 0.30 ms (max 1.21 ms); synthetic SCH with
  20,000 added rectangles / 20,001 changes 19.10 ms (max 19.49 ms). These include
  egui tessellation, exclude GPU/presentation and ran alongside build activity;
  they measure CPU work rather than delivered frame rate.
- Windows Computer Use on the rebuilt GUI at 1707x1019: exercised the reported
  15,583-addition PCB comparison, pan, Page Up zoom, document Fit, canvas picking,
  Added filter off/on, Top Layer visibility off/on and native After -> Overlay.
  Returned to the same PCB tab after viewing both SCH comparisons. Verified the
  267-addition baseline SCH, pan/zoom/selection, and the seven-change native
  revision with red R11 removal, blue R1 movement, orange C1/wires and green C99.
  Working and diff tabs coexist. Only one PCBCraft GUI was running.
- Interactive GUI profiler batches logged CPU medians 2.52-3.18 ms and p95
  4.14-6.49 ms, with cold rebuilds/switches up to 129.74 ms. GPU presentation is
  not measured. First construction and changes to display/content still rebuild
  artwork; this optimization targets recurring navigation/rendering cost.
- Final gates PASS: fmt, strict workspace/all-target Clippy, all 797 workspace
  tests (188 app / 330 core / 1 geom / 160 IO / 118 render), Linux GNU all-target
  check and native Windows GUI build. Seven original PDU source-file SHA-256
  hashes remain unchanged. Source changes remain uncommitted.

### GitHub Releases update channel ? 2026-11-07

- User-requested release preparation for MX-Goliath/PCBCraft. Added an explicit
  stable-channel check immediately left of title-bar Search, with a compact
  rounded control, popup, received-byte percentage/bar, cancellation, retries,
  release notes, no-public-release and network/rate-limit states. The popup stays
  open while downloading. This chrome change is an authorized exception to the
  Altium visual target; ChatGPT Desktop similarity is approximate, not measured.
- Update service stays in eda-app, outside the document/history/geometry model.
  Background HTTP uses HTTPS and OS trust, semantic version precedence, exact
  native asset names and repository URLs, response/size bounds and streamed
  SHA-256 plus length verification. Only verified files become installable;
  error/cancel paths remove their temporary files. GitHub asset digests or
  exact-name checksum sidecars are required. No client token or private-account
  access is shipped. Checks happen on demand, without automatic downloads.
- Selection preserves Windows installer vs portable.flag and Linux DEB vs
  AppImage runtime APPIMAGE. macOS has separate arm64/x64 DMGs. Installation is
  explicit through the native installer/archive; portable, AppImage and macOS
  replacement instructions preserve settings. No unattended replacement/restart.
- Added native DEB, per-user Inno Setup and macOS app/DMG packaging, sharing Rust
  and font license notices with the existing portable/AppImage builders. DEB
  dependencies combine ELF analysis and dlopened libraries; AppImage Debian
  copyright lookup handles /usr-merged paths. macOS supports optional Developer
  ID/hardened runtime/notarization/stapling, defaults to ad-hoc signing. Apple
  credentials and Windows Authenticode are not provisioned.
- Four-runner CI (Windows, Ubuntu 22.04, Apple Silicon and Intel macOS) builds all
  six packages with SHA-256 sidecars; a complete-set/tag/hash gate precedes draft
  creation. Only publishing a public stable draft makes it visible to clients.
  Branch workflow_dispatch builds artifacts without creating a release. The
  CI channel derives from github.repository; local builds default to this repo
  and accept a compile-time PCBCRAFT_UPDATE_REPOSITORY override.
- Added 12 Rust regressions for distribution/version/channel/URL/hash selection,
  streaming, corruption/truncation/oversize/cancel cleanup and every popup phase;
  five Python regressions validate release tags, complete sets, tampering and
  DEB/macOS metadata. Python tests, script compilation, YAML syntax, tracked and
  new-file privacy scans passed. Final workspace and native GUI results follow.
- The preceding test GUI had an unsaved project group. Following the user's
  explicit instruction not to save, closed it via Discard Changes; no document
  save was performed. Only one PCBCraft GUI is used for the following checks.
- Native Windows build and final strict workspace/all-target Clippy PASS. The
  first full test attempt exhausted disk space while linking an existing IO
  example. Package-scoped `cargo clean -p eda-io --profile dev` removed only Cargo
  build artifacts and recovered about 15 GiB of physical disk space; the full
  test run is being repeated. CI disables dev/test debug symbols and incremental
  files to bound hosted-runner artifact size without reducing test coverage.
- Home, About and the startup log now use the Cargo package version rather than
  a hard-coded 0.1.0. GitHub connector metadata confirmed the expected canonical
  repository is private. Native Windows GUI at 1707x1019 shows the rounded
  control immediately left of Search; clicking runs a real anonymous request,
  shows Checking and then No releases yet with the private-repository explanation.
  Release checks and the popup do not change the document's dirty state. Actual
  release-package download/installation cannot be exercised before a public
  stable release exists; streamed download and every progress phase are covered
  by the automated regressions. Linux/macOS native builds remain CI work.
- Final gates PASS after artifact cleanup: cargo fmt --all -- --check, strict
  workspace/all-target Clippy, native Windows GUI build, and all 809 workspace
  tests (200 app / 330 core / 1 geom / 160 IO / 118 render), including GPU tests.
  Five release Python tests, script byte-compilation, workflow YAML syntax,
  repository privacy and diff whitespace checks also pass. Native popup retry and
  outside-click dismissal were verified; the update control stays left of Search.
  One new PCBCraft GUI remains open. No EDA documents were saved. Release/package
  publication and native Linux/macOS execution were not performed in this local
  Windows preparation task. Source changes remain uncommitted.
- Pre-commit review caught AppImage's local Arch-license variable shadowing the
  shared notices collector, causing UnboundLocalError before packaging on every
  platform. Renamed it and added a staging regression with native Linux tools
  mocked: it fails before the fix and passes afterward, checking license files,
  launcher, output and checksum. All six Python release tests pass. Rust sources
  are unchanged from the verified 809-test build. Commit/push requested by user.
