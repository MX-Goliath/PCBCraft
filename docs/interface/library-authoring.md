# File-based library authoring

Reference: installed Altium Designer Enterprise 24.4.1, observed through Windows
Computer Use on synthetic documents only, 2026-11-04. The complete target remains
`ALTIUM_RUST_AGENT_PLAN.md`. This is a partial implementation, not a claim of full
Altium interface or format parity.

## Implemented

- File > New > Library creates an editable SchLib or PcbLib document. File-based
  libraries also open through File > Open. JSON preserves the complete editable
  library and the original native archive.
- Library member add, duplicate, rename and remove use checked `eda-core`
  transactions and ordinary Undo/Redo. Member selection changes the current view
  without dirtying the saved contents or adding a history operation.
- Schematic symbols have local geometry, pins, default designator/comment,
  description, parameters, parts and alternate display modes. Existing drawing,
  property and transform commands operate on the active definition.
- Footprints use the existing pad/track/arc/fill/text/region editors and checked
  property operations. New geometry belongs to the active footprint; library
  definitions have a local top-side origin and no nets or copper polygons.
  Library Options edits name, description, height and footprint parameters.
  PCB Library lists pad/primitive counts, selectable primitives and a preview.
- Footprint models can be added, edited and removed. Explicit pin maps support
  one logical pin mapped to multiple pad designators. ECO uses that mapping and
  refuses conflicting nets. Open authored footprint libraries are available to
  design synchronization before saving.
- Make Available exposes the open library in Components. Applied edits, Undo/Redo
  and Save As refresh its available definitions while the editor is open. Place from SCH Library starts
  placement in an open ordinary schematic. Make Library from Document extracts
  independently editable definitions from placed components.
- SchLib/PcbLib native output and IntLib compilation are independent Rust writers.
  Compilation checks footprint resolution and pin/pad mapping before replacing
  the destination. Writes use a same-directory temporary file and non-clobbering
  backup names. An unsupported native export fails before touching the target.

## Native format observations

- SchLib uses CFB FileHeader, per-symbol Data and Storage. Parts use OwnerPartId;
  modes use OwnerPartDisplayMode. Text and binary pins import independently.
  Long symbol names use the text SectionKeys mapping, observed after saving a
  synthetic long name in Altium.
- PcbLib requires the native Library/Data KIND, VERSION and BOARDVERSION fields;
  omitting these caused Altium's "Parameters version is not supported" error.
  After adding them, Altium opened the synthetic two-pad footprint. Long names
  use a separate binary SectionKeys table and 31 UTF-16-unit CFB section names.
  Footprint user parameters use the Footprint namespace of PrimitiveParameters,
  with a leading group count; observed directly after Add/Save in Altium.
- Native model records 46/47 encode explicit pin-to-pad maps with DESINTF,
  DESIMPCOUNT and indexed DESIMP fields. A one-pin/two-pad map was authored and
  saved through Altium to observe this representation.
- IntLib contains Version.Txt, compressed SchLib/PCBLib streams, LibCrossRef.Txt
  and Parameters followed by three spaces and .bin. The source-only container
  was recognized by Altium but extracted an empty package, so acceptance by our
  own reader is insufficient evidence of native compatibility.
  A separately compiled synthetic native LibPkg established that original source
  identities must be local Windows paths and the outer container must be CFB V3.
  A corrected diagnostic container extracted both source libraries in Altium.
  The writer uses deterministic logical C:\\PCBCraft source identities, including
  on Linux. The original source directories need not exist for extraction.
- Unmodified imported native libraries return their original bytes. Edited SchLib
  preserves unknown properties on represented objects and unrelated container
  streams. Unrepresented symbol record/auxiliary stream types refuse native save.

## Remaining scope and limits

Exact native layout, controls, icons, typography, all DPI states and Linux GUI
comparison are incomplete. Current dialogs are implemented in egui. Workspace,
DbLib, pad/via libraries, integrated project authoring, PCB-side comparison/ECO,
footprint wizards and full native reports remain disabled or
absent. IntLib is a compiled export/import source, not a directly editable tab.

