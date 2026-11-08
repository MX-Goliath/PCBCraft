# ADR 0001: initial shell

Accepted October 19, 2026.

Retain the original decision: an independent Rust core, egui/eframe and a single
wgpu renderer. Windows is checked locally; Linux uses separate CI.

For the initial rectangular pads and straight tracks, use i64 nm,
a coordinate range of +/- 1 m and schema_version=1. All values in this range are
also represented exactly by JSON clients using doubles. The original plan's
10 pm scale remains a candidate for native Altium representation: a separate ADR
and migration are required before native IO. The current choice does not promise
exact representation of all Altium coordinates.

Physical data is serialized separately from view state. Zoom and pan do not
change the model. A rejected edit does not change history. Undo/redo increase
the revision. Early history uses snapshots, with no promise of suitability for
large boards or long sessions.

The first iteration uses resizable/hideable egui panels and ordinary tabs.
Full docking, floating, autohide and multiple windows are deferred and are not
marked complete. Engineering menu commands are currently disabled.
