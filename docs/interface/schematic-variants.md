# Project assembly variants - 2026-11-02 checkpoint

This checkpoint was stopped by the user before completion of the full variant
stage. Implemented behavior and remaining interoperability/UI work are distinct.

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

## Explicitly unfinished

- This is not full Altium variant parity. Final real-window QA of PCBCraft,
  exact dialog dimensions/typography/icons, full grid/multiselect behavior,
  unresolved UID repair, complete native drawing styles and reporting remain.
- Newly selected alternative graphics are persisted in PCBCraftAlternateDataN
  extension fields for PCBCraft reopening. Authoring a native .PrjPcbVariants
  library for those edits is not implemented; Altium will not obtain the new
  symbol from these extension fields. Existing native library data is read only.
  Native alternate-library identity matching is bounded and can be unavailable.
- Physical paths from repeated sheet references are distinguished, but native
  Repeat()/annotation, fully folded multipart physical components and complete
  channel variant semantics are not implemented. Repeated-sheet project PDF
  refuses missing explicit channel pagination rather than silently duplicating it.
- Native global variant-parameter encoding and new-alternate native reopening
  have not been verified in Altium. Native values containing a pipe/newline are
  refused on save. Unknown unsupported kinds remain subject to validation.
- BOM, placement, PCB 3D, assembly drawings, fabrication variants, OutJob variant
  execution and library-update workflows are outside this completed checkpoint.
- Latest validation and any known failures are recorded in PLAN.md.
