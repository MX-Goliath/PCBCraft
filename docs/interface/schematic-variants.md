# Project assembly variants

The 2026-11-07 follow-up extends the earlier checkpoint. This remains a partial
Altium implementation; observation, implementation and verification are separate.

## Reference observations

Altium Designer Enterprise 24.4.1 was observed on the existing research project:
Project / Variants opens Variant Management, with project components above and
component parameters below, variant columns, Add/Delete/Edit Variant, Drawing
Style, OK and Cancel. Projects contains [No Variations] and named variants; the
active entry is bold. Editor and a physical sheet tab appear below the schematic.
Selecting a variant leaves Editor as the logical design. The native manager also
reported stale links for VD7/VD9; this warning was observed, not repaired.
No native project or schematic was saved during the observation.

The native Desktop corpus confirms ProjectVariantN sections, component Kind=0
(fitted), 1 (not fitted), 2 (alternate), hierarchical UniqueId, ParamVariationN
(ParameterName/VariantValue), ParamDesignatorN and an independent
.PrjPcbVariants SchLib container. Earlier import only recognized VariantN names
and therefore missed real project variants. Legacy VariantN remains supported.

Primary references:

- https://www.altium.com/documentation/altium-designer/design-variants
- https://www.altium.com/documentation/altium-designer/variants-management-dialog
- https://www.altium.com/documentation/altium-designer/variants-in-design

The installed 24.4.1 observation takes precedence over newer documentation.

## Implemented

- GUI-independent project variant, component variation, parameter, alternative
  fragment and validated history models in eda-core. Duplicate variant names,
  identities and component entries are rejected before a history mutation.
- Native project import for fitted/not fitted/alternate metadata and component
  parameter overrides; unknown project/variation fields are retained. Known
  alternate symbols are decoded through the existing independent SchLib reader.
- Project / Variants manager: component/status grid, filter, varied-only filter,
  parameter overrides and reset, create/duplicate/rename/delete variants,
  variant parameters, local Undo/Redo, OK/Cancel and alternate-library selection.
- Active variant selection in Projects and the physical view toolbar, with
  per-project persisted selection. Switching views does not dirty the schematic.
- Separate read-only physical view, not-fitted cross/dimming, parameter resolution
  and italic modified text, alternate geometry with stable component/pin identity.
  Alternate pin numbers, connection points and electrical types must agree.
  Unavailable/incompatible alternate graphics produce an error rather than an
  apparently successful base-symbol substitution.
- Project variants have independent dirty state and history. Save/Save All
  writes variant data atomically to PrjPcb; Save Project As includes current
  definitions. External file changes refuse an overwrite. Quit/group replacement
  includes unsaved variants. Source SchDoc and PCB fabrication data are unchanged.
- Current physical view is used by print/PDF snapshots. Base netlist/ERC/ECO
  continues to include the physical components, including not-fitted parts.
  Catalog and view caches avoid loading project sheets each rendered frame.

## 2026-11-07 follow-up

Observed in installed Altium Enterprise 24.4.1 at 1707x1019/1026:
Variant Management has six project columns followed by variant columns, and
parameter columns for every variant. Add/Edit opens Edit Project Variant;
component cells open Edit Component Variation with Fitted / Not Fitted /
Alternate Part and Replace Component. Drawing Style offers cross or mesh,
not-fitted text, masking and modified parameter text/font. Fabrication and
paste-mask controls were observed, not implemented.

A disposable project copy in local/variants-qa was authored and saved in Altium.
Its native format establishes that the first variant's global parameters are
[Parameter2_1], [Parameter2_2], etc.; project parameters occupy owner 1.
The alternate cache is an ordinary SchLib named .PrjPcbVariants, with symbols
matched by AltLibLink_DesignItemID. AlternatePart can be empty for Kind=2.
Original schematic, PCB and integrated-library hashes were unchanged.
Altium reopened the generated alternate library and global parameter. Its native
reader requires variation records before parameter records and interleaved
ParamVariationN/ParamDesignatorN pairs. An ordered QA copy showed C1 alternate
1404, 4-11-00008 and 10nF in the manager and the green modified value in the
physical view. The Save As probe also exposed unsupported Windows verbatim paths;
project references now remove that prefix. Legacy ANSI Cyrillic metadata is
preserved even when document paths contain ASCII only.

Implemented in this follow-up:

- All-variant parameter grid, native column order, separate variant and component
  dialogs, clone with unique name, Ctrl/Shift row selection, bulk Fitted/Not
  Fitted/parameter/reset operations, local undo/redo and explicit UID relinking.
- GUI-independent commands validate the whole transaction. Exact UID matches
  take precedence over designator-only entries. Parameter names are case
  insensitive. History retains at most 128 commits. Undo normalizes selection.
