# Altium interface observations

October 19, 2026, local UI through computer-use. Project files were not changed.
Screenshots were viewed in chat; no local screenshot archive was created.

Altium Designer Enterprise 24.4.1; title ReferenceB.PrjPcb.
Dark theme, English interface. The main captured image was 1707 × 1027 logical
pixels. System DPI, the build beyond 24.4.1, installed extensions and the complete
set of editors were not checked.

Home: File, View, Project, Window, Help. Projects on the left, Messages on the
right, document tabs at the top, Panels at the bottom right.

PCB: File, Edit, View, Project, Place, Design, Tools, Route, Reports,
Window, Help. Projects on the left; tabs above the canvas; Active Bar above the
workspace; a layer row at the bottom of the canvas; coordinate and snapping status
at the bottom. Messages and Properties share tabs in one right-hand dock area.
The initial shell opened Messages at the bottom; after clarification of the visual
contract, it was moved into the shared right-hand area with Properties and bottom tabs.

Properties without selection: Board, Search, General/Parameters/Health Check,
Selection Filter, Snap Options, Grids/Guides/Axes, snapping all/current/off,
objects for snapping, snap distance, axis snap range, Board Information.
Properties of selected pad rep1 were studied again without document edits:
Net Information; Properties with Designator, Layer, Net, Electrical Type,
Propagation Delay, Pin Package Length, Jumper; Template/Library; X/Y/Rotation;
Pad Stack with preview, Simple/Top-Middle-Bottom/Full Stack and
COPPER/PASTE/SOLDER tables. Electrical-field and stack values belong to the user's
document and must not be copied into PCBCraft's own board.

File in PCB: New, Open (Ctrl+O), Close (Ctrl+F4), Open Project,
Open Project Group, Save (Ctrl+S), Save As, Save Copy As, Save All,
Save Project As, Save Project Group As, Import, Export, Import Wizard,
Run Script, Fabrication Outputs, Assembly Outputs, Print (Ctrl+P),
Smart PDF, Recent Documents/Projects/Project Groups, Exit (Alt+F4).

PCBCraft uses Ctrl+F4 for Close (Ctrl+W is a PCB-only alias; in SCH it means Wire),
its own format and only the available command subset. Native Altium PCB/SCH
extensions are not claimed as supported.

Repeat comparison: both windows maximized, captured images 1707 × 1027.
System DPI and exact font metrics are not yet confirmed.
Visual parity status: `visual-parity.md`.

Repeat session: observed Projects width about 330 points, Properties about 316.
Title strip about 23, menu down to y52, document tabs down to y78;
Active Bar around x623..1097/y84..114. In Board Properties: Selection Filter
y194, Snap Options y356, Board Information y827. These are landmarks read from
images 1707 points wide, not pixel-analysis results.
Images in this session were 1019..1027 points high depending on frame/capture
state; full-height comparison is not considered normalized.

The installed DevExpress theme contains the name Segoe UI. The Windows version
of PCBCraft reads system Segoe UI and Semibold; WPF and egui metrics have not
been measured separately.

## Schematic: October 19, 2026 observation

Altium Designer Enterprise 24.4.1, ReferenceB.PrjPcb/ReferenceB.SchDoc.
Menus: File/Edit/View/Project/Place/Design/Tools/Simulate/Reports/Window/Help;
Active Bar above the sheet; Editor and the document tab at the bottom.
Properties without selection: Document Options, General/Parameters, Selection
Filter, General, Page Options, Margin and Zones. Visible grid 1 mm, snap grid
2.5 mm, document font Altium GOST Type A 10, Landscape, A4 292.1×193.04 mm.
Values were compared with file records. They do not imply verification of all
editor-setting variants. The project's minimal.SchDoc was opened in the reference
application and displayed its basic objects; the user's source was not saved
over the original.

## Schematic: Place menu, October 20, 2026

Observed in open Altium 24.4.1 without changing the schematic: Bus, Bus Entry,
Part…, Power Port, Wire (Ctrl+W), Net Label, Port, Off Sheet Connector,
Sheet Symbol, Sheet Entry, Device Sheet Symbol, Reuse Block…, Harness,
Directives, Text String, Text Frame, Note, Drawing Tools, Comment….
Drawing Tools: Arc, Full Circle, Elliptical Arc, Ellipse, Line, Rectangle,
Round Rectangle, Polygon, Bezier, Graphic….

PCBCraft preserves this order; unsupported features are disabled.
Part… currently offers its own R/C components; the separate Junction is a PCBCraft
addition. This observes labels/order, not every command and shortcut sequence.
