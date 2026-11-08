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
