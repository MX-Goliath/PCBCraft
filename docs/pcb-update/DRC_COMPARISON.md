# Run DRC comparison - 2026-11-04

This is an observation log, not a claim of complete Altium equivalence.
ReferenceB design files, native reports, snapshots and generated probe copies
remain local and ignored by Git. Original design files were not edited.

## Observed in Altium Designer Enterprise 24.4.1.13

Computer Use opened the PCB editor, Design Rule Checker, report, Messages,
PCB Filter and text Properties. Reference windows used the same desktop DPI.
The native checker measures 771 x 700 logical pixels. Its inherited stop limit
was 10000; all reported comparison runs completed without reaching it.

The unchanged ReferenceB copy reports two violations:

- Clearance: SMD pad versus a Bottom Layer keepout region, 0.173 mm < 0.2 mm.
- Board Outline Clearance: Top Layer stroke text, 0.675 mm < 0.7 mm.

The same source initially produced 26 PCBCraft violations: 22 internal No Net
copper clearances and four board-edge findings, including two compatibility
record duplicates. It also produced 27 unchecked conditions.

Independent copies changed one input at a time:

- Keepout mask 8 retains the SMD violation; mask 17 removes it (two versus
  one native violations). This verifies the SMD restriction bit.
- Replacing the graphical component kind with Standard retains two native
  violations. A component-kind exemption would therefore be unjustified.
- Changing the general clearance rule from Different Nets to Any Net produces
  1139 native clearances, including the same No Net fill/track intersections
  previously reported only by PCBCraft. Two unassigned objects must not be
  compared by a Different Nets rule. Assigned versus unassigned copper and
  keepouts still need checking.
- Increasing only polygon/region outline matrix clearance to 0.46 mm produces
  two additional native violations, both displayed as 0.45 mm < 0.46 mm.
- The polygon matrix controls at 0.451 mm and 0.4505 mm also each produce
  the two polygon violations; the original 0.449999 mm constraint passes.
- A straight-edge, ordinary triangular region with an approximately 805 nm
  deficit is reported by Altium (0.449 mm < 0.45 mm). This rejects applying
  a global 1 micrometre board-outline tolerance to explain the polygon case.
- Adding a collinear midpoint to each disputed polygon edge retains two
  native violations. Vertex-only distance checking does not explain the gap.

Native text Properties shows Sans Serif stroke text, height 1.1 mm and stroke
0.2 mm. Its Properties location differs from the stored text baseline. Display
bounds or an inferred baseline offset cannot establish exact DRC geometry.
Extended native text coordinates at offsets 244/248 match the Properties
location for this text; other texts use different alignment. They are not
universally a glyph bounding-box origin. The 124/128 fields are native text
rectangle dimensions, so treating them as exact glyph extents is unverified.

## Implemented

- Exclude marked legacy region duplicates and component body projections from
  manufactured-geometry DRC, mask apertures and drill checks, without deleting
  their native persistence records. Coincident unmarked real objects remain.
- Apply object-specific keepout clearance on its signal layer, or across copper
  layers for the Keep-Out Layer. Keepouts do not become short circuits or
  routing-width objects. Invalid restriction masks are explicitly unchecked.
- Import and preserve binary Track/Arc/Fill restriction masks; correct the
  modern Arc layer field offset. Region masks were already retained.
- Correct Different Nets selection for two No Net objects, in both cached and
  uncached rule resolution. Routing still treats unrelated No Net copper as
  physical obstacles.
- Explicitly report unimplemented copper-text and text board-outline geometry
  instead of silently presenting those checks as complete.
- Adjust checker geometry, check-only catalog and separate Online/Batch Used
  On / All On / All Off commands. Unsupported checks remain disabled.

With the corrected keepout metadata, the model-space comparison has three
findings: the matching 0.172657 mm keepout clearance and two unresolved polygon
outline clearances. Text outline geometry remains explicitly unchecked.

## Remaining differences

PCBCraft measures the two polygon outline gaps as 0.449194 mm under a
0.449999 mm constraint; Altium accepts the original copy. Increasing board-arc
sampling does not remove that discrepancy. No global tolerance or suppression
was added to force the reports to match. The missing native text violation,
full glyph/font geometry, all keepout mask/layer combinations, native report
format/docked Messages, batch/online coverage and complete UI/DPI parity remain
open. The earlier ReferenceE differences in RULES.md also remain open.

The appended matrix outside the reference board and a source-free native PCB
that failed to load are inconclusive probes; they do not verify negative cases
or native export interoperability.

Final live PCBCraft QA confirms the 771 x 700 checker, working context menu,
independent Online All Off / Used On actions with Batch unchanged, and the
three-violation / 29-unchecked report. Repeating native DRC still produces two
violations. The source copy remains byte-identical to the original.

Final fmt and strict workspace/all-target Clippy pass. All 727 workspace tests
pass, including native keepout mask roundtrips, routing obstacle regressions,
small-viewport dialog layout and GPU checks. Details are recorded in PLAN.md.

Primary references: [keepouts](https://www.altium.com/documentation/altium-designer/pcb/object-specific-keepouts),
[DRC setup](https://www.altium.com/documentation/altium-designer/pcb/drc/setting-up-running).
The installed 24.4.1 observations take precedence over newer documentation.
