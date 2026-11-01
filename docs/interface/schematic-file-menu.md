# Schematic File menu

## Reference and status

Observed through Windows Computer Use in Altium Designer Enterprise 24.4.1 on
2026-11-01, with a schematic active. The supplied screenshot is an additional
reference. Native windows were inspected at the same desktop DPI; both main
windows were maximized for the menu comparison. This is a **partial**
implementation, not a claim of full Altium compatibility or pixel parity.

Observed File order:

1. New →; Open… (`Ctrl+O`); Close (`Ctrl+F4`).
2. Open Project…; Open Project Group….
3. Save (`Ctrl+S`); Save As…; Save Copy As…; Save All.
4. Save Project As…; Save Project Group As….
5. Import →; Export →; Import Wizard; Run Script….
6. Print… (`Ctrl+P`); Smart PDF….
7. Recent Documents →; Recent Projects →; Recent Project Groups →.
8. Exit (`Alt+F4`).

Separators, an icon gutter and submenu arrows follow the observed groups. The
native File popup is approximately 180 logical pixels wide with 22-pixel rows.
Save As has no shortcut caption in this menu; PCBCraft retains Ctrl+Shift+S as
an additional shortcut. Unsupported commands are disabled.

## Implemented operations

| Command | Behavior and limits |
| --- | --- |
| New / Project | Creates a local native PrjPcb. Uses a save dialog rather than Altium's Create Project dialog/Workspace service. |
| New / Schematic | Creates an independent ASCII 5.0 SchDoc source archive and editable scene. First Save adds the sheet to its associated project. |
| New / PCB | Uses the existing board model/editor. |
| New / Design Project Group | Replaces project membership after Save/Discard/Cancel protection; open document tabs remain available. |
| Open | Multiple documents; native project/group/library dispatch; duplicate open paths activate their existing tab. |
| Open Project / Group | Loads PrjPcb or DsnWrk, with group members validated before replacing the workspace. The actual Altium LastWorkspace file confirmed `[ProjectGroup]`, `Version=1.0`, `[ProjectN]`, `ProjectPath`. |
| Save / Save As | Existing verified SchDoc/PCB writers and atomic JSON writer. Save As changes the file name without destroying Undo; associated project document references are updated. Unknown native records can still cause a safe refusal. |
| Save Copy As | Separate destination; preserves active path, model, dirty state and Undo. Refuses to overwrite any open document/source, including itself. |
| Save All | Stops on first cancellation/failure. Also saves changed project-group membership. |
| Save Project As | Preserves unknown project sections, variants and options; rebases source/generated document references to retain their original targets. Does not duplicate all project source files. |
| Save Project Group As | Native DsnWrk references; group file name appears in Projects. Membership changes participate in Exit and replacement protection. |
| Import / DXF/DWG | ASCII DXF subset: LINE, unbulged LWPOLYLINE, ARC, CIRCLE, TEXT with quarter-turn rotation, POINT and SOLID triangles/quadrilaterals with true color. Units are converted to document coordinates. Unsupported entities fail before the single undoable Insert. |
| Export / DXF/DWG | Visible page linework, fills, symbols and font outlines from the shared schematic renderer, flattened to ordered DXF SOLID triangles in millimetres with true color. This includes pins, power ports, ports, multiline lettering and component graphics. Raster images, unknown objects and non-outline fonts cause a safe refusal. 250,000-face limit. No electrical semantics or native symbol library transfer. |
| Export / Netlist Schematic | Observed Export File then Export NetList dialog, Project/Document scope. Protel NET is implemented using native component records and physical pin/net records, CRLF and Windows-1251 as observed on the reference host. Uses current unsaved models and project hierarchy/net options. Parts share one physical component; compiler errors, ambiguous identifiers and unrepresentable fields refuse output before replacing the destination. Other observed formats remain disabled. |
| Import Wizard | Opens existing Altium/native JSON formats and libraries; inserts DXF into the active schematic. Does not include other CAD converters. |
| Run Script | PCBCraft declarative `*.pcbscript.json` commands; all commands validate together and produce one Undo step. No external interpreter, shell or Altium Delphi/API compatibility. |
| Print | Read-only Preview SCH, document/project drawing selection, paper/orientation, color/gray/mono, fit/actual scale, offsets/centering. Windows uses the system printer dialog and GDI; Linux uses CUPS `lp`. Printer properties determine the physical media. |
| Smart PDF | Current document or selected project sheets, preserving current unsaved scene snapshots. Lossless RGB pages, page/component/pin/label/port bookmarks and invisible destination links; optional component parameters/project-compiled net information. Pin links cycle through a net's visible physical pins, preferring another selected page. Compilation includes unchecked project sheets, but link targets stay within exported pages. |
| Recent lists | Separate bounded histories for documents, projects and groups; old combined settings migrate on startup. |
| Close / Exit | Existing document Save/Discard/Cancel protection, plus changed group membership on Exit. |

