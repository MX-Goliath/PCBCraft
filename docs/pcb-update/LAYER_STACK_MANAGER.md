# Layer Stack Manager — 2026-10-22

Open **Design > Layer Stack Manager** in a PCB document. The manager occupies the
central workspace with Stackup / Via Types / Impedance tabs, a cross-section below
the tables and a Properties panel on the right. It edits an isolated draft bound
to the originating document and history revision. Apply makes one validated Undo
transaction. Close offers Apply, Discard or Cancel when there are unapplied edits.
Opening, closing or applying an unchanged draft does not dirty the document.
Complete/cancel a pending PCB edit before opening the manager.

## Stackup

Select a row to edit its name, signal/plane or core/prepreg/solder-mask kind,
material, thickness, dielectric constant Dk and loss tangent Df. Thickness is also
editable in the table. Units can be switched between mm and mils. Add Layer
allocates an unused copper identity and inserts dielectric separation; Add
Dielectric adds a material row. Move Up/Down changes physical order without
renumbering copper identities. Top and Bottom must remain the outside copper
layers. A copper layer referenced by objects, pad stacks, vias or polygons cannot
be removed. Profiles referring to missing layers also prevent Apply.

Imported V9 stack rows supply names, physical order, thickness, material, Dk and
Df. Old documents without a usable V9 stack get explicit editable defaults.
The typed stack is persisted in PCBCraft JSON. The board's copper layer list and
3D thickness follow the applied stack. Previously hidden layers remain hidden;
new copper layers become visible. Undo restores both the model stack and layers.
The cross-section is schematic, with thickness explicitly not to scale.

## Via Types

Add named Through, Blind/Buried or Micro types, select first/last copper layers,
set pad diameter and drill diameter. Through spans Top–Bottom; blind/buried must
not span the full board; microvia spans adjacent copper layers. Start precedes end
in physical order. The drill must be smaller than the pad diameter.
“Apply and use for placement” commits the draft and copies these dimensions/span
into the PCB via placement defaults. Existing vias are not retroactively changed.
This is a placement preset, not a live type binding on already placed vias.

## Impedance

Add named single-ended or differential profiles with target impedance, tolerance,
signal and reference layer(s), width and gap. Calculation uses quasi-static IPC-
style analytical approximations for microstrip and symmetric stripline. Width can
be solved numerically for the requested target. Estimates refuse intervening
copper, asymmetric stripline or internal single-reference configurations.
Profiles persist independently of a successful estimate; unavailable estimates
are displayed as unavailable, never as zero. Single-ended routing can use the
profile's width and signal layer through “Apply and use routing width”.
Differential pair routing is not implemented, so its use button is disabled.

## Explicit differences from Altium 24.4.1

This is a functional subset, not verified pixel-for-pixel parity. No live Altium
Layer Stack Manager session was available for this change. Native `.PcbDoc` stack
writing is **not implemented**: V9 custom stack data and other native dependent
records cannot safely be updated by patching layer names alone. A changed typed
stack therefore refuses native saving before touching the destination. Use
**Save As > PCBCraft JSON**; source archive bytes remain preserved. Undoing all
stack changes restores the previous native-save eligibility.

Native via-type/impedance custom data is retained in source archives, but is not
imported into these editable tables. No Simbeor/field solver, frequency-dependent
loss calculation, material library, manufacturing constraint verification,
impedance-driven DRC rules, rigid-flex substacks, backdrill,
stacked/staggered microvia sequences or native file interoperability is claimed.
The approximate impedance result is not a manufacturing sign-off value.

## Verification

Regression tests exercise imported units/materials, stable copper identities,
physical stack thickness, layer removal dependencies, via ranges, impedance
response and inverse width solving, JSON save/reopen, destination-preserving
native-save refusal, atomic Apply/Undo/Redo, no-op Apply, stale drafts and hidden
layer preservation. Run fmt, strict workspace Clippy and all workspace tests.

Verified locally: 236 workspace tests passed, strict Clippy and formatting passed.
Real Linux GUI: add internal copper, Apply, add/use a through-via preset, add a
single-ended impedance profile and solve 50 ohms, Apply and Undo back to the clean
board. On the local TI PMP23595 board, six copper layers, original prepregs/core,
mask rows, Dk/materials and 2.2082 mm physical thickness displayed correctly;
unchanged Apply/Close left the document clean. TI files were not modified or added.
Signal/Plane classification is stack metadata; automatic negative-plane generation
is not implemented.

## Restoration work — 2026-10-23

All work preceding this stage was committed and pushed as `83f2b7c`.
Live Altium ReferenceA was opened for reference, but the Windows session was then locked;
the Layer Stack Manager itself could not be inspected. No pixel parity or real
Windows GUI verification of this stage is claimed. Existing screenshots from PCB
editing are not evidence for the stack manager.

