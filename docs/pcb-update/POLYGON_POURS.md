# Polygon pour corrections, 2026-11-05

The reference is Altium Designer Enterprise 24.4.1 on Windows. This checkpoint
addresses failed or very slow repours, native settings and stale background work.
It does not establish complete Altium geometry or interface parity.

## Observations and reference behavior

- Live Tools > Polygon Pours contains Polygon Manager, counted Shelve/Restore,
  Repour Selected, Repour All, Repour Violating Polygons and Repour Modified.
- The native manager separates Remove Dead Copper, Remove Islands Less Than,
  Remove Necks Less Than and Pour Over Same Net. Area filtering does not depend
  on whether dead copper removal is enabled.
- A local reference contains 74 polygons, including solder-mask polygons. The
  previous model rejected the non-copper layers and ignored native REMOVEDEAD,
  REMOVEISLANDSBYAREA, AREATHRESHOLD, REMOVENECKS and POUROVERSTYLE.
- Native area uses squared 1/10000-mil coordinates. A threshold of
  250000000000 corresponds to 2500 square mil, or 1.6129 square mm, as displayed
  by the installed manager. Original native files remain separate from research
  copies and generated outputs.

Primary documentation: [Polygons on Signal Layers](https://www.altium.com/documentation/altium-designer/pcb/signal-layer-polygons),
[same-net connection troubleshooting](https://www.altium.com/documentation/knowledge-base/altium-designer/pour-polygon-to-connect-the-same-net-copper-objects-inside-the-outline).

## Implementation

- Manual Repour uses the same revision-checked background path as automatic
  repour. Explicit requests refresh imported copper even when its stale flag is
  clear, and retry a previously failed revision. Applying derived copper does
  not add another Undo action or replace a newer document revision.
- Superseded and closed jobs cooperatively cancel. One worker per document
  coalesces edits; cancellation is checked between geometry operations. Progress
  reports completed/total polygons. A single boolean operation is not preempted.
- A document-space R-tree conservatively includes the largest relevant rule
  matrix clearance and basic/advanced thermal air gaps before selecting obstacle
  candidates. Geometry and buffered
  obstacles are cached only for that calculation. Unknown rule bounds disable
  pruning rather than conceal unsupported input.
- Obstacles are unioned and subtracted in batches. Exterior/interior winding is
  normalized so mixed native winding cannot cancel overlapping obstacles.
  Generated fragments never obstruct their own replacement pour.
- Same-net track/arc/fill/region behavior is separate from rule-controlled
  pad/via connections. Dead copper, minimum island area and neck removal are
  independent. Narrow neck removal is a morphological opening intersected with
  the original allowed copper, so cleanup cannot grow into clearance voids.
- Solder, paste, overlay and mechanical polygons generate graphic regions
  without electrical clearance or dead-copper filtering. Explicit same-layer
  polygon cutouts still apply. Copper-only keepout restrictions are honored;
  track-only/via-only keepouts do not carve polygon copper.
- Shelving removes copper immediately from the authoritative board, so display,
  DRC and connectivity do not wait for a worker. Definitions and cutouts remain.
- Native options are interpreted and written to native property names. Version
  1 JSON upgrades new settings without resurrecting intentionally deleted pours,
  preserves existing typed edits and marks changed definitions stale.
  Legacy PCBCraft clearance/connection/area extension fields remain readable.
- Tools uses the observed Polygon Pours submenu and order. Unimplemented manager,
  selected-only and violation-only actions are disabled. Repour Modified currently
  rebuilds the board-wide stale set, including dependencies.
- Large JSON saves use a buffered writer before the existing atomic replacement.

## Verification and limits

Regressions cover mixed-winding obstacle overlap and holes, same-net modes and
pad connections, overlapping polygons, mask polygons and cutouts, selective
keepouts, neck splitting, large matrix clearance outside the polygon bounds,
cancellation, explicit retries, shelving, native fields, JSON migration and
history. Final command results and real-window checks are recorded in PLAN.md.

`cargo run --release -p eda-io --example pcb_polygon_profile -- INPUT.PcbDoc`
loads and repours twice without writing the source. An optional second path
explicitly saves the research result; keep private outputs under `target/`.

Hatched generation, automatic/custom thermal conductors, full split-plane
semantics, exact arc tolerance/optimal void rotation, all preferences and native
dependency combinations remain incomplete. Hatched repour returns an error and
preserves existing copper instead of silently replacing it with solid copper.
Neck geometry and complete electrical/bitmap equivalence with Altium are not
certified. Native same-net numeric value 1 was observed as All; the 0/2 mapping
to None/Polygons Only is an implementation interpretation with internal roundtrip
coverage, pending an isolated external save/reopen check. A newly authored
template-free native PCB was rejected by the installed
Altium during research; internal write/read verification alone is not evidence
of external Altium compatibility. Linux runtime and exact DPI/pixel parity remain
unverified.
