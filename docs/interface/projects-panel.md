# Projects panel — 2026-10-23

## Observed reference

Live Altium Designer Enterprise 24.4.1 on Windows, maximized at the current
system DPI; reference window 1918 × 1030 and PCBCraft 1920 × 1032.
Reference project: `private-projects/ReferenceA/ReferenceA.PrjPcb`.
No original document was edited or saved during observation.

Projects occupies the left dock through x=554. Header starts at y=60 with
23 px height, toolbar icons are centered around y=104, search spans x=26..539,
y=124..146. First tree row is centered at y=170; rows are 16 px high and each
nesting level adds 12 px. File names stay on one line. The active project has
an outline; selected rows have a blue background. Source files have type icons,
schematic sheet numbers and separate open/status marks at the right.

ReferenceA has Source Documents (two PCBs and one schematic), virtual ActiveBOM,
Settings → Output Job Files → BOM.OutJob, and Generated → Documents / Text
Documents. Free Documents is a separate group. Other already-open native
projects demonstrated Variants and nested schematic sheets.

## Implemented

The shared PCB/SCH Projects panel now uses that layout and tree grouping.
The project reader separates Document and GeneratedDocument entries, reads
variant descriptions and the virtual BOM flag, and discovers child sheets from
FileName records without importing their complete rendering models. Generated
files do not masquerade as editable source files. Open project documents are
excluded from Free Documents using their native source path.

Expansion and selection use stable IDs. Search retains matching ancestors;
clicking supported documents opens or activates them. Keyboard tree navigation
and guarded Close use existing document operations. Unknown compiler/VCS states
are neutral; unsupported editors, variants and destructive project commands
are disabled. Startup zero-sized frames no longer collapse the dock to 190 px;
the same dock width is retained across PCB and schematic editors.

## Verification

Windows dev build opened the original ReferenceA project. Actual window checks covered
source document activation, PCB/SCH switching, Settings / Output Job Files
expansion and BOM search with ancestor retention. Regression tests cover source
and generated metadata, sheet references, cycle/shared-child handling and the
zero-sized startup followed by editor switching. Workspace: 292 tests passing;
fmt check and strict Clippy pass.

## Remaining differences

This is not a claim of complete pixel or behavioral equivalence. Hand-painted
icons and egui rasterization are approximate; exact DPI and Linux runtime
comparison remain open. Native compiler/VCS states, complete context menus,
docking/autohide, DsnWrk persistence, project editing, OutJob/ActiveBOM editing
and variant execution are not implemented. Sheet hierarchy is inferred from
file references, not compiled multi-channel instances; each document appears
once and malformed/unresolved references fall back to a flat source list.
