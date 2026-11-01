# Schematic Active Bar

Target: Altium Designer Enterprise 24.4.1. Status: partial; full visual and
functional equivalence is not established. The running editor is the reference.

## Observed reference

Live inspection on 2026-11-01 corrected the initial interpretation of the user
screenshot. There are fourteen groups: Selection Filter, Move, Select, Align,
Part, Nets, Power Ports, Harnesses, Sheet Symbols, Connectors, Directives, Text
Tools, Drawing Tools and **Simulation Sources**. All arcs and drawing commands
belong to Drawing Tools; the final gold hexagon is Simulation Sources.

At a 1707 by 1026 logical window the strip measures about 476 by 33 1/3 points,
with 33 1/3 point buttons. Popup rows measure about 29 1/3 points, separators
1 1/3 points, icon centers x=16 and text x=36. Observed popup widths include
Move 368, Select 161, Align 187, Nets 109, Power 227, Harnesses 159, Sheet 170,
Connectors 167, Directives 141, Text 115 and Drawing 148 points. Simulation
Sources gains "Simulation Generic Component" after its library is loaded;
the resulting eight-row popup is about 224 by 235 points.

The Selection Filter is an All - On button followed by a 3 by 4 grid. Short
click invokes the last command; right-click or a one-second hold opens a group.
The last command becomes the group's icon/default across tabs during a session.
The installed AD24/System/advsch.rcs supplies declarative captions/order;
no Altium execution engine or application code is used.

Computer Use through @oai/sky now works. Earlier missing-pipe/kernel-asset
errors are historical. Research edits were confined to the ActiveBarReference, ActiveBarImplementation
and ActiveBarFinal copies under target/routing-check, derived from the earlier
HarnessGui research file. A temporary Device Sheet folder registration was
removed after observation; its files were preserved.

## Implemented behavior

| Group | Behavior |
|---|---|
| Selection Filter | All - On and twelve object-type toggles |
| Move | Move/connected Drag, selection offset, rotations, owner-group stacking, Sheet Symbol flips and Sheet Entry operations |
| Select | Lasso, inside/outside, touching rectangle/line, all, connection and toggle |
| Align | Edges/centers, horizontal/vertical distribution, grid and combined dialog |
| Part | Components library panel and placement |
| Nets | Wire, Bus, Bus Entry, Net Label |
| Power Ports | GND, VCC, +12, +5, -5, Arrow, Wave, Bar, Circle, Signal Ground, Earth |
| Harnesses | Signal Harness, Harness Connector, Harness Entry, existing Predefined workflow |
| Sheet Symbols | Sheet Symbol/Entry; Device Sheet folder selector and reference placement; local Design Reuse |
| Connectors | Port and Off Sheet Connector |
| Directives | Generic No ERC, Parameter Set, Differential Pair, Blanket, Compile Mask |
| Text Tools | Text String, Text Frame, Note |
| Drawing Tools | Arc, Full Circle, Elliptical Arc, Ellipse, Line, Rectangle, Round Rectangle, Polygon, Bezier, Graphic |
| Simulation Sources | Voltage/Current Source, Simulation Generic Component library, Voltage/Differential/Current/Power Probe, Initial Condition |

Edits go through validated core commands and atomic history. New resources arm
placement without changing the document until a canvas click. Copying paired
differential probes preserves their pairing and creates a new pair identity.
Compile Masks exclude completely contained electrical objects and owned children;
collapsing the mask restores them. Parameter Sets retain owned parameters,
classes and supported PCB rule records. Differential Pair classification uses
the reserved child parameter, rather than the editable label.

Simulation sources are independent component/model data, with DC/AC magnitude
parameters and original geometry. The existing library importer can read the
installed Simulation Generic Components.IntLib; without it the panel offers
the three built-in source/initial-condition symbols. Probe result plots stay
empty until results exist; no simulator execution is claimed.

Graphic placement follows the observed first-click file selection, preview,
then placement sequence. Image input is bounded and decoded before insertion.
External and embedded images have distinct native storage behavior.

Device references carry SymbolType=Device Sheet, U_<name>, a file stem and
entries derived from child ports. Registered folder lists persist in settings.
Local snippets can be created from the canvas selection through
Snippets > Create Snippet from selected objects, then placed or opened through
Design Reuse. A new snippet contains only the selected graph with fresh IDs;
the original native archive and unselected content are excluded. PCBCraft's
local snippet catalog uses .snippet.json plus .pcbcraft.json. Native SchDoc
files can also be listed and placed from a registered snippet folder.

## Native persistence

The preserving SchDoc writer handles authored graphics, owner-group ordering,
Off Sheet Connector, Parameter Set/Probe and children, Blanket in Additional,
Compile Mask, source components/SIM model parameters, Device Sheet source kind
and external/embedded images. It verifies the saved data by reimport. Moving
an existing component requires all its native geometry to be represented;
unrepresented inactive parts/display modes explicitly block native save.
Unknown native index dependencies and cross-stream stack changes also refuse
save. Source-free SchDoc creation remains unsupported.

## Remaining differences

- Original vector icons remain approximations. Complete pixel, font, hover,
  pressed, placement-handle and dialog parity needs further comparison.
- Device Sheet multichannel annotation, read-only child semantics and automatic
  regeneration of entries are incomplete. Multi-port layout beyond the observed
  one-port example is inferred. FullPath uses registered folders.
- Native Altium snippet metadata compatibility, native thumbnail tiles and local
  snippet deletion are incomplete. PCBCraft's manifest is a separate format.
  Cloud Reuse Blocks and Managed sources are disabled without Workspace support.
- Non-DC stimulus editing, simulation execution, result plots, and transfer of
  every schematic class/rule into PCB compilation are incomplete.
- Text selection/alignment bounds approximate font metrics. Bezier bounds use
  the control polygon; some region tests use bounding boxes. Exact preference
  behavior, concave lasso containment and arc/ellipse editing handles differ.
  Alignment does not preserve attached wire connectivity.
- Elliptical Arc initially spans 0 to 90 degrees. Placement/editing preferences
  and all native default styles are not fully matched. Linux runtime/DPI
  comparison is outstanding.

Verification results are recorded in PLAN.md; generated logs and isolated QA
artifacts stay under target/routing-check/.

## Primary documentation

- [Active Bar and environment elements](https://www.altium.com/documentation/altium-designer/design-environment-elements)
- [Schematic placement/editing](https://www.altium.com/documentation/altium-designer/schematic/placement-editing-techniques)
- [Device Sheets](https://www.altium.com/documentation/altium-designer/schematic/design-reuse/device-sheets)
- [Reuse Blocks and Snippets](https://www.altium.com/documentation/altium-designer/schematic/design-reuse/reuse-blocks-snippets)
