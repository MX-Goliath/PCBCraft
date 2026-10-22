# Cross Probe between schematic and PCB

Implemented 2026-10-22. The Altium-style entry points are Tools > Cross Probe,
T then C, and Ctrl+click for a one-shot jump. In the persistent mode, clicking an
object activates the counterpart document; the mode remains active in both
editors. Esc or a right click leaves the mode. Pan and zoom continue working.
The status bar identifies the mode. Tool changes leave the mode.

Supported objects:

- Component body/designator ↔ component. Imported UNIQUEID / SOURCEUNIQUEID
  links take precedence, including the final component segment of a source path with `/` or `\` separators.
  If either link is absent, a nonempty assigned designator is used. Two present
  but conflicting links never fall back to a coincidentally equal designator.
- Schematic pin ↔ component pad, using component identity and pin/pad number.
  Multiple physical pads with the same number are highlighted together.
- Wire, net label, power port or port ↔ PCB net. Tracks, vias and standalone pads
  probe their net. Pads belonging to components probe the corresponding pin.
  Connectivity follows wire endpoints, T junctions, pins and explicit junctions;
  bare interior crossings do not connect. Equal named labels/ports join islands
  on a sheet. PCB net mapping prefers connected pin identities over net names;
  contradictory PCB nets are reported rather than merged.

Search is scoped to the source document's loaded PrjPcb project. Its opposite
editor documents are opened on demand and existing tabs are reused. Without a
project, only open free documents participate. Unrelated loaded projects are
excluded. When several documents contain the target (including multi-sheet
nets), or several physical component instances match a repeated channel, a
destination chooser lists the document and physical designator/path. Missing links leave the active document
unchanged and explain the problem in Messages. Multiple PCB component/pad instances remain separate choices; repeated same-part
matches on a schematic are rejected as ambiguous.

Only camera, layer visibility and selection/highlight state change. No document
command is recorded, dirty state is unchanged, and Undo/Redo history is untouched.
Pending PCB edits must be finished/cancelled before probing into their document.
The PCB target's layers are made visible and the selection fitted. The schematic
view centres on its target. Component and pin selection appear in Properties;
net selection highlights its connected primitives. A mode click cannot place,
move, route or delete objects. Locked visible PCB objects can be inspected.

## Verification

`fixtures/cross-probe/demo.PrjPcb` references two existing authored fixtures;
there are no third-party/TI designs in this fixture. Tests cover both directions,
component/pin/net resolution, immutable document/history, native IDs vs renamed
references, wrong-project exclusion, absent/ambiguous destinations, automatic
opening without duplicate tabs, T,C across idle frames, Esc, crossing vs junction
connectivity and named islands.

Linux GUI verification: Ctrl+click R1 opens the previously unopened PCB and
selects R1; Ctrl+click its pad selects schematic pin 1. T,C then clicking the
wire highlights two pads and a track on PCB; clicking the track returns to the
schematic net. Esc exits; neither tab becomes dirty.

## Remaining differences

This is correspondence/navigation, not an Altium project compiler or ECO engine.
Existing native links to repeated component/pad instances are supported through
explicit instance selection. Full hierarchical compilation, bus/harness
expansion, sheet-entry scoping and variant mappings are not resolved. A schematic
net spanning several compiled PCB channel nets reports ambiguity. Source UID
suffix collisions require explicit instance selection. Net names are
fallbacks, not proof of hierarchical connectivity. Split-window continuous
probing without activating the destination, Altium masking preferences and exact
zoom/pixel parity have not been reproduced or verified against a live AD 24.4.1
instance in this Linux environment.

Read-only local audit of TI PMP23595: 164/167 schematic components have PCB
candidates; 36 require physical-instance selection. Three have no match. The
TI design stays outside Git. Reproduce with `cargo run --locked -p eda-io
--example cross_probe_check -- /path/to/project.PrjPcb`.