- Drawing Style edits remain local until manager OK. Manager Cancel discards both
  style and variant changes. Cross/mesh, masked symbols, overlay text and varied
  parameter color/bold/italic/suffix are implemented as read-only projections.
- Native global parameter sections are read/written and reindexed on deletion;
  opaque parameter properties and base project parameter sections are retained.
  Legacy inline ParameterN fields remain readable. Private parameter UID fields
  preserve repeated-designator identity on PCBCraft reopening; ambiguous native
  values without physical identity are refused.
- Native alternate library authoring through the independent SchLib writer;
  Save/Save All/Save As include the companion library. Existing unused symbols
  and source container streams are preserved through the existing library
  writer's conservative checks. Unsupported multipart/display-mode/image or
  auxiliary data produces an error. Both project and companion have external
  change guards; project-write failure attempts companion rollback.
- Detailed CSV report includes states, overrides, global parameters and unresolved
  links. Assembly BOM CSV resolves alternate metadata and parameter expressions,
  excludes Not Fitted / graphical / No BOM items, and refuses unresolved links or
  multipart components requiring physical folding. Export is available from
  Detailed Report. CSV is an independent output format, not an Altium template.
- The variant_roundtrip example makes a separate native project/library copy for
  interoperability inspection. Current validation is recorded in PLAN.md.

## Schematic appearance correction — 2026-11-07

The previous checkpoint verified functionality but did not establish a visual
match. Further live AD24.4.1 inspection exposed these specific differences:

- Physical views mask the non-component electrical context. With No Variations,
  component parameter strings are masked too; designators and symbol bodies stay
  normally colored. Not-fitted symbols are automatically masked even when their
  optional graphics are off. Modified parameter styling takes precedence.
- Not-fitted crosses follow symbol/pin geometry, excluding parameter strings.
  A remote package-code annotation must not enlarge or move the cross. Gray Box
  and Mesh uses thin gray lines in both diagonal directions, not a red hatch.
- The observed changed-parameter font is Times New Roman 10, bold italic, green.
  Keeping the original GOST family and only changing italic was incorrect.
- Library substitutions keep the sheet's parameter coordinates, visibility,
  orientation and alignment. Values are resolved from alternate metadata and
  compared with the original sheet strings before applying changed-text styling.
- The physical Active Bar has five icon slots: Selection Filter, Select,
  Toggle Fitted / Not Fitted, Choose Component Alternate Part, Edit Component
  Variation. The former text dropdown/button strip was an invented layout.
- Active Bar overlays the working area. Reserving a 26-pixel strip above the
  schematic shifted the fitted page vertically; that extra strip is removed.

These rules are implemented in the independent projection and GUI. Untouched
old appearance defaults (including the earlier dim=false form) are upgraded;
customized styles are retained. Parameter
font family/size can now be set explicitly. The scene-dump example variant_scene
exports logical/assembly JSON for diagnosis without editing the source sheet.
The comparison also exposed verbatim Win32 prefixes in alternate-library links;
the native serializer now normalizes those links as well as document paths.
Exact rasterization, cross margins and all Active Bar interactions still require
separate verification; implementation alone is not evidence of pixel parity.
Final Windows QA confirmed the migrated style without manual changes (0.508mm,
Times New Roman 10, Bold and Italic), masked context, C2 cross, C1 changed value,
component picking/cancel/Undo and the corrected page center. Tests and remaining
limits are recorded in PLAN.md. Full pixel identity across zoom levels/DPI is
not claimed.

## Remaining parity gaps

- Exact UI/DPI/icon/typographic parity, native resize/sort/cell-range/clipboard
  behavior and complete drawing-style/font/preferences storage remain partial.
  Windows PCBCraft verification at 1707x1019 confirmed aligned variant columns,
  the component variation child dialog, Not Fitted, Undo/Redo, manager OK, native
  project/companion saving and assembly projection (red C2 cross, green C1 value).
  GUI-exported GuiVariantBOM.csv contains alternate C1 / 1404 / 10 нФ and excludes
  C2. The original unsaved group was saved as SessionSnapshot.DsnWrk before the
  authorized restart. The single updated GUI is left on the disposable QA copy.
  Schematic, PCB and integrated-library hashes still match their original copies.
- Native new-alternate writing covers one complete symbol part/display mode;
  arbitrary native library data may be refused. PCBCraftAlternateDataN also
  remains for exact PCBCraft geometry roundtrip. Native Altium reopening
  is verified on the bounded QA copy, separately from the independent reader.
- Repeat()/compiled channel annotation, fully folded multipart components,
  channel pagination and complete channel variant semantics remain incomplete.
  Project PDF still refuses repeated sheets without explicit pagination.
- PCB 3D/assembly drawings/placement/fabrication/paste-mask variant execution,
  OutJob integration, ActiveBOM templates and library-update workflows remain
  unsupported. Their controls remain disabled. Linux runtime visual verification
  is not established by the shared Rust layout or a type check.
