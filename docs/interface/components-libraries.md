# Components and local libraries — October 20, 2026 observations

Reference: running Altium Designer Enterprise 24.4.1, window 1707 × 1027 logical
points. ReferenceB.SchDoc is open. The user separately requested starting Components,
so the previous wait for complete Properties parity no longer blocked this stage.
Properties accuracy remains partial.

## Observed in Altium

- Right-hand Components panel about 316 wide, tab next to Messages/Properties.
- Top row: filter, library dropdown, menu, view switch.
  The list has a FILE-BASED LIBRARIES section, including ReferenceB.IntLib.
- Search; Design Item ID / Description / Value table, rows about 18 high,
  vertical/horizontal scrolling and Results: 31.
- In the supplied IntLib: Details with favorite parameters, Show More/Less,
  then a common Models section, References, Part Choices and Where Used.
  Previously observed separate Symbol/Model sections belong to another panel state.
- Menu: Footprints, Libraries Preferences…, File-based Libraries Search…,
  Refresh (F5). Opening the menu and selecting a library did not change the schematic.

## Implemented and verified

In PCBCraft, open a library through File → Open (Ctrl+O), or Components →
menu → Libraries Preferences… → Install…. The panel is available through View and
Panels → Components, and Place → Part… → Components. Installed paths are persisted
in PCBCraft settings and read on the next launch. Existing SchLib/IntLib references
in PrjPcb load when opening the project and are available from its tree.
Writing the Altium project document list is not implemented.

Search matches words against name, description and parameters; it is local search
without Workspace access. Details displays values and link descriptions.
Models previews the symbol, footprint information and model links.
Part and display mode are selected in the card context menu.
Double-clicking a row/card or selecting Place from the context menu starts
repeated placement on the active schematic.
Space rotates, clicking inserts, Esc/right-click finishes.
An unused designator is assigned. All insertions go through model validation and history.

Dragging a row or white card onto the free canvas inserts one component when the
left button is released. Only the preview changes before release.
Leaving the canvas, Esc, focus loss and document switching cancel the drag.
Drop checks the source session UUID and revision; a fast gesture within one frame
is also handled. One insertion is one Undo/Redo operation. The gesture uses egui
and eda-render coordinates, without Win32/OLE drag-and-drop.

The supplied ReferenceB.IntLib was used for verification: 31 symbols, including
7084 with four display modes. Rust reads it through CFB/zlib. Altium is not called
for import, preview or placement. SchLib Binary 5.0 is read directly; IntLib through
embedded SchLib files. Text records, binary pins, PinFrac, PinTextData, UTF-16
PinWideText, fonts, parameters, model links and image Storage are normalized.
Parts/modes become separate geometry variants. Owner-index conversion does not
mix the component with the synthetic sheet.

Instances contain their own UUIDs, parameters, geometry, fonts and images;
saved .pcbcraft.json opens without the original library. Native libraries are
not overwritten. Diagnostic CLI:
`cargo run -p eda-io --example library_inspect -- path/to/library.IntLib`.

## Current stage boundaries

Comparison iteration on October 20, 2026: at 1707 × 1027 with dock 316, the top
control order, search with active-filter row, 18-high table rows, Details heading
around y=559 and white 248 × 184 card around y=773 were reproduced for 1396 with
five favorite parameters. Columns sort and resize; the table/Details divider is
draggable. Collapsed Details remains above the tabs. Expanded parameters sort
without case/punctuation sensitivity; stars change favorites; Show Less retains
only favorites.

Observed: 1396 displays blue C? and 1005\0402, and gray original `="C_USDD"`,
rather than the evaluated Comment. 7086 (LMV321) shows DA?, `="Part Number"` and
input/output electrical-type arrows.
Implemented: separate presentation conversion for preview only; placed-instance
geometry and values are unchanged. Card scale is fitted to visible bounds with
fonts, excluding hidden parameters. Table Value is the Value parameter;
Comment is not substituted for it. The supplied IntLib has no embedded PcbLib:
the observed “Footprint is not present” is displayed. For other containers,
missing preview is not declared to mean a missing model.

Full pixel parity is not yet confirmed. Differences remain in font rasterization,
some icons/colors, scrollbars and standard egui controls. Other scales/DPI, all
symbols/modes, electrical markers beyond observed input/output, and the native
Libraries Preferences dialog have not been fully compared. Workspace information
text was partially reconstructed from a clipped native view;
References/Manufacturer Part Search/Learn More links are disabled.
Unsupported filters, view switching and Refresh are also inactive.
There is no library removal/update management, SchLib editor, instance update
from definitions or packing of multipart components into physical packages.
Part selection exists, but multipart package accounting/annotation remains future
work. Definition parameters cannot be edited through Components; placed-instance
editing remains in Properties.

No ASCII SchLib, DbLib/SVNDbLib, managed Workspace, server search, footprint/PCB
geometry or 3D preview; the corresponding commands are disabled.
Model links are metadata, not a completed footprint/simulation. Special pin-label
positions and pin-symbol widths do not yet provide exact visual compatibility.
Unknown visible records produce warnings. Support for every Altium library
variant is not claimed.

Windows GUI was checked manually: import, 31 rows, LMV321 search, placing 1396 as
C7 on a schematic copy, Escape, Undo/Redo and restoring the installed library
after restart. New iteration: dragging from a row and card onto the application's
own schematic, C1/C2, Undo/Redo, save/reopen; visual comparison of 1396 and 7086
with open Altium. Headless egui tests cover both sources, normal/fast gestures,
preview without model changes, cancellation, tab switching, stale revision and
collapsed-heading availability. Total: 43 workspace tests.
Linux GNU is checked by compiling all targets; Linux GUI and matching metrics
are unverified. The user's original SchDoc/IntLib files were not overwritten.
