# ADR 0002: first stage of Altium schematic import

Accepted October 19, 2026. Related original plan sections: §3, §5, §9–10, §25–26.

Import and native writing have independent support levels. CFB/ASCII SchDoc 5.0
viewing and PrjPcb references are currently implemented. There is no native writer.
Components, the scene, original fields and the original file are stored in eda-core;
the CFB/INI parser lives in eda-io, and screen geometry lives in eda-render.

The scene remains in i64 nm. File coordinates in 10 mil units and their
`_FRAC/100000` fractions are calculated in i128 and rounded once to the nearest nm,
including negative values. Maximum rounding error is 0.5 nm. Original coordinate
fields and bytes are preserved without this loss. This is a sufficient viewing
contract, but not yet a solution for editing/native roundtrips at the original
plan's 10 pm precision. A separate migration is required before such editing.
The geometry range is expanded from ±1 m to ±100 m for large sheets.
SchDoc child-object positions are already absolute; rotating them again is prohibited.

UUID v5 is derived from the document's native UniqueID and record index. A file
without UniqueID receives a content-based ID. IDs are stable when reimporting the
same document; index stability after third-party editing is not promised.

JSON v1 is extended with optional `schematic`; older documents remain readable.
The imported scene is saved only in the application's own format.
Unknown streams and fields are retained in the original SchDoc archive;
this does not guarantee their validity after future geometry changes.

Linux uses the same parser, model and renderer. Windows project paths are
normalized, relative references are resolved from the project folder, and unique
case-insensitive matches are searched separately. Windows/Altium fonts are not
distributed; a user directory and a free fallback are supported.