Behavioral reference: [Altium's layer stack documentation](https://www.altium.com/documentation/altium-designer/pcb/defining-layer-stack).
This is current documentation; behaviors and dimensions still require comparison
against the installed 24.4.1 version.

Implemented independently in `eda-core`: persistent Stack Symmetry, top-to-bottom
or bottom-to-top material reconciliation, mirrored property edits and paired
insertion/removal/reordering. Names and electrical identities remain independent.
Copper insertion splits dielectric intervals while preserving total dielectric
thickness; odd integer remainders retain symmetry. Plane insertion allocates plane
identities. Invalid operations leave the draft unchanged. Board dependencies are
checked again on Apply; deleting copper used by board objects remains refused.

The editor now has isolated draft Undo/Redo, Ctrl+S to apply, Ctrl+W to close and
Ctrl+Q for the existing mm/mil switch. Save/Undo/Redo/Close menu commands target the
stack draft. Apply clears draft history and creates one board-history transaction.
Tab order is Stackup / Impedance / Via Types. Full-stack mirroring, four-unit cycling,
native stack writing and the other unsupported features above remain outstanding.

Regression tests cover symmetric property propagation without identity changes,
odd-nanometre insertion, dielectric splitting, invalid-edit rollback, used-layer
rejection, paired moves, JSON compatibility and isolated draft/board Undo/Redo.

This stage passes 310 workspace tests, `cargo fmt --all -- --check`, strict
workspace Clippy and the Windows dev build. Original ReferenceA `.PrjPcb` and `.PcbDoc`
SHA-256 hashes remain unchanged. New stack changes are local and uncommitted
at the beginning of the real-window comparison.

### Live AD 24.4.1 observations after session unlock

At 1920x1030: Projects ends at x554; the right Properties dock starts at x1549.
Stackup is a separate document tab; the menu is File / Edit / View / Project /
Tools / Window / Help. Toolbar Add / Modify / Delete starts at y96. Table header
is y128–148, data rows are 21 px high. Columns: #, Name, Material, Type, Weight,
Thickness, Dk, Df. Copper rows are orange, core/prepreg gray-brown, mask/overlay
green. Bottom tab order: Stackup / Impedance / Via Types. Stackup has no permanent
cross-section beneath the table; Via Types shows layer bands and via spans.

ReferenceA rows: Top Overlay, Top Solder (0.01016 mm), Top Layer (0.03556 mm),
Dielectric2 (Prepreg, FR-4, 0.7 mm, Dk 4.8), Bottom Layer (0.03556 mm),
Bottom Solder (0.01016 mm), Bottom Overlay. Altium reports conductive thickness
0.07112 mm and total thickness including mask 0.79144 mm. Symmetry is enabled.
The implicit through span is named Thru 1:2. No impedance profiles are present.
Only selection and tab navigation were performed in the original Altium project.

Implemented next: imported zero-thickness overlay rows; measured column order,
row colors/height and Board thickness summary; context menus and draft commands;
document tab navigation between board and stack draft. Board revision checks still
reject applying a draft after intervening board edits. Unsaved stack drafts block
closing their board or quitting. Via Types uses a schematic span diagram; its
implicit through span is a display fallback, not an imported preset binding.

Remaining differences include imported symmetry/via/impedance custom data,
full column editing/resizing/search, material metadata/library, exact icons and
typography, all document-window arrangements and the advanced features above.
V9_STACKCUSTOMDATA was observed to be zlib/base64 XML containing exact thicknesses,
IsSymmetric and ViaSpans keyed by layer UUID; this is recorded evidence, not a claim
that native custom-data reading/writing is implemented. Legacy V9 dimensions can
differ by 1 nm due to mil decimal rounding. Missing native Df currently uses the
existing model default, so displayed Df may differ from Altium's empty field.

Windows verification: ReferenceA table shows seven rows and 0.79144 mm total; enable
symmetry, add Signal in the central dielectric, observe four copper layers and
unchanged dielectric thickness, Undo back to two copper layers. Switching to PCB
and back retains the draft and its history; a second Undo returns to a clean draft.
The original PCB SHA-256 remains unchanged. 311 workspace tests, strict Clippy,
format checking and the final Windows build pass. Exact visual parity is still
not claimed: toolbar/property spacing, fields, icons and unsupported controls differ.


### Properties follow-up (2026-10-23)
Live AD observations now include copper and solder-mask fields. Copper exposes
Manufacturer, Material, Process, Weight, Thickness, Orientation, Copper Orientation,
Description, Note, Comment. Mask exposes Manufacturer, Material, Thickness, Dk,
Df, Description, Color, Solid, Frequency, GlassTransTemp, Note, Comment.
PCBCraft adds per-layer fabrication metadata with backward-compatible JSON and
history; weight edits convert to physical thickness. Properties sections collapse,
scroll and filter by field name. Stackup has type-dependent fields; Via Types has
a visible implicit through span and ordered span choices; Impedance has an explicit
empty state and constrains reference choices around the selected signal layer.
Changing to an outer signal disables stripline rather than creating invalid references.

Still partial: native metadata/custom-data import and write-back, material-library
selection, mask color rendering, copper-orientation editing, via mirror, substack
configuration and roughness physics. Unsupported controls are disabled. Added
placement-preset and analytical-calculation sections are PCBCraft functionality,
not a verified reproduction of Altium's populated profile controls. Descriptive
metadata does not influence the impedance calculation. Exact pixel parity is not
claimed for these Properties panels.

Validation: 312 workspace tests pass, strict Clippy and fmt check pass. The new
Windows build was inspected at 1920x1030 on all three tabs, including dielectric
fields, the implicit through span, and a newly added single-ended profile with
Stripline disabled for Top Layer. Undo removed that temporary profile and restored
a clean draft. The original PCB SHA-256 is unchanged.