Native export currently refuses embedded symbol images, footprint 3D resources,
unverified footprint-name encodings, and integrated simulation/other model
resources. Native PCB property/pad-stack/region limitations documented in
`../pcb-update/IMPLEMENTATION.md` also apply. Edited imported PcbLib auxiliary and
opaque primitive coverage is incomplete. JSON remains the lossless editing
format for unsupported native data; full binary round-trip compatibility is not
claimed.
Modified imported PcbLib files with unrepresented footprint auxiliary reference
streams refuse native save before touching the destination; unchanged files keep
their complete original bytes. Unrelated container streams are retained.

Adding a part/mode creates an empty geometry variant with shared metadata. A
library extracted from an ordinary schematic captures only each placed section,
not unplaced sections/modes. After closing an editor its last available snapshot remains until Refresh,
reopening or removal. Available File-based Libraries supports Refresh/Remove;
removal never deletes the library file. Refresh prefers the open editor over
the last saved file and retains the previous usable snapshot on read failure. Full multipart package placement/annotation remains partial.

## Verification journal

Required gates, current test totals and final real-window/native results are
recorded in PLAN.md. Synthetic files live under ignored
`target/library-observations`; no user project data is included in fixtures.

Previous checkpoint: 753 workspace tests, formatting, strict all-target Clippy,
Windows executable build and Linux GNU all-target type check pass. The final
executable was inspected through Computer Use with both native library tabs,
the footprint primitive preview and the model-link dialog. Linux GUI remains
unverified. Production IntLib generated by the Rust writer without manual
patches extracts both sources in Altium 24.4.1; its symbol opens with U?, pins
1/2 and a resolved TEST_2PAD footprint preview.

## File library refinement, 2026-11-08

Observed directly in installed AD 24.4.1 on the synthetic Authoring.SchLib:
SCH Library has one row per symbol, Description and expandable Part A/Part B;
Edit opens Properties. Tools includes New/Remove Part and Mode with
Previous/Next/Rename/Add/Remove and Normal/Alternate entries. New parts and
alternate modes are empty. Rename is disabled in Normal; native removal of
Normal was not exercised. The Pins tab
has independent designator/name visibility controls and Add/Edit/Delete.
A native alternate renamed to `Alternate QA` was saved separately under
ignored target/library-observations/authoring-20261108; its root record contains
`CustomDisplayModeName1=Alternate QA`.

PCBCraft conservatively protects Normal and the last part; exact native Remove
semantics for Normal are not claimed. Implemented checked part/mode removal
across all sibling variants, remaining
part/mode renumbering, shared component Type and mode names. Removed variants
and names return with one Undo; other symbols are untouched. Custom mode names
use observed native fields on import/export, and stale names are removed from
rewritten root records. Editable definitions retain U? rather than the placed
section suffix U?A, preventing repeated-save suffix accumulation. Browsing and
placing symbols still expose their section annotations independently.

The editor now groups the symbol list by component, provides section navigation
in Properties and mode navigation in Tools, searches descriptions, and opens
the library panel after creation. Pins offers individual visibility, selection,
edit and deletion; generic Pin Properties includes Hidden/Locked. Footprint
model Edit replaces its original row even when its name changes, preserving
other models, current flags and unrelated metadata. Invalid duplicate names keep
the naming dialog open; modal Cancel/Esc leaves the definition unchanged.

The model table previews uniquely resolved open or available PCB footprints;
unsaved open geometry takes precedence. Ambiguous references have no preview.
This is a 2D preview, not verification of all native 3D/model workflows.
Available definitions update from applied editor revisions, including history.
Refresh/Remove is implemented for installed file libraries; closed editor
snapshots, external resources and placed component updates retain the limits
above. Automated/native-window verification for this refinement is in PLAN.md.

