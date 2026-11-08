# Opening Altium projects and schematics

The current stage covers import, viewing and basic schematic scene editing.
Controls and limitations: [schematic editor](schematic-editing.md). Altium Designer
is not required to run PCBCraft. File → Open accepts `.PrjPcb`, `.SchDoc` and the
application's JSON; File → Open Project and the folder button in Projects open a project.
The first existing SchDoc opens automatically from a project; other sheets load
when selected in the tree. Reopening activates the existing tab.
Missing files remain in the tree with an explanation.
PCB remains a reference with opening disabled; SchLib Binary 5.0 / IntLib open
in Components. [Libraries and limitations](interface/components-libraries.md).

## Support levels

| Area | Implemented | Limitations |
|---|---|---|
| PrjPcb | INI `[DocumentN]`, relative Windows paths, UTF8/BOM, UTF16 LE, legacy 1251/1252 | Legacy encoding is inferred from existing paths; absolute Windows drive paths need correction when moving to Linux. No project writing, compilation, variants or project special strings |
| SchDoc | CFB Binary / ASCII Version 5.0, FileHeader, Additional/unknown stream preservation | Not all versions/record types are implemented. Additional is preserved but not rendered |
| Components | Composite symbols, current part and display mode, designator/comment, parameters, hidden fields, absolute child-object coordinates; group move/rotate/copy, designator/value editing; built-in R/C | Local SchLib/IntLib through Components; no full annotation or ECO; moving does not stretch wires |
| Geometry | Wires/buses/bus entries, lines, polygons (including concave), rectangles, ellipses/arcs, Bezier; open START/ENDLINESHAPE=1 arrows, end sizes | Other endings, exact rendering of all dash types and special shapes remain incomplete |
| Text | Cyrillic/UTF8 overrides, rotations, justification, font table, bold/italic/underline, text frames, parameter references with quotes and chains | Complex expressions, built-in dynamic strings, per-line overbars and some mirrored/custom positions still need implementation |
| Electrical symbols | Separate pin-name/number fonts, basic bubbles/clock, ports/power ports, junctions colored from the adjoining wire, Thin Cross NoERC, IEEE Dot/Clock/High Current | Port/power/IEEE styles are partially rendered; unsupported IEEE symbols are explicitly marked. This is graphics, not compiled connectivity; custom pin positions are not yet implemented |
| Sheet | Standard/custom sizes, orientation, background, grid, border/zone dividers, system font, standard title block of 350×80 native units | Other title-block styles, custom templates and sheet entries are partially displayed; date/path come from the current session, not a native writer |
| Images | CFB Storage zlib, embedded PNG/JPEG/BMP/GIF/TIFF/ICO, aspect ratio | External images are not read automatically. Decode limit: 8192×8192, 64 MiB; unavailable resources have a placeholder |
| Properties | Component/primitive selection, filters; X/Y, Designator/Comment, text and wire/line width editing; parameters and warnings | Other parameters are read-only; visible grid/unit changes are view state |
| Saving | Application JSON with a typed scene, original fields and the complete original SchDoc | No native writer, preservation guarantees after native editing, ERC/netlist or PCB exchange |

Warnings about unsupported records appear in Import Information and Messages.
The absence of a warning does not confirm pixel parity, electrical correctness
or full support for a file version.
Opening does not change source files. Save As creates `.pcbcraft.json`;
IO guards prohibit writing JSON over Altium extensions.

Document fonts are looked up by family name separately from UI fonts. On the
observed machine, `Altium GOST Type A` is absent and Altium displays an Arial-like
fallback; PCBCraft no longer substitutes installed `GOST Type AU` for it.
Free Liberation Sans with Cyrillic support is embedded when Arial is absent.
`PCBCRAFT_SCHEMATIC_FONT_DIR` is checked before system fonts.
Exact metrics for all arbitrary families and visual Linux runtime remain unverified.

Thin axis-aligned lines are drawn with integer physical-pixel widths;
fractional capture scaling can hide individual lines in both applications.
Wire geometry is not deleted or changed by this.
PgUp/PgDn zoom the schematic by 1.5 around the pointer; Home fits the sheet.

## Verifiable example and diagnostics

`fixtures/schematic/minimal.PrjPcb` and `minimal.SchDoc` were created for the project
and contain no user designs. The SchDoc was opened in Altium 24.4.1:
R1, wires, GND, Cyrillic and a text frame were displayed.
The initial fixture displayed a blank sheet; the final file uses CRLF,
MINORVERSION=9 and only known records. The exact cause of the initial difference
was not isolated separately. Unknown records are checked in tests rather than
included in the fixture intended to open in the reference application.

```text
cargo run -p eda-app -- fixtures/schematic/minimal.PrjPcb
cargo run -p eda-io --example altium_inspect -- fixtures/schematic/minimal.SchDoc
```

Import was checked locally for ReferenceB, ReferenceI, TIDA-01597 and 25 sheets
from AWR2944EVM. User files and Altium exports remain outside Git.
Summary counts check record-reading completeness but do not replace independent
verification of every symbol and connection. Detailed log: verification.md.

## Implementation basis

The format was studied using local files and an ASCII copy from Altium; no
third-party Altium/KiCad execution code is used. Reference sources:

- [SchDoc reverse-engineering observations](https://github.com/vadmium/python-altium/blob/master/format.md): CFB and record structure; not an official standard.
- [Altium Schematic API types](https://www.altium.com/documentation/altium-dxp-developer/schematic-api-types-reference?version=3.2): object types and enumerations.
- [Altium parameters](https://www.altium.com/documentation/altium-designer/schematic/parameters): parameter purposes and indirect strings.
- [rust-cfb](https://github.com/mdsteele/rust-cfb): a general OLE Compound File reader.
- [earcutr](https://docs.rs/earcutr/0.5.0/earcutr/): tessellation of displayed outlines.

Next steps: extend the golden corpus and visual checks, cover nonstandard
primitives/templates and font metrics, extend basic editing to vertices and groups,
and introduce a connectivity/ERC compiler. PCB import remains a separate next
stage at the user's request.