Model edits and scripts use eda-core validation/history. DXF/project/PDF
packaging lives in eda-io; page geometry, camera placement, glyph/image
rasterization and print preview live in eda-render. Altium is not an execution
dependency.

## Explicit gaps

The following observed New entries remain disabled: Draftsman Document, CAM
Document, Output Job File, Component…, Library…, Script Project/Unit/Form, and
AdvancedSim Model/Netlist/Sub-Circuit. These require their own authoring systems.
The observed Import entries Protel DOS Schematic and Add Importer… are disabled.
DWG/binary DXF are unsupported. Export entries Orcad SDT Schematic,
Orcad v7 Capture Design, P-CAD V16 Schematic Design and Add Exporter…
are disabled. No extension-plugin manager is provided. The Netlist format list
follows the 20 observed entries; only Protel Netlist is enabled. NET character
encoding follows the Russian reference host rather than changing with the OS
locale. Compilers' existing unsupported hierarchy/harness/multichannel cases
remain explicit errors.

Smart PDF uses raster drawing pages, so schematic lettering is not searchable
vector text. Bookmarks and destination links remain interactive. Resolution is
capped at 24 megapixels per sheet and selected-page data at 512 MiB. Net names
use the project's compilation settings, not a replacement for Altium's complete
project compiler. Excel BOM templates, variants, grouped net navigation, PDF option
families and Altium job/configuration formats are absent. Print/Smart PDF
dialogs do not have exact native layout, artwork or typography; icons are
independent vector drawings. Grayscale preview of some embedded/special-color
objects can differ from final raster conversion. Printing on real hardware and
Linux runtime/DPI parity are unverified.

Project Save As rebases document references, not every external path-valued
option. Group writing preserves membership, not all arbitrary workspace keys.
Native interoperability outside the documented subset is unverified.

DXF drawing output is graphic interchange: fonts and strokes become triangles,
not editable CAD text/line entities or schematic electrical objects. Curves and
glyph outlines are approximated; geometry follows the existing renderer, including
its cosmetic-stroke rules, page clipping and platform font fallback. Importing
this DXF produces independent filled polygons. Exact native AutoCAD exporter
options/layer/block mapping and pixel parity are not claimed.

## Additional observations: netlists

Computer Use confirmed the native Export NetList dialog (approximately 349 by
221 logical pixels), its Scope group (Project, Document), Netlist format combo,
and OK/Cancel controls. The observed formats, in order, are Cadnetix, Calay,
EDIF, EESof, Intergraph, Mentor BoardStation, MultiWire, OrCad/PCB2, PADS ASCII,
Pcad, PCAD, PCADnlt, Protel, Protel2, Racal, RINF, SciCards, Tango, Telesis,
WireList. An isolated Protel NET export under target/file-menu-qa confirmed
component bracket records (designator, footprint, comment, three reserved
fields), net parenthesis records and `designator-pin` endpoints. The original
schematic was not saved or edited for this observation.

The native Export to AutoCAD dialog was also inspected: Options includes AutoCAD
Version (initially R12), Format (DXF), Units (Imperial); Schematic Options contains
Include Template, followed by OK/Cancel. It was canceled without creating an
export. PCBCraft currently writes fixed R2004/mm graphical DXF and does not yet
reproduce that options dialog. Native file-dialog ownership/centering also differs.

## Script example

`fixtures/schematic/set-title.pcbscript.json` sets the active schematic's Title.
Run it through File → Run Script…; Ctrl+Z reverts the complete script.

## Verification

Regression coverage includes native authored-sheet Save As/second Save/copy,
sheet properties and reimport, project-reference relocation and unknown-section
preservation, group roundtrip, canceled/failed batch atomicity, DXF unit
conversion and rejection, raster output immutability, PDF bookmarks and invalid
destination preservation. Required workspace gates and live Windows QA results
are recorded in PLAN.md. QA logs/artifacts are under `target/file-menu-qa`.
