# Schematic annotation — 2026-10-22

Entry point: **Tools > Annotation > Annotate Schematics...** in a schematic editor.
The Annotate dialog operates on all schematic source documents of the active
project, including sheets not previously open. A free document annotates only
itself. Missing project sheets stop annotation rather than allowing a potentially
colliding partial-project result. Other projects and boards are not changed.

## Workflow

1. Choose Down Then Across, Across Then Down, Up Then Across or Across Then Up.
   Ordering uses component origins in sheet coordinates, with stable identity as
   the tie-breaker. Change sheet order with Up/Down and set each start index.
2. Enable target sheets, optionally select Selected Parts Only. Disabled sheets
   still reserve their designator numbers. Existing numbers are preserved unless
   Renumber existing designators is enabled; duplicate movable designators are
   assigned free numbers. Number allocation is global per prefix across sheets.
3. Reset All proposes `?` for eligible designators and selects renumbering for the
   next Update. Reset Duplicates proposes `?` only for conflicting movable
   packages. These operations change the preview, not the open documents.
4. Update Changes List computes the proposal. Review Current/New Designator,
   sheet, library reference and status. Proposed names can be edited manually;
   changes-only filtering does not remove other rows from collision checking.
5. Accept Changes (Create ECO) opens Engineering Change Order. Validate Changes
   checks the entire project proposal. Execute Changes rechecks all source
   revisions before applying any change, then commits one Undo step per changed
   sheet. Repeated Execute is disabled. Cancel before Execute leaves models and
   histories unchanged. Undo/Redo is performed on each changed document.

Locked designators and excluded packages are preserved. Existing multi-part
packages keep their common base designator and part letters. A locked/excluded
section protects the package. Optional grouping packs compatible unannotated
sections into packages without changing section IDs; repeated section numbers
start another package. Compatibility requires matching library/source, part
count, design item, comment, models and parameters. Unknown part-suffix formats
are rejected instead of guessed. Graphical components (kind 2) are excluded.

Apply updates visible/hidden designator text and raw display values, preserving
component IDs, original native Unique IDs, source records and source archives.
Native source is marked modified. UID-based Cross Probe continues to resolve
renamed components. PCB designators and designator-only fallback links are not
updated by this schematic command; synchronization with PCB is a separate task.
Edited schematics use the application's existing **PCBCraft JSON** save path;
there is still no native SchDoc writer.

## Limits of Altium parity

The menu path is implemented as requested. No live Altium 24.4.1 observation was
available to verify this dialog's exact dimensions, controls or all edge cases.
This is logical source-sheet annotation, not compiled multi-channel/physical or
board-level annotation. Repeated hierarchical source sheets are annotated once.
Custom parameter-priority sorting, advanced part swapping, custom section suffix
schemes beyond A–Z, quiet/back annotation, project setting persistence, annotation
reports and direct native SchDoc saving are not implemented. ECO covers component
designators only, with per-sheet rather than global multi-document Undo.

The engine is in eda-core, and the app only collects scope, previews and commits.
Allocation uses bounded counters and lookup tables rather than restarting a linear
number search for each component. The changes table virtualizes visible rows.

## Validation

247 workspace tests passed, formatting and strict Clippy passed. Core regressions
include 10,000 components, per-prefix counters, sheet starts/exclusions/selection,
locked/duplicate names, leading zeros and punctuation prefixes, multipart package
identity, source staleness, hidden text, native UID/source preservation and history.
App tests cover project execution and refusal before any partial commit.

Real Linux GUI: Tools submenu, Reset/Update with start 100, proposal table, ECO
Validate/Execute, Undo/Redo and saved JSON verified on the editing-demo copy.
Component IDs stayed unchanged. TI PMP23595 loaded five source sheets and 167
logical components: default Update proposed no changes; full renumbering passed
ECO validation, then was cancelled. TI source files were neither edited nor added
to the repository.
