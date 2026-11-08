# ADR 0003: editing an independent schematic scene

Accepted October 20, 2026. Supplements ADR 0002 without changing the native IO contract.

The editor operates on the existing scene in nm. All changes go through
`eda-core::schematic_edit::{Request, Edit}` and History; commands do not depend on
GUI/GPU. Revision checks prevent committing an outdated preview.
The GUI stores the tool and field drafts separately from the model; the renderer
reads the model and a separate preview without writing to the document as the
cursor moves.

Import preserves original records and the archive; edits change only the scene.
The `source_modified` flag explicitly separates the current scene from the
historical archive. Designator/Comment receive typed roles needed after copying
without original records. Old JSON remains compatible through defaults;
new primitives receive UUID v4 and a sentinel source_index; imported UUIDs
are preserved when geometry changes.

Moving/rotating a component is one command over its entire editable group,
including hidden parameters. Properties preserves parameter presentation,
original expressions, sheet settings, instance metadata and model metadata.
Hidden parameters have UUIDs and are edited independently of visible objects;
alternative native representations remain in the historical archive.
Saving/reopening checks the application's JSON, not a native roundtrip.
Full engineering semantics, 10 pm, a durable journal, operation ID and a compiler
remain open acceptance criteria.

Portability: one egui input, model and command implementation for Windows/Linux;
clipboard Copy/Cut/Paste events are handled separately from Key, as required by
egui-winit 0.32.3. No platform system calls were added to the core.
