# SCH and PCB controls — November 2, 2026

Reference: available Altium Designer Enterprise 24.4.1 on Windows. This maps
verified paths and remaining differences; it does not claim full parity with all
Altium settings and interactive commands.

## Observations in installed Altium

A separate `interaction-reference.SchDoc` was copied from
`fixtures/schematic/minimal.SchDoc` into `.tools`. User projects were neither edited
nor saved. PCB selection and menus were observed in an existing document;
geometry-changing commands were not run.

- Ctrl+PgDn zooms to the test schematic's object area.
- Left-drag from empty space creates a selection rectangle. A rectangle enclosing
  component labels but excluding its outermost pin selected child labels and No ERC.
  The parent had unfilled markers. Clicking the component body gave green markers.
- Another ordinary click on the selected component deselected it.
- The first click on a bent Wire selected the whole wire with green nodes.
  Another click on its vertical portion gave red segment markers.
- PCB: S opened Select. Observed entries included Select overlapped (Shift+Tab),
  Select next (Tab), geometric selection methods, All, Board, Net, Connected Copper
  (Ctrl+H), Physical Connection and additional methods.

Available Computer Use supports clicks, keyboard shortcuts and left-drag, but
provides no documented input for holding a modifier while using the mouse or for
right-drag. Therefore Alt/Shift/Ctrl mouse combinations below are supported by
official documentation and PCBCraft event tests; separate live verification in
AD 24.4.1 is not claimed.

## Rule sources

- [Schematic shortcuts](https://www.altium.com/documentation/altium-designer/shortcut-keys/schematic-editors)
- [PCB shortcuts](https://www.altium.com/documentation/altium-designer/shortcut-keys/pcb-editors)
- [Design object selection](https://www.altium.com/documentation/altium-designer/design-object-selection)
- [Route selection techniques](https://www.altium.com/documentation/altium-designer/pcb/routing/interactive)

Official pages are updated independently of installed AD24. Their rules are kept
separate from the version-specific observations above.

## Implementation

| Action | SCH | PCB |
| --- | --- | --- |
| Left-click object | Select; repeat click deselects a component or refines a Wire to a segment | Select; repeated clicks cycle overlaps, deselecting a single object |
| Shift+left-click | Add/remove without clearing others; Wire refinement is retained | Add/remove without clearing others |
| Left-drag from empty space, left to right | Only fully enclosed objects; child text is independently selectable | Full enclosure |
| Right-to-left rectangle | Intersection, including partially enclosed objects | Intersection with individual objects' copper geometry, not just their AABBs |
| Shift+rectangle | Additive; gesture may start over an object | Additive; does not start moving a component body over desired tracks |
| Ctrl+rectangle | Ordinary selection | Pads independently of parent components |
| Alt+left-click | Select pointed net object, highlight electrical group in open project sheets | Select visible airwire |
| Alt+Shift+left-click | No separate cumulative highlight mode claimed | Add to selected airwires |
| Alt+right-to-left rectangle | No separate mode claimed | Select visible airwires crossing the rectangle; Shift adds |
| Hold right button | Pan | Pan |
| Wheel / Shift+wheel | Vertical / horizontal view movement | Same; existing wheel-zoom preference retained |
| Ctrl+wheel, pinch | Zoom at pointer | Same |
| Hold middle button; Ctrl+right button | Zoom by mouse movement | Same |
| PgUp / PgDn, Home | Zoom at pointer; center view | Same |
| Hold left button on object | Move/existing connected Drag; preview until release | Group move, local track Drag, object handles |
| Alt while moving | Constrain displacement to one axis | Same |
| Ctrl+Shift while moving | Temporarily bypass grid and electrical snapping | Existing snap bypass |
| Ctrl+arrow / Ctrl+Shift+arrow | One / ten grid steps, one history command | Same |
| Ctrl+1..8, Shift+1..8 | Store / add to selection memory | Same |
| Alt+1..8, Alt+Shift+1..8 | Restore / add from memory; deleted IDs discarded | Same |
| Shift+C | Clear net highlight without model changes | Clear net highlight filter |
| S / X, S,A / X,A | Select/deselect menu; select/deselect all | Same, with a PCB command subset |
| Tab in selection mode | Existing editor behavior | Connected copper on layer → connected layers → entire net → original set; identical stages skipped |
| Ctrl+H | Not reassigned to a PCB command | Pick object to select physically connected copper |

Selection and highlighting live in view state. Electrical-connectivity and copper
selection geometry calculations live in `eda-core`; input and camera in
`eda-render`; every move goes through validation and history in the application.

Schematic highlighting uses the shared compiler's `electrical_islands`, including
groups without component pins and unnamed nets. Textual name matches outside the
resolved project scope do not substitute for electrical connectivity.
Closed sheets are read into snapshots without automatically opening editor tabs.
Complex unsupported compiler inputs produce a message.

Physical PCB selection accounts for layer copper, actual via spans, plated pads
and holes. An individual airwire does not connect copper islands.
Tab does not turn a component pin into selection of the whole package instead of
the net. None of these operations creates Undo entries.

## Verification and boundaries

Tests use real egui events, including press/move/release within one frame,
rectangle direction, Shift on press released before mouse release, type filters,
focus loss, wheel isolation from panels and zoom anchoring.
Controller tests cover memory, net highlighting without group selection,
Ctrl+arrows/Undo, PCB cycling, Alt+Shift airwires and Tab stages.
A separate multistep test checks Tab/Shift+Tab, property-field registration and
the next S key: selection expansion must not transfer focus into an input field.

The extracted Windows portable build was checked for an ordinary SCH rectangle,
enclosing/crossing PCB rectangles, R1 selection without document changes,
Track → Tab selecting a physically attached pad and displaying the S menu.
Empty space inside an unpoured polygon no longer starts moving its definition.
The build ran from a temporary directory with only Windows on PATH; settings are
created in Data next to the EXE. Strict Clippy and fmt passed; 677 tests passed,
one pre-existing GPU pixel-comparison test fails (difference 4 with tolerance 3).
Details and log: PLAN.md.

Remaining differences: not every dialog and preference is reproduced, including
Click Clears Selection, per-type Shift Click to Select, Use Left/Right Selection,
Always Drag and electrical hotspot; zoom-drag speed was not measured in AD24.
No complete native right-click context menu, automatic edge-pan, all Physical/
Component/Room selection variants or Ctrl+Shift+1..8 filtering.
The X menu supports All; its other regions are disabled. The PCB S menu has a
working subset. Exact markers for partially selected symbols, text-selection
geometry and every overlap edge case require further comparison.
Connected Drag remains a local solver without native shove/gloss.
This stage does not confirm full 3D-input parity.

Final check and live-window results are recorded in PLAN.md.
