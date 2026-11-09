# Workspace panels — 2026-11-08

## Reference and observations

Reference: installed Altium Designer Enterprise 24.4.1, Windows. Observed the
Projects title and tab strips, Close/Allow Dock context menu and its horizontal
and vertical controls, the pin, border buttons and popup panel. The reference
was returned to its original pinned Projects/Home state; no document was edited.
Automated title dragging in Altium did not produce an observable move, so it is
not evidence for its floating/drop geometry.

The documented interaction contract is also described in
[Altium Working with Panels](https://www.altium.com/documentation/altium-designer/working-with-panels):
four docking edges, tab insertion/reordering, recursive tiling, caption versus
individual-tab dragging, popup rails, Ctrl to suppress docking, F4 to hide/show
floating panels, and restoring closed panels at their previous location.
The current online documentation is not a measurement of version 24.4.1.

## Implementation

`eda-app/src/dock_layout.rs` stores recursive splits and tab groups, active tabs,
four edge positions, floating bounds, split fractions, edge extents, popup state
and Allow Dock flags. Each panel belongs to exactly one group. Closing retains
membership; moving removes membership once and collapses empty split branches.
This is workspace UI state, independent of documents and their edit history.

Drag a tab, its title, or the empty caption area to move only that panel.
The caption never implicitly moves its neighboring tabs. This follows the
user's explicit 2026-11-08 correction to the earlier group-caption behavior.
A complete blue target joins tabs; the four arrow
targets split the target group; the application border docks to that edge.
Insertion markers indicate tab order. Dropping in free space creates an actual
eframe/winit viewport, which can leave the main window. Ctrl suppresses targets;
Esc cancels without changing topology. Floating edges snap to other floating
windows. F4 hides/shows floating windows without closing their panels.

The pin hides only the active panel and creates its border button. Other tabs
and split neighbors retain their location and visibility. Hover opens
the requested panel; leaving hides it after a short delay. Clicking keeps it
open until clicking outside. Popup content has no duplicate bottom tab strip;
its inner edge resizes it. Pinning restores only the requested panel. The close
cross and all header context commands also act on that panel. Allow Dock flags
are stored per panel and remain independent after regrouping and restart.
Docked panels and recursive separators are resizable. Floating panels have
resizable native edges and maximize/restore commands and caption double-click.
Context menus additionally expose explicit Floating/Dock commands.

Layout and open/closed panels use the existing settings persistence. Old
settings migrate on first use. The existing Restore Panels command resets the
topology and visibility. Switching SCH/PCB/Home, Git comparisons or Layer Stack
Manager does not replace the topology. Properties follows its actual location,
including a floating window, while retaining the active editor's content.

Native positions store the **outer** origin and **inner** size. Recording the
inner origin as an outer position caused Windows frame-offset drift and was
corrected. Cross-window hit testing converts coordinates to the main viewport's
DPI scale; this does not affect document coordinates.

## Verification and remaining differences

Windows GUI: Projects detached to a native window, returned using Dock, moved
from the left into Properties tabs, split below Properties, and hidden by the
pin into right-side border buttons. Final-build/restart checks are recorded in
PLAN.md. Automated pointer tests cover four edges, grouping, splitting,
membership, reordering, popup selection, migration/roundtrip and DPI conversion.
The final executable also passed Messages/Properties/Messages switching while
a popup was already open, and F4 hide/restore from native/main windows.
A native edge-resize probe did not produce a visible size change through
Computer Use; actual native resize parity is therefore not verified.
The individual-panel correction passed 818 workspace tests, strict Clippy,
formatting and a Windows dev build. In the user's existing right-side tab
group, dragging the empty Git caption detached only Git. A second drag docked
only Projects on the left; Properties, Components and Messages stayed on the
right and Git retained its native window. The updated application was left open.

Full Altium parity is **not certified**. Equal-size/equal-DPI pixel comparison,
all caption/menu/tab metrics and native icons, floating transparency, animated
popup timing/preferences, desktop work-area snapping and proprietary TLT
layout import/export are not reproduced or not verified. Floating drag feedback is an
outline; the native window takes its final position on release. Whole fractal
roots do not have a separate draggable outer caption. Cross-native-window
mouse drops cannot be exercised with the current Computer Use helper because
it rejects drag endpoints outside the source window; model/input coordinates
and native detach/return were checked separately. Multiple monitors with
different DPI remain unverified in the real GUI.

Linux uses the same layout and eframe viewports. This session's Linux cross-check
stopped in `openssl-sys` because the existing reqwest/native-tls dependency has
no target OpenSSL sysroot here. Linux runtime was not exercised. Wayland does
not expose global window positions to eframe; exact cross-window positioning
and native window placement there remain a platform limitation.

## Properties width feedback - November 9, 2026

The user reports that selecting certain components expands Properties across
most of the window, and manually shrinking it springs back. Static inspection
found that the content's `scope_builder` propagated its expanded minimum rectangle
into the dock/SidePanel. Its clip rectangle limited painting, not layout size.
The resulting panel size was then persisted as the dock extent.

The content now uses an isolated child UI; the parent allocates only the dock's
assigned rectangle. Long fields and wide nested layouts cannot enlarge the dock
through their minimum size. Properties labels inherit truncation, and schematic
Properties no longer imposes a 200-point minimum on its inner content. Stored
widths and user-controlled resizing remain available, without resetting layouts.
At extremely narrow widths, controls may clip; values are retained in full.

Regression sources exercise oversized content, pointer shrinking and subsequent
frames at 100/150/200 percent DPI, plus actual schematic/PCB components with long
fields across document switches and unchanged models. No compilation, test run,
or new GUI instance was started under the user's no-rebuild instruction. Formatting
and static checks pass; updated-window behavior and Altium/Linux parity are pending.

## Dialog width stability - November 9, 2026

The user reports Opening document oscillating in width as PCB import phases change,
and similar behavior elsewhere. A nonresizable egui Window still fits its content;
preventing manual resizing alone does not reserve a stable width. The application
now uses shared stable content hosts for every Window/Modal call site. Windows use
their declared/current resize width; each modal supplies its existing form/page
width explicitly. A horizontal overflow viewport prevents a wide child minimum
rectangle from enlarging its host. Labels wrap within that width, while oversized
tables/controls remain reachable by scrolling. Existing form coordinates, explicit
page sizes and permitted user resizing are retained.

Opening document has a 420-point content width, with single-line filename and phase
labels and full-text tooltips. Elapsed time and cancellation do not change its
layout. The update popup uses the shared width host. The 3D loading card paints
its frame at a fixed rectangle and places clipped/truncated status content inside;
phase, elapsed-time and error strings cannot determine the frame geometry. The
renderer Object Visibility modal was audited: its explicit width and bounded
matrix already separate text from outer geometry. Native OS dialogs are unchanged.

Added regression sources cover repeated long/short text transitions, overflowing
controls, user resizing, modal Cancel interaction, document name/phase/time/cancel
changes, and 3D card bounds. DPI cases include 100/150/200 percent. They have not
been compiled or run: the user's no-rebuild instruction remains in force. Only
formatting, whitespace and source/feature-registry checks were run. Actual-window
Windows/Linux verification and equal-DPI Altium comparison remain pending. Dialog
height can still change for wrapped content or a deliberately different form page;
this change prevents content-driven width oscillation.
