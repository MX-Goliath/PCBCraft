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
- Make Available exposes a snapshot in Components. Place from SCH Library starts
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
DbLib, pad/via libraries, integrated project authoring, full library refresh and
update-conflict management, footprint wizards and reports remain disabled or
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
not unplaced sections/modes. Make Available is a snapshot and needs repeating
after further edits. Full multipart package placement/annotation remains partial.

## Verification journal

Required gates, current test totals and final real-window/native results are
recorded in PLAN.md. Synthetic files live under ignored
`target/library-observations`; no user project data is included in fixtures.

Final verification: 753 workspace tests, formatting, strict all-target Clippy,
Windows executable build and Linux GNU all-target type check pass. The final
executable was inspected through Computer Use with both native library tabs,
the footprint primitive preview and the model-link dialog. Linux GUI remains
unverified. Production IntLib generated by the Rust writer without manual
patches extracts both sources in Altium 24.4.1; its symbol opens with U?, pins
1/2 and a resolved TEST_2PAD footprint preview.
