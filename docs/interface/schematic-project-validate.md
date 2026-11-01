# Schematic Project / Validate

## Observed in Altium Designer Enterprise 24.4.1

Windows Computer Use, 2026-11-01, maximized 1707 × 1026 logical window on the
same desktop used for PCBCraft QA. The user's schematic was not edited or saved.

- Project starts with **Validate PCB Project ReferenceB.PrjPcb** when a project
  sheet is active; a free document shows **Validate**.
- The following rows are **Cross Probe Next Message**, Shift+Ctrl+F3, and
  **Cross Probe Previous Message**, Shift+Ctrl+F4. A separator follows.
- The project popup is approximately 305 pixels wide. The remaining menu order
  is recorded in the implementation; commands outside this task remain disabled.
- Validate runs without a result dialog. A warning-only run left Properties
  active. Opening Messages showed Class, Document, Source, Message, Time, Date,
  No. columns, a large result list, and Details at the bottom above dock tabs.
- ReferenceB validation produced three yellow warning rows and one green
  `Compile successful, no errors found.` row. The first warning was `Net NetC6_1 has no driving
  source`, followed by a pin list. Details included the diagnostic and electrical
  objects. The accessibility tree confirms Source **Compiler**.
- Further observation on a separate complete project copy confirmed the Error
  Reporting groups and all 17 Connection Matrix types: eight pins, four ports,
  four sheet entries and Unconnected. Passive/Unconnected is No Report.
- Messages context menu: Group By, Cross Probe, Cross Probe Next Message,
  Cross Probe Previous Message, Place Specific No ERC for this violation...,
  Clear All, Clear Selected, Clear Class, Save..., Save Selection...,
  Export To Report..., Copy, Print.... The alias message includes named object
  kinds and counts: `Nets Wire ExampleGround has multiple names (Net Label ExampleGround,
  Net Label MMi_GND, Power Object GND (23))`. Driver Details uses short pin names.
- Controlled copies with changed native TypeN levels caused an Altium index
  error and runtime error 216; the Altium process subsequently exited. Attempts
  to recover capture encountered `window crop is outside captured monitor` and
  `foreground window did not report a process id`. Restart through Computer Use
  did not expose a targetable window. No TypeN-to-rule mapping or unsaved-session
  recovery is claimed. Original PrjPcb/SchDoc hashes remained unchanged.

## Implemented

- Read-only validation in `eda-core::schematic_validation`; no GUI/GPU, file
  writes, automatic annotation, project changes or history transactions.
- Active project snapshots use current unsaved documents and new project sheets;
  closed project schematics are read without opening tabs. A free schematic can
  also be validated locally. Missing/import-failed documents and unsupported
  compilation semantics produce fatal diagnostics, without a success result.
- Shared project compiler supplies electrical islands, including pinless islands,
  so ERC and netlist/ECO use the same connectivity, hierarchy, bus/harness,
  compile-mask and project naming semantics and limitations.
- Unannotated and duplicate designators, compatible multipart grouping,
  unconnected pins, one-pin nets, missing driving sources and electrical pin
  type conflicts. All 17 rows/columns of a native PrjPcb ERC connection
  matrix control pin, port, sheet-entry and unconnected severity. Invalid/truncated matrix input is
  rejected. Validation retains one-pin nets regardless of output omission flags.
- A missing driver is reported for nets containing inputs, rather than every
  passive-only net. This distinction was checked against ReferenceB's two native driver
  warnings. Net aliases produce a warning and validation continues; ordinary
  ECO/netlist compilation retains its conservative alias refusal. Driver pin lists
  use natural designator order. The read-only `eda-io` example
  `schematic_validate` provides an independent invocation of the same checks.
  On ReferenceB it reports exactly the three observed electrical problems:
  NetC6_1 and NetC13_2 without drivers, and ExampleGround/MMI_GND aliases. The source
  PrjPcb and SchDoc SHA-256 hashes stayed unchanged during PCBCraft QA.
- Generic No ERC at pin hotspots suppresses those pin checks; Generic No ERC on
  a wire suppresses diagnostics for its electrical island. Expanded masks exclude
  covered objects and directives. Rule-specific No ERC subtypes are unsupported.
- Project menu order, dynamic validation label, custom Validate icon, compiler
  columns and severity squares, resizable columns, horizontal/vertical scrolling,
  resizable Details, tooltips, multiple selection, column sorting, grouping,
  Copy, Save/Save Selection as tab-separated text, Clear All/Selected/Class.
  Export To Report, Print and specific No ERC remain disabled. Export text format
  and grouping behavior are implemented but not yet compared with native output.
  Double-click a diagnostic or Details target
  to open/focus its schematic object. Next/Previous shortcuts wrap over diagnostic
  rows that have targets. Later application messages append with their own
  timestamps and WorkspaceManager source; rerunning Validate replaces results.
- Warning-only runs preserve the current Properties tab; error/fatal results show
  Messages. This automatic error-panel behavior is an implementation choice,
  pending native verification. Later native controlled-level probes confirmed
  that Error/Fatal runs have no summary row. The implementation now follows this
  behavior; Warning-only runs retain the native success text. Error squares are
  orange and Fatal Error squares red. These latest UI adjustments were not
  rechecked live before the user requested stop and commit.

## Explicit remaining differences

**This is partial validation, not full Altium ERC parity.** Every run includes a
coverage warning to prevent a successful compiler result being mistaken for a
complete electrical check.

Remaining native `Electrical Rules Check / TypeN` reporting levels beyond
the independently confirmed Type10 (no driving source) and Type14 (multiple names),
rule-specific No ERC, variants/channels, all native ERC
rules, dynamic on-canvas violation markers, Messages native report/printing,
exact grouping/export formats, compiler object trees under Projects, cancellation and
background scheduling are not implemented. Unsupported existing compiler
semantics (including imported hidden-pin connectivity) report failure. Some
compiler errors have no object target. Native message wording, ordering, severity
defaults, pair grouping, driver rules, panel auto-show conditions, keyboard/menu
mnemonics, icon rasterization and exact pixel/font parity remain incomplete.
The 17 matrix categories follow the observed native order and were tested locally;
an exhaustive native conflict-message comparison has not been performed.
Linux runtime/DPI parity has not been visually verified.

## Verification

Regression coverage includes immutable models/history, unsaved project snapshots,
closed-sheet lazy navigation, missing-file failures, repeated runs, selection and
navigation wrapping, real egui panel layout, annotation/multipart checks, global
power connectivity, masks, generic No ERC and native matrix parsing/severity.
Final commands and live GUI findings are recorded in PLAN.md.

## Resumed native reporting-level observation

After the user restarted Altium, four bit-coded level probes and two independent
inverse checks confirmed Type10 and Type14. Values 0/1/2/3 mean No Report,
Warning, Error, Fatal Error. Type10=0/Type14=3 produced only the fatal alias;
Type10=3/Type14=0 produced only two fatal driver diagnostics. Both Error and
Fatal runs omitted the success row. PrjPcb import now retains valid native
reporting levels, applies these two confirmed mappings, and rejects invalid
levels. Other rule IDs are retained but not interpreted. Regression tests cover
section isolation and independent suppression/severity for the two rules.

The prior generated copies also changed TypeN keys in Modification Levels and
Difference Levels, which use different value ranges. Restricting edits to the
Electrical Rules Check section restored successful native loading. The original
user project was not edited. Native observation stopped immediately on the
user's instruction to stop and commit; specific No ERC inspection was unfinished.
