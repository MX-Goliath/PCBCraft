# Schematic signal harnesses

Reference: the running Altium Designer Enterprise 24.4.1 on Windows, observed
2026-10-31. Research used the separate `target/routing-check/HarnessReference.SchDoc`;
the user's Altium project was not changed. This is schematic signal harness
support, not the separate mechanical Harness Design product.

## Editing

`Place > Harness` provides Signal Harness, Harness Connector, Harness Entry and
Predefined Harness Connector. The menu prefixes are P,H,H / P,H,C / P,H,E.
A connector uses two clicks for its rectangle; Space rotates its primary side.
An entry must be placed inside its connector and projects onto the opposite
edge. Numeric entry names increment. Connector Properties edits the type,
visibility, Full/Prefix bus text, geometry, primary connection, border, fill and
entries. Entry Properties edits the name, nested type and font.

Signal Harness, Bus, Bus Entry, Port and Sheet Entry placement is enabled.
Lines snap to grid/electrical hotspots. Shift+Space cycles 90-degree, 45-degree
and any-angle modes; Space switches the corner direction. Backspace/Delete
removes the last confirmed vertex. Enter confirms a vertex. Esc/right-click
finishes the confirmed line and keeps the tool available for the next line;
another Esc/right-click leaves the tool. Tab pauses placement for Properties;
Esc or the pause overlay resumes it.

Predefined Connector reads definitions from open sheets, project-listed
`.Harness` files and schematic sidecars. It offers a filter, entry sorting,
optional Signal Harness with length, and optional Port with name/width/Auto.
The entire group floats on the cursor and commits as one history operation.
Native `.Harness` syntax is `Serial=RX,{Control:Logic}`; `LOCKED;` definitions
are retained when saving. UTF-8 and legacy Windows-1251 input are supported.

Model changes use validated eda-core transactions. Ownership survives
move/rotate/copy/delete and Undo/Redo. Connected drag stretches attached wires
and same-kind bundle paths. Entry drag remains constrained to its connector
edge, expanding the connector when needed.

## Electrical meaning

The project compiler expands harnesses into independent leaf channels before
scalar wire union. Equal Harness Types describe structure; they do not short
unconnected instances. Physical paths, project port scope and matching
Sheet Entry/Port hierarchy determine which instances connect. Nested harnesses
and numeric bus ranges expand to separate channels. Ordinary bus wires require
labels identifying a bit; Bus Entry geometry can select the attachment point.
Bare interior crossings do not connect, while endpoint/T contacts do.

A Net Label on a harness overrides leaf net names as `Label.Entry`. Named
connector-free bundles use declared definitions (including LOCKED definitions)
and qualified scalar labels. Conflicting definitions/types, missing/unknown
types, duplicate entries, invalid bus members and cyclic nesting are explicit
compile errors. Local cross-probing follows expanded channel joins.

## Native storage

Native SchDoc records are 215 Connector, 216 Entry, 217 Connector Type and
218 Signal Harness. They reside in `Additional`; child OwnerIndex values in
that list are local, not FileHeader indexes. The importer resolves this
distinction, including omitted zero OwnerIndex. DistanceFromTop uses native
100-mil units and `_FRAC1`; ordinary coordinates use the native 10-mil scale.

The preserving native writer updates/inserts/deletes these records, remaps
owners after ordinary FileHeader insertions, and verifies the reimported scene
and attachment topology before replacing a destination. Unknown index
dependencies retain the existing refusal guard. Unchanged saves preserve the
original bytes. Native saves update the sibling `.Harness`, preserving LOCKED
definitions. The two destination files are individually replaced; they are not
a filesystem-wide atomic pair. A new document without a native source archive
still uses PCBCraft JSON, as with the existing native writer.

The Rust-written isolated `HarnessRoundtrip.SchDoc` opened in Altium 24.4.1,
showing renamed Serial/RX, a newly authored nested TX entry and a Logic
connector. This validates that sample, not every SchDoc dialect.

Final Windows GUI QA on `HarnessGui.SchDoc`: Properties/Add entry, native Save,
Predefined Serial with three entries + 500mil Signal Harness + HarnessPort,
atomic Undo/Redo, native Save and successful Altium 24.4.1 reopen. The added
entry and complete second harness group are visible in Altium. GUI checks
caught and fixed authored sync-flag serialization and a dialog layout overflow.
585 workspace tests, fmt check, strict workspace/all-target Clippy and Linux
GNU all-target compilation pass. Linux runtime has not been exercised.

## Remaining parity work

Full Altium equivalence is not claimed. Automatic obstacle-avoiding Auto Wire,
all manual look-ahead preferences, automatic net classes, repeated/multichannel
sheet instances, Device Sheet folders, rich context actions and the Active Bar
hold-menu are not implemented here. The Predefined dialog groups available
definitions under Free Documents, rather than reproducing every project tree.
Qualified leaf-name casing is normalized internally. Conflicting/incomplete
electrical designs can be edited but compilation reports the conflict; native
sidecar generation can also reject conflicting definitions.

Exact selection handles, brace curves, glyphs, field/table controls and every
DPI/state have not been matched. Port/Sheet Entry harness coloring currently
uses the default harness colors without Altium's preference toggle. Existing
native geometry/fonts/colors are retained in the model. Linux runtime and
visual parity require a separate check. Historical registry entries about
unsupported buses/harnesses describe earlier work; `schematic.signal_harnesses`
records the current bounded implementation.