Final refinement validation: fmt check, strict all-target workspace Clippy,
Windows dev build and all 827 workspace tests pass. Actual Windows GUI verified
new SchLib/pin/native Save, section/mode navigation, pin visibility and Undo,
model replacement and Undo, open-footprint preview and SCH/PCB Library titles.
Altium 24.4.1 opened the GUI-created GuiNew.SchLib with pin 1 and the separately
saved GuiVariants.SchLib with Part A/B, original graphics, TEST_2PAD and the
Service GUI mode. One PCBCraft GUI remains open; source projects were not saved.
The initial camera scale, dialog metrics, annotations and other unverified
visual details remain differences; this is not a full-parity checkpoint.

## Placed schematic updates — 2026-11-08

Observed in AD 24.4.1 using the isolated `library_update_probe` fixture:
Update From Library chooses sheets/types, then individual component actions,
then an Engineering Change Order. Advanced settings distinguish graphics,
parameters and models, adding/removing parameters and preserving annotation
locations/visibility. The Select Parameter Changes dialog can reject individual
values. The default update proposes replacing both 10k and an instance override
47k with the library's 12k. This is an observed replacement policy, not automatic
three-way detection of local overrides. `NativeUpdated.SchDoc` was saved
separately from the fixture baseline; user projects were not saved.

Implementation: Tools / Update From Libraries, Update Selected From Libraries
and SchLib / Update Schematics use checked `eda-core::library_update` previews.
The wizard supports per-sheet/per-instance selection, graphics/parameter/model
actions, parameter exclusions, addition/removal, annotation presentation and
lock-field settings, grouped ECO validation/execution and a copyable report.
Instance UUID/source index/native UniqueID, position, rotation and designator
survive. Pin IDs are matched by number and connection point; changed pin
connection points are reported before execution and wires keep their locations.
Locked parameters and disabled synchronization are respected. Direct SchLib
updates cover matching symbols throughout all open sheets and relink them to
that source library; ordinary Update From Libraries retains instance sources.

Open library definitions take precedence over disk. Resolution prefers the
recorded source and rejects ambiguous same-name matches. Input revisions and
file fingerprints protect the comparison. All target scenes are prepared before
any history is changed. Closed project sheets open as dirty tabs with Undo;
execution does not save source files. Compiled variant views are read-only.

Remaining differences: workspace/DbLib revisions, updating assembly variant
definitions, replacement via Choose Component, selective model merge, complete
native pin/opaque-record coverage, mirrored native graphical updates, complete PCB
footprint update coverage (see the direct PcbLib path below), automatic parameter positioning and exact
wizard/dialog/control/icon parity. Native components retaining inactive
parts/display modes refuse graphical replacement; parameter/model updates
remain available. Native
unsupported-data save guards remain in force. This remains partial.

Native verification refinement: hidden non-drawing parameter collection order
is reconciled only after UUID/ownership restoration. Drawable component order
is checked separately. Replaced component graphics are removed from the native
tree; new graphical children are written in model drawing order. This prevents
a newly written filled body from covering retained pin names in Altium.
Source library/Design Item ID and lock-field changes are encoded independently
of geometry. The first GUI output (`GuiUpdated.SchDoc`, v6) exposed the drawing
order defect and is retained as a failed observation, not accepted evidence.
The regenerated `RustUpdated.SchDoc` in v7 opens in AD 24.4.1 with P1/P2 visible
above the yellow body for both normal and rotated instances. Re-import confirms
U1 Value=12k, U2 Value=47k and Vendor=Example on both. Pin edits synchronize the
presentation name with the pin name. Direct Update Schematics commits the active
library Properties draft before taking the source snapshot.

Further observed UI difference: Update Selected From Libraries remains enabled
with no selection in AD 24.4.1. It opens an empty component-types wizard; Finish
shows an Error modal saying no changes were made to the project and leaves the
wizard open. PCBCraft currently reports an empty scope before opening the wizard.

Parameter-dialog refinement: AD 24.4.1 uses a 949 × 533 parameter matrix with
Object Type / Document / Identifier, a System Parameters group (Comment,
Component Kind, Description, Library Reference), and one column per user
parameter. Selecting U2 / Value and Reject Selected changes the displayed 12k
back to 47k; Update Selected and Update All become available. This was observed
without executing an ECO. The earlier vertical change list has been replaced
with this matrix structure, fixed-width scrollable columns, per-cell selection,
Update/Reject Selected and All, Show all parts, pending-change markers and
addition/removal colors. Protected cells cannot be accepted by bulk commands.
The component page now separates schematic/library columns and Actions, and
wizard footer buttons use the observed 75 × 23 size. Full source paths distinguish
component-type groups even when library filenames coincide.

