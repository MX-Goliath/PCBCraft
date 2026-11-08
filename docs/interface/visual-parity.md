# Altium Designer visual parity

Required target: an exact copy of the available Altium Designer Enterprise 24.4.1,
on Windows and Linux. Requirements come from the user and original plan, §7 and §25.
Exact parity is not yet achieved. Structure and density were rechecked in Board
and Pad states; the remaining differences below are open work.

October 19, 2026: both real maximized application windows were inspected again.
Captured images: width 1707; height 1019..1027 across frame/capture states.
Screenshots were displayed in chat. DPI was not measured; no pixel comparison was run.

| Surface | Completed | Remaining |
|---|---|---|
| Shell | Shared title bar 23 points; Quick Access, drag, double-click maximize, min/max/close, resize edges | Exact metrics/icons, Linux resize/drag behavior, system menu |
| Menus | Observed order, Home/PCB context, File, Ctrl+F4 | Complete contents and geometry of every menu |
| Projects | Left area 330 points, Search with vector icon, folders and PCB icons in tree | Exact toolbar set, header dropdown/pin/autohide, project model |
| Messages/Properties | Shared right-hand area 316 points, bottom tabs, active blue header | Full docking, exact header/tab states, pin/float, remaining bottom tabs |
| Board Properties | General/Parameters, disabled Health Check; Selection Filter; Snap Options table at 21 points per row; Board Information | Exact tabs/fields/spacing, selection-context dropdown, remaining selection/snapping types, guides/axis range and health engine |
| Pad Properties | Net Information, Properties, Pad Stack; Designator/Layer/X/Y/X-Size/Y-Size edit the application's model with undo | Electrical/net/propagation/template/rotation/lock and multilayer pad stack; an original rectangular pad is shown instead of physical 3D |
| Active Bar | Strip about 472×31 points, 13 original vector icons, Fit/Grid/Select | Exact pictograms, colors, states, placement/routing tools |
| Colors and density | Neutral gray groups/fields, borders; Windows Segoe UI, embedded Selawik Regular/Semibold | Colors chosen visually; WPF/egui rasterization and Linux metrics not yet measured |
| Linux | Same UI/layout, embedded fonts, successful all-targets cargo check of the entire workspace for Linux GNU | Native linking, tests and X11/Wayland runtime, DPI/input comparison |

Real Messages → Properties switches, Board/Pad states, X=12mm input from 10mm
applied with Tab and reverted with Ctrl+Z were checked.
fmt, clippy and six tests pass; the new Windows build was launched and inspected.
Final Board Properties landmarks: Selection Filter around y194,
Snap Options y358, Board Information y829; reference around y194/356/827.
These are visual estimates, not an exact-parity tolerance.

Snapping uses actual rectangular-pad centers/vertices/edges and track endpoints
with distance/layer/visibility limits. Unsupported table rows are disabled,
so their state differs from Altium.

Unsupported engineering commands are disabled. Their presence in menus does not
mean implementation. Geometry differences between the two current boards do not
compare the same PCB document: PCB import is still absent.

## Schematic import/viewer

The same ReferenceB.SchDoc was compared in PCBCraft and Altium 24.4.1.
The sheet/background, main symbols, wires, parameters and selected component in
Properties are reproduced. Quoted parameter indirection, IEEE dots in test points
and rotated text anchors were fixed.
The result is not yet an exact copy: text metrics/alignment differ; thin-outline/
wire rasterization needs further comparison; Active Bar is still shared with PCB;
port/IEEE/NoERC styles and sheet entries have partial support.
Electrical connectivity was not checked. No Linux visual run has been performed.
See altium-import.md for explicit native-record support boundaries.

### Repeat ReferenceB comparison — October 20, 2026

The full sheet and PgUp zoom (1.5×) were observed in open Altium 24.4.1.
Original long-wire geometry was confirmed with a zoomed fragment and an ASCII
copy. Some thin strokes disappear at full-sheet scale depending on zoom;
deleting these segments from the model would be incorrect.

The following confirmed differences were fixed:

- `Altium GOST Type A` is absent from installed families; Altium's family selector
  is blank and the sheet displays an ordinary sans-serif fallback.
  Substituting `GOST Type AU` produced visibly narrow labels. Family lookup now
  requires an exact name; fallback is Arial, then embedded Liberation Sans.
  Choosing Arial is an inference from the reference appearance, not a GDI GetTextFace measurement.
- IEEE Dot doubled in size; High Current (11) is an outlined triangle instead of
  a placeholder; LED-line endings, a separate pin-number font and name spacing
  are handled; Thin Cross NoERC was reduced. Vertical GND has no extra half-stem
  offset; Arrow power port has a closed triangle.
- Junction uses the geometrically adjoining wire's color. There is still no
  compiled electrical net.
- Shared Designator/Comment without OWNERPARTDISPLAYMODE are no longer excluded
  by an alternative graphics mode. This restores XP1 and its Part Number in
  ReferenceB; visible primitive count increased 579 → 581. Pin-owned parameters
  such as Net Name are retained as metadata rather than printed separately;
  a regression test covers this too.
- Restored the Editor footer, standard border with a 20 native-unit inset, zone
  dividers, system font and standard 350×80-unit title block with three areas,
  Size/Number/Revision, Date/File/Sheet/Drawn By.
  Placeholder `*` is not printed as a title value.
- With viewport x335..1385 and footer y971..995, paper occupies approximately
  x335..1385, y190..884 in both windows. This is an estimate from displayed
  captures, not an automated pixel-comparison report. PgUp/PgDn were added to PCBCraft.
- Axis-aligned stroke widths are rounded to physical pixels; antialiasing and
  a 1-point minimum remain for readability at fractional zoom. Fine grid is
  hidden below a 5-point pitch. The document grid pitch itself is unchanged.

Exact parity remains open: GDI/egui antialiasing, pixel-grid phase and thin-line
contrast, some label baselines/metrics, title-block fields and project special-string
behavior, Active Bar, Properties and other styles. Linux passed all-targets source
checks; Linux GUI runtime and DPI were not checked. Neither an absence of import
warnings nor matching overall sheet size is a criterion for exact completion.
