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
impedance-driven DRC rules, automatic symmetry, rigid-flex substacks, backdrill,
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
