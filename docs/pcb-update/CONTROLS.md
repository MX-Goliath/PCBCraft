# PCB navigation and editing controls — 2026-10-22

This iteration repairs the PCB input path. It does not claim complete Altium
Designer 24.4.1 parity: no live Altium instance is available in this Linux session.
The existing layout is retained. Cursor dimensions, mouse preferences and every
native modal interaction still need matched-window/DPI verification.

## Navigation

| Input | Behaviour |
| --- | --- |
| Ctrl + wheel | Zoom at pointer, including smoothed wheel events |
| Trackpad pinch | Zoom at pointer |
| Wheel | Vertical scroll by default |
| Shift + wheel | Horizontal scroll |
| Right or middle button drag | Pan, including dragging beyond canvas |
| PgUp / PgDn | Zoom in / out at pointer |
| Home | Centre the view at pointer |
| Ctrl + PgDn or V,D | Fit all document objects |
| V,F or View > Fit Board | Fit the board outline, excluding off-board drawings |
| Ctrl + Home or View > Selected Objects | Fit the selected objects |

Preferences > PCB Editor / General stores Cursor Type (Small 90, Small 45,
Large 90) and Zoom with mouse wheel. Small 90 is the default, with a 14-point
cross rather than full-canvas lines. Large 90 is an explicit choice. Normal
system cursors remain over panels; panning uses a hand. If snapping puts the
cross outside the visible canvas, a system crosshair keeps the pointer visible.
Changing navigation preferences does not reset an imported board's snap grid.

Ctrl-wheel was previously read from egui's smooth_scroll_delta, although egui
routes it into zoom_delta. The canvas now uses both streams without applying a
Ctrl-wheel event twice. Pinch and residual zoom frames after releasing Ctrl use
the same pointer anchor. Navigation is limited to the canvas; fields retain
keyboard navigation. Camera scale remains within 0.001–100000 logical px/mm.

## Editing gestures

- Click selects; Shift-click adds/removes. Double-click opens Properties.
- Left drag moves an object/group; Ctrl-drag uses the existing local connected
  drag implementation. Shift+Ctrl bypasses snapping.
- A selection frame drawn left-to-right requires containment; right-to-left
  uses crossing bounds. Shift adds to the selection.
- M,M starts Move; M,D starts connected Drag. With no prior selection, the first
  object click picks the object and reference; the next click places it.
- Space / Shift+Space rotate the active mouse move, Move/Drag or paste preview
  in opposite directions. During routing they retain the corner-mode meanings.
- Ctrl+C / Ctrl+X pick a clipboard reference; Ctrl+V starts placement. Rotation
  of the paste preview is applied to the committed geometry too.
- Right click cancels a mouse drag. Release outside canvas, window focus loss
  and switching documents discard transient mouse drags without adding history.
- Esc cancels placement/move. During routing it finishes already entered legs;
  a second Esc leaves the tool. Backspace removes the last route leg.
- Ctrl+Z / Ctrl+Y operate the document history; while routing Ctrl+Z backtracks.

Primary press coordinates and drag state are retained separately from egui's
release-frame response. A press/move/release crossing the drag threshold within
a single update is processed once, rather than lost or committed as a click.
Move uses the same snapping settings at both endpoints, including disabled grid.
Rotation and movement are preview-only until a successful drop, then form one
history entry. Stale revision checks and locked-object validation still apply.

## Validation and limits

Regressions exercise real egui wheel, pinch, pointer and keyboard events:
zoom anchor/tail/limits, both scroll mappings, panel isolation, right/middle pan,
fit targets, cursor bounds and fast release. Controller tests cover fast move,
Undo/Redo, cancellation, box direction, grid-off positions, picking for Move/Drag,
connected endpoints, rotated paste and rotating an active mouse drag. Settings
round-trip and shortcuts/tab-switch cleanup are also covered.

Validation on Linux/Xvfb used a focused real window: Ctrl-wheel zoom in/out
returned the canvas to the same image (one raster pixel differed), right-drag
panned, V,F excluded an off-board drawing, and mouse move plus Space saved R1
at (14,16) mm / 90° with pads at (14,15) and (14,17) mm. Undo/Redo restored
these states. Escape cancellation also suppresses primary pointer events that
arrive in the same frame, until release; a regression covers restarting a
new drag after that release.
Ctrl+W routed a net-1 track from (25,10) to (25,15) mm and saved it correctly.
Component source and footprint descriptions now have distinct scroll IDs,
removing the collision exposed by selecting the moved component.

Connected Drag remains the existing local endpoint-stretching operation, not a
shove router. General DRC, obstacle solving, native context menus, automatic
edge-panning and all Altium selection/drag preference combinations are not
implemented by this change. Crossing selection still uses item bounds rather
than exact intersection with arbitrary copper. Exact native fit margins,
rotation-step preferences and cursor rasterization are not confirmed.