The v7 GUI path verified matrix rejection, ECO Check/Done, Undo/Redo and separate
native Save As. Re-import of `GuiUpdatedFinal.SchDoc` confirms 12k/47k, Vendor,
two bodies/four pins and body-before-pin order. AD opens both visible pin-name
sets correctly. Its separately saved `GuiNativeNormalized.SchDoc` exposed a
second native-writing issue: Comment could be emitted both as an explicit
authored annotation and a hidden scalar, and a newly copied hidden Vendor object
could also be emitted a second time. Scalar parameter writing now accounts for
explicit visible/non-drawing presentation objects. Regression assertions count
the actual native Comment and Vendor records per owner, not only the final
parameter map. Existing imported duplicates are retained; there is no implicit
repair of user files. V7 is retained as intermediate evidence with this defect.
AD marked that intermediate file dirty on opening and added native defaults,
fonts and PinUniqueId records in its copy; the exact cause of the dirty marker
has not been established solely from those differences.

Final native/window validation uses regenerated v8 sources, not the intermediate
duplicates. The final GUI build completed the parameter-matrix rejection,
Validate/Execute, Undo/Redo and Save As path. `GuiUpdatedVerified.SchDoc` has
17 native records, two bodies/four pins, one Comment/Value/Vendor per owner,
Value=12k/47k, unchanged placement/rotation and body-before-pin order. AD 24.4.1
opened it without a dirty marker and visibly displayed P1/P2 and 12k/47k.
Direct SchLib Update Schematics immediately updated two components in one open
sheet; `GuiDirectUpdate.SchDoc` contains 12k/12k and Source=Parts.SchLib. One Undo
returned the saved 12k/47k sheet to a clean state. Final fmt check, strict
workspace/all-target Clippy, Windows build and all 844 workspace tests pass
(222 app / 340 core / 1 geom / 163 IO / 118 render). One PCBCraft GUI remains open.

Remaining controls include Return Selected to Default, Choose Component, native
column sorting/drag selection and the in-wizard Selected Parts Only toggle.
Unsupported commands are disabled. Selected-only updates are available through
their separate Tools command; exact native control/typography/icon/DPI parity
is still not claimed. Final real-window results and Rust gates are in PLAN.md.

Additional footprint observation: PcbLib / Update PCB With Current Footprint opens
Component(s) Update Options. Observed defaults: Text / On Any Layer off;
other primitives on silkscreen/mechanical/other layers on; solder/paste mask
expansion off; thermal overrides and footprint parameters on. A uniquely named
`PCBCraft_Update_6CF7B9E1` footprint was placed on a separate synthetic board,
rotated 90 degrees and given U1 / LOCAL_VALUE. Its library pad 1 was resized
from 2 × 2 mm to 3 × 2 mm. Applying the default options updated the open board
immediately, without a schematic-style wizard/ECO. The saved native result
retains position, rotation, component UniqueID `LRMUVKPG`, comment text and text
positions, while updating the pad and library height. Baseline and updated
files are retained under ignored `footprint-update-20261108`. No user board
shares that unique footprint name; user projects were not saved.

The observation also exposed an import defect: manually placed native components
without SOURCEDESIGNATOR lost their actual designator text. The reader now keeps
the native text and uses it as the component fallback; a populated source field
retains its existing precedence. A binary regression covers ASCII and Unicode,
untouched exact save and an unrelated geometry edit without rewriting Components6.
Byte-derived import UUIDs differ between native saves; native UniqueID is the
identity evidence, not those UUIDs.

