# Document and editor strips — 2026-10-23

## Observation

Live Altium Designer Enterprise 24.4.1, original ReferenceA.PrjPcb,
ReferenceA.PcbDoc and ReferenceA.SchDoc; no source edits or saves.
Reference window 1918 × 1030, Windows display at the current desktop DPI.

- Document strip: 26 px, dark gray, green PCB / yellow schematic icons,
  file names, schematic project ordinal, Home Page. No separate permanent
  close button after every file. Right-click offers Close, close groups,
  split/tile/merge and Open In New Window.
- PCB strip: approximately 22 px; active-layer swatch and LS at left,
  two small scroll arrows, light text and 9 px colored squares.
  Copper layers carry stack ordinals. The observed ReferenceA sequence begins
  Top Layer, Bottom Layer, Mechanical 1, Top Assembly, Mechanical 7,
  Top Courtyard, Bottom Assembly, Bottom Courtyard, Top 3D Body.
- LS menu: All Layers, Signal Layers, Plane Layers, NonSignal Layers,
  Mechanical Layers, followed by user sets (work and TS in this installation).
- Schematic strip: Editor, then ReferenceA. There is no PCB layer strip.

## Implementation

`eda-app/src/document_bars.rs` provides the shared document tabs and both
bottom strips. Document switching, dirty markers and guarded close reuse the
existing session identity/unsaved-change flow. Middle-click close is also
supported; its native behavior was not independently verified.
The active document is revealed in the horizontally scrollable strip.
PCB layer selection uses the existing RouteLayer action, preserving routing
transitions. LS uses the display catalog, including configured empty mechanical
layers. Layer-set changes are view state and do not dirty the board. System
display categories (Connections, DRC, etc.) are not document-layer tabs.
The shell title and status heights were adjusted to align strip placement.

## Verification and remaining differences

Windows GUI checked with the same original project: schematic and PCB strips,
document switching, Bottom Layer selection, LS Signal/All Layers and arrow scrolling.
`cargo fmt --all -- --check`, strict workspace/all-target Clippy and 288
workspace tests passed; Windows development build succeeded. Regression coverage checks
hidden copper numbering, mechanical-before-standard order, excluded system
categories, configured empty layers and immutable model on layer-set changes.

Exact pixel parity remains partial: hand-drawn file/Home icons, tab edge and
font rasterization, overflow states, and platform DPI need further comparison.
Batch close, split/tile/merge/new-window commands and Manage Layer Sets are
disabled. Imported user layer sets and compiled schematic sheet views are not
implemented; the latter tab is disabled. Other editor toolbars, docking and
canvas differences are outside this change. Linux runtime not checked.
