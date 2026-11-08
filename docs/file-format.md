# PCBCraft JSON v1

Current example: `fixtures/controller.pcbcraft.json`.

A document contains schema_version=1, UUID id, name, kind (Board/Schematic),
size {x,y}, pads and tracks. Coordinates and dimensions are i64 nanometers.
Each coordinate must be within +/- 100 000 000 000 nm (expanded for large sheets).
Document and pad dimensions are positive. Track widths are positive.
Object IDs are unique within a document. Net/footprint references are not yet present.
Unknown fields and versions are rejected; data is not discarded on save.

Pad: id, name, position, size, layer.
Track: id, start, end, width, layer.
Layer: Top, Bottom, Overlay, Outline.

Loading and writing are limited to 256 MiB. The model is validated before writing;
a temporary file is created in the destination directory, synchronized and
replaces the file through tempfile::persist. Recovery guarantees after power loss
and directory fsync have not yet been verified.

The optional `schematic` field contains the sheet, fonts, components, primitives,
original records, base64 embedded images, warnings and the original SchDoc in
base64. Older JSON v1 documents without this field still open.
Older program versions reject the unknown field instead of losing content.
Scene IDs and owner references are checked for uniqueness/existence;
object counts and geometry ranges are bounded.

The application's format is separate from SchDoc. Import does not imply native
writing; saving to SchDoc/PrjPcb/PcbDoc/SchLib/SchDot extensions is rejected.
Source data precision and preservation: ADR 0002 and altium-import.md.
