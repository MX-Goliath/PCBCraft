# Schematic Properties — AD Enterprise 24.4.1

Observed October 20, 2026, Windows, 1707 × 1027 logical-pixel window.
Right dock: x=1390..1706, content x=1398..1693. Source: the live window,
not another version's documentation. ReferenceB already had an unsaved marker
before research; its source was neither edited nor saved. Additional states were
studied on a separate original ASCII sheet outside the user's project.

## Observed layout

Properties header y=54..78; context row y=86..110, with
`Components (and 11 more)` and filter buttons on the right. Search y=120..142.
Document Options has General/Parameters; Component has General/Pins;
Port has General/Parameters. Parameter, Designator, Text, Text Frame,
Net Label, Power Port, Wire and Rectangle have no such tabs.
Selection status is at the bottom above Messages/Properties tabs.

Sections: heading about 20 px, group background RGB 65,65,65, fields about 22 px,
ordinary row pitch 26 px. Labels are right-aligned; fields start about 117 px
from the content's left edge. Long labels are clipped without widening the dock.
Normal dock width is 316 px; Projects is 330 px.

| Context | Observed sections/fields |
|---|---|
| Document General | Selection Filter: All - On, 12 separate categories in four rows. General: Units mm/mils, Visible Grid + eye, Snap Grid + checkbox, Snap to Electrical Object Hotspots, Snap Distance, Document Font, Sheet Border + color, Sheet Color. Page Options: Formatting and Size, Template/Standard/Custom, Sheet Size + actual dimensions, Orientation, Title Block, Margin and Zones, Show Zones, Vertical/Horizontal, Origin. |
| Document Parameters | All/Parameters/Rules; Name/Value; user parameters followed by dynamic parameters; Add…/pencil/trash. Internally scrolling table. |
| Component General | General: Designator and Comment + eye/lock; Part/of Parts; multiline Description; Type; Design Item ID; Source. Location: X/Y and Rotation. Parameters: All/Footprints/Models/Parameters, then Links/Rules; models, parameters, Show More, links, Add…/pencil/trash. |
| Component Pins | Pins/Name/Show Full table; pin names and numbers, visibility icons; bottom lock/visibility/Add/pencil/trash row. |
| Parameter/Designator | Location: X/Y, Rotation. Properties: Component (link), Name, Value, name/value visibility; Autoposition, Lock Parameter, Allow Synchronization with Database; Font family/size/color, B/I/U/strike, 3×3 Justification matrix. Comment shows the original expression, e.g. `="R_USDD"`, rather than evaluated text. |
| Text | Location; Properties: Text, URL, Font, Justification. |
| Net Label | Location; Properties: Net Name, Font, Justification. |
| Power Port | Location; Properties: Name + eye, Style + color, Font. General (Net), Parameters (Net) below. |
| Wire | General (Net): Physical Name, Net Name, Power Net, High Speed, Differential Pair. Parameters (Net): All/Parameters/Rules/Classes, table. Vertices: Width + color, Index/X/Y, Add/trash. Smallest=101600, Small=254000, Medium=508000, Large=1016000 nm. |
| Text Frame | Location: X/Y only. Properties: multiline Text, Word Wrap, Clip to Area, Font, Alignment, Text Margin, Width/Height, Border checkbox/width/color, Fill Color. |
| Rectangle | Location: X/Y. Properties: Width, Height, Border width/color, Line Style; Fill Color checkbox/color, Transparent. |
| Port | Location: X/Y. Properties: Name, I/O Type, Harness Type, Cross Ref, Width/Height/Location diagram, dimensions, Font, Alignment, Border, Fill. General (Net), Parameters (Net). |

## Implementation and verification boundaries

This is an observation map, **not a claim of full parity**.
The implementation stores hidden parameters, original expressions, presentation,
component and model metadata separately from the historical source archive.
Properties edits the model through validated commands and Undo/Redo.
New JSON fields have defaults for old documents; the Altium archive stays original.

Observed standard sheet list: A4/A3/A2/A1/A0, A/B/C/D/E,
Letter/Legal/Tabloid, OrCAD A/B/C/D/E. Import, Properties and format labels share
one drawing-area table in nm rather than ISO paper sizes.
New documents use this table too: A4 Landscape is 292.1×193.04 mm rather than the
previous inconsistent 297×210 mm. Selecting any of the 18 formats preserves
orientation; the active format is highlighted. Changes use SheetProperties and
Undo/Redo. Arbitrary loaded-document dimensions are not rewritten automatically;
reselecting a standard format explicitly applies its dimensions.

The canvas cursor is drawn in eda-render: a 14 logical-px black crosshair with a
white outline, independent of the Windows pointer theme and document zoom.
It follows the mouse; tool-coordinate snapping remains separate.
The system pointer is hidden only over the active canvas; panels, dialogs and an
inactive window retain the ordinary pointer.

Show More expands parameters in source-object order. Initial Power/Resistance
favorites reproduce observed R9; this assumes an initial setting rather than
proving the setting for every Altium component. Stars let users change this list
in PCBCraft.

Still to confirm/complete separately: multiple selection and header type filters,
placement/move, Sheet Symbol/Entry, Pin, No ERC, Image, IEEE, arcs/ellipses/Bezier,
template/custom/title-block variants, model parameters, library dialogs and compiled
net metadata. Unimplemented actions must remain disabled. Strike, Clip to Area,
Text Margin and some additional flags still lack a complete model contract.
Dynamic document parameters, exact locking/autoposition semantics and alignment
of some tables also need separate work.

Linux uses the same egui widgets and Rust commands. Pixel, rasterization and
metric parity on Linux requires a separate GUI run.

The user's condition requires completing Properties before moving to
Components/IntLib; that transition has not yet happened.