Further AD 24.4.1 observation: replacing the library parameter `Parameter 1` with
`LibraryVendor=LibraryVendor` and applying Footprint Parameters removes the old
footprint parameter from the placed component and adds the new one. The separate
`NativeBoardParametersUpdated.PcbDoc` preserves that result. Enabling Text / On Any
Layer on the same pad-only library keeps the component's U1 annotation and comment.
Component annotation is therefore excluded from replacement of footprint text.

Implementation: `eda-core::footprint_update` validates replacements on a cloned
board and commits through History with revision checks. Geometry is normalized
to the library origin, mirrored for the placed side and transformed to its pose.
Pad names carry board nets; matched pads retain IDs/source records and local mask
settings unless the corresponding mask option is enabled. Annotation and external
tracks remain intact. Conflicting duplicate-pad nets, removal of connected pads,
locked or unsupported inputs and model conflicts reject the entire batch.
Placed components store `footprint_parameters` separately from schematic/base
`parameters`. Native import/export updates only the group with both
APPURTENANCE and VARIANTGUID equal to Footprint; the base, System and named-variant
groups retain their own values and opaque fields. Legacy JSON recovery resolves
both namespaces by native UniqueID without replacing local base values. The PCB
component Parameters tab shows Name / Value / Source, including Footprint rows.
The PcbLib Tools commands and panel context menu cover current/all footprints in
all matching open boards. The 424 × 458 options dialog follows the observed group
order. Source/target revision, path, open scope and pending edits are checked before
any history changes; there is no implicit save.

Intermediate real-window verification: Current / Cancel leaves the board clean;
All / OK changes pad 1, Ctrl+Z restores the clean baseline and Ctrl+Y restores
the update. The component Parameters tab shows LibraryVendor / LibraryVendor /
Footprint. `GuiFootprintNamespaced_v1.PcbDoc` opens clean in AD 24.4.1 and keeps
the native footprint/System/base parameter groups separate. It remains an
intermediate result: byte comparison found a verbatim Windows path prefix and
an old HEIGHT value left in Components6 instead of the library's explicit 0mil.
References now use the shared ordinary
Win32-path conversion. The native writer explicitly changes/removes HEIGHT and
FOOTPRINTDESCRIPTION and verifies both on read-back; the regression changes a
5.41 mm height to the library's absent/zero height while updating its description.
The Properties height display also prefers the component's HEIGHT metadata.
Final verification: fmt check, strict workspace/all-target Clippy, Windows build
and all 854 tests pass (225 app, 344 core, 1 geom, 166 IO, 118 render, including
GPU). The final GUI run shows baseline height 5.410 mm, updated height 0.000 mm,
Current update, clean Undo, Redo and separate Save As. The saved
`GuiFootprintVerified.PcbDoc` retains U1 / LOCAL_VALUE, position, 90-degree rotation,
native UniqueID LRMUVKPG, exact original Texts6 bytes and ordinary library path.
Pad 1 is approximately 3 × 2 mm, pad 2 remains 2 × 2 mm. Its base parameter table
is empty and its footprint table contains LibraryVendor. AD 24.4.1 opens this
final file clean; Properties confirms Height 0mil and the Footprint parameter
source. The intermediate v1 is kept as the recorded failed metadata checkpoint.

This remains partial: Thermal Relief Overrides is unchecked and disabled because
cross-archive thermal copying is not modelled. Matched native pads retain their
original opaque bytes. Full PCB-side comparison/ECO, native auxiliary/3D export,
all unusual pad-number changes, locked-update behavior and exact font/control/DPI
parity require more work. Rust gates and real-window verification for this stage
are recorded separately in PLAN.md.
Visible differences include checkbox size and section-bar colors, initial
footprint fit and library pad net labels; matching the dialog bounds and group
order does not establish full visual parity.

Supplementary primary reference (current documentation, not version-specific
visual evidence): [Updating Components from Database and File-based Libraries](https://www.altium.com/documentation/altium-designer/schematic/updating-components-database-file-based-libraries).
The direct PcbLib command scope is also described in
[Updating Footprints from Libraries](https://www.altium.com/documentation/altium-designer/pcb/placing-components/updating-footprints-libraries).
AD 24.4.1 live observations remain the reference for the interface states above.
