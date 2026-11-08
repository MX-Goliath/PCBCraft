# First-iteration verification

October 19, 2026, Windows, Rust 1.90.0, x86_64-pc-windows-gnu.
Toolchain installed in `.tools`; system PATH was unchanged.

## Automated checks

Windows debug build succeeded. `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings` and
`cargo test --workspace` succeeded through scripts/dev.ps1 with the local toolchain.
All 4 tests passed. Substantive scenarios: atomic rejection of invalid edits,
undo/redo revisions, history branching, JSON roundtrip and file replacement,
dirty state relative to saved content, camera roundtrip and preserving the point
under the cursor when zooming.

## Manual scenario

- [x] Launch the real wgpu window and inspect layout.
- [x] Select J1.1, change X from 10 to 12 mm, Apply, undo/redo.
- [x] Create PCB and schematic; switch tabs and Home.
- [x] Hide/restore Properties through View.
- [ ] Fully verify manual resizing (automated drag did not confirm a size change).
- [x] Save document, close, open through Recent; verify the edit.
- [x] Cancel aborts closing a new PCB tab.
- [x] Exit with two unsaved documents: Save sequentially saves PCB and schematic,
      then closes the application. Both resulting JSON files checked.
- [ ] Separately check Discard.
- [ ] Check opening errors, grid, units, zoom/pan/fit.
- [ ] Restart and settings restoration.

Linux CI is prepared but has not run remotely yet.
Local WSL provides no installed Linux environment; none was installed.
Native Altium files and engineering equivalence were not checked.

## Local Windows environment

The original system had no available Rust/MSVC toolchain. A local Rust 1.90.0 GNU
toolchain was prepared for this session. GNU ld failed when starting the GUI
binary; the final build uses rust-lld through scripts/windows-linker.rs and LLVM
dlltool. This workaround is limited to .tools and scripts/dev.ps1; normal MSVC/
Linux builds do not use it. The first launch also revealed missing native wgpu
backend features; they are now explicitly enabled. The final build was relaunched
and inspected.

Examples/checks did not modify user project files in Altium.
GUI verification documents were saved inside ignored .tools.

## Visual contract and layout clarification

The original plan is preserved and linked to current tasks. The required exact
visual-copy target is recorded in AGENTS.md, PLAN.md and the original plan.
After shell changes, fmt, clippy, all 4 tests and build were rerun.
The window was launched and maximized to the same captured image size as Altium.
Messages/Properties switching in the shared right-hand area was checked.
Exact parity is not claimed; differences: interface/visual-parity.md.
The document open before rebuilding was saved in .tools/ui-before-layout.json.

## Properties and shared window styling iteration

Windows build, fmt --check, clippy -D warnings and all 6 tests passed.
Two new tests check snapping: radius/boundary, disabling, layer/visibility scope,
document immutability and projection onto a finite pad edge.
The same sources, including all test targets, passed
`scripts/dev.ps1 check --locked --all-targets --target x86_64-unknown-linux-gnu`.
This is cross cargo check, not Linux execution/linking. The first run exposed
Windows linker flags leaking into cross-check; dev.ps1 was fixed and the rerun passed.

The real Windows window verified new Board/Pad Properties, scrolling, the active
right-hand area's blue heading, absence of Apply and X applied with Tab:
10 → 12mm; after leaving the field, Ctrl+Z restores 10mm and geometry.
View → Layers verified hiding/restoring Top Layer; its pads/tracks disappear
from the canvas when hidden.
Final normal (1440×900) and maximized windows were reinspected, including Search
with vector icon, dense snapping table and Pad Stack preview.
Before each rebuild, open examples were saved to .tools/ui-before-properties.json,
.tools/ui-properties-qa.json, .tools/ui-density-qa.json and .tools/ui-padstack-qa.json;
Altium project files were unchanged. Unavailable engineering controls are disabled.

Title bar is shared Rust UI without Win32; double-click maximize and system
Alt+F4 close were checked. Drag/edge-resize/minimize and separate window-command
buttons require additional manual scenarios. Linux GUI, DPI, visual font equivalence
and exact pixel parity remain unverified. See interface/visual-parity.md.

## Altium project / schematic import iteration

Windows build, fmt --check, clippy --locked --all-targets -D warnings and
15 workspace tests pass. All source targets passed checks for
x86_64-unknown-linux-gnu; Linux linking/GUI have not run yet.
New checks cover ASCII/CFB scene equivalence, stable IDs, absolute child coordinates,
parts/display modes/hidden fields, UTF8/legacy/Cyrillic, negative fractional
coordinates, large sheets, UTF16 and portable project paths, missing/case-sensitive
references, quoted parameter indirection and cycles, zlib Storage, malformed
lengths/numbers/owners, unknown-stream archives and refusal to write JSON over native files.

Local native files (not added to Git):

| File/set | Reading result |
|---|---|
| ReferenceB.SchDoc | 3480 records, 64 components, 579 visible primitives, 100 wires, 115 visible pins |
| ReferenceISheet.SchDoc | 434 records, 6 components, 193 primitives, 17 wires, 62 pins |
| TIDA-01597_Schematic.SchDoc | 1767 records, 20 components, 325 primitives, 43 wires, 59 pins |
| AWR2944EVM | All 25 SchDoc files read; types 43 and Additional 225 flagged with warnings |

CFB and an ASCII ReferenceB copy saved from Altium into ignored .tools produced
identical object counts. This does not check full electrical equivalence.
The original embedded in GUI-saved JSON has the same SHA256 as the source SchDoc
on disk. User designs were not copied into public fixtures. New tests use the
project's own minimal.SchDoc; this fixture was also opened/inspected in Altium 24.4.1.

Windows GUI verified ReferenceB PrjPcb opening through Ctrl+O, the real-reference
tree, sheet display, R9 selection and its Cyrillic Properties, wheel zoom;
opening TIDA PrjPcb and another EVMHardware sheet through the tree creates a tab
with a separate scene. ReferenceB and TIDA schematics were compared with the
reference. Quoted special strings, IEEE Dot, rotated anchors, text scale, frame
alignment and zone direction were then fixed. Corrected TIDA was reinspected:
the complete title fits its frame, components/wires and embedded logo are visible.
ReferenceB Save As created the application's JSON with the original archive;
the native file was not the write destination. Saved JSON was opened through
Ctrl+O in the new build: scene and Import Information were preserved without a
loading error. GUI pan, all object styles and large performance scenarios remain unchecked.

During research, Altium exited after closing the project's fixture; cause unknown.
It was restarted and restored the original ReferenceB project.
No save over the user's SchDoc occurred. Opening a document and exporting a copy
do not confirm native-writer completeness (PCBCraft has none).
At that iteration this was a read-only viewer; later editing is described below.

## ReferenceB visual iteration — October 20, 2026

PASS: fmt --all -- --check; clippy --workspace --locked --all-targets --
-D warnings; test --workspace --locked (19 tests); Windows debug build;
check --workspace --locked --all-targets --target x86_64-unknown-linux-gnu.
Commands ran through scripts/dev.ps1 with local Rust 1.90 GNU.
Linux results mean type/target checks, not native linking or GUI.

Added preservation checks for arrows and their sizes, independent pin-name/number
fonts and sheet system font, IEEE 11 without placeholder, and scene in the
application's JSON. A separate regression preserves shared Designator/Comment
under an alternative display mode while hiding other-mode graphics.
Exact point-on-segment geometry is checked at endpoints, inside, outside/nearby,
reversed direction and large coordinates. Portable display paths without Windows
device prefixes are checked.

Repeat headless import of the ReferenceB original and ASCII copy: each has
3480 records, 64 components, 581 primitives, 100 wires, 115 pins.
The shared-parameter visibility fix restored XP1 and its Comment, previously
excluded by symbol mode; pin-owned Net Name remains metadata rather than a
separate sheet string. Type 3 no longer warns in this file: its Dot and High
Current are implemented. Original SchDoc remains 823296 bytes,
LastWriteTime October 2, 2026 17:46:04, SHA256 redacted-source-sha256.
It was not overwritten. Temporary demonstration documents were saved separately
in ignored .tools before restarts.

Real windows were checked at the same full-sheet view and 1.5× zoom: DA2,
pin names/numbers, TP25/TP29, LED, vertical GND and title block.
After restoring default layout, canvas is x335..1385; paper approximately
y190..884, Editor footer y971..995, matching the reference landmarks.
Theme loss on Restore Default Layout was found/fixed; it now also resets camera
fit. Exact pixel parity is unconfirmed; observations, font-fallback inferences
and residual differences are recorded separately in interface/visual-parity.md.
Electrical equivalence was not checked.

## Basic schematic editing — October 20, 2026

PASS through scripts/dev.ps1 (Rust 1.90 GNU): fmt --all -- --check,
clippy --workspace --locked --all-targets -- -D warnings,
test --workspace --locked (27 tests), Windows debug build and
check --workspace --locked --all-targets --target x86_64-unknown-linux-gnu.
Linux GUI/linking were not checked locally; CI for both operating systems remains.

Added command checks: preview leaves model/history unchanged; group Delete,
child-primitive rotation/movement, Designator/Comment, Undo/Redo;
stale revision, partially invalid Batch and out-of-range edits are wholly rejected.
Clipboard transfers UUIDs/owners/fonts. Snapping uses the electrical pin end,
not its origin at the symbol body.
Import regression: CFB → Move/Rotate/Properties → JSON → Load, comparing archive
and original records, Undo to the initial source_modified flag.
App interaction tests check cancellation of preview/unfinished wires, a wire as
one transaction, fast drag release and stale Move.
A renderer regression preserves the initial hit at mouse release, including
press/move/release in one frame; events are read without nested context input locking.

Real Windows window: new schematic; R1, cancellation of next placement,
Properties Comment 10k→47k updates sheet text; Move, Space, Ctrl+Z/Y;
Ctrl+C/V creates R2; Ctrl+W and a wire between electrical pin ends.
Saved JSON inspected on disk: wire ends (114300000,137160000) and
(162560000,137160000) nm match pin endpoints. After rebuild/restart, reopening
the same JSON restored scene/values/wire. Fast mouse-drag of R2 moved the entire
group; Ctrl+Z restored its position and cleared dirty state.
C1, GND and Net1 snapped to a wire, plus Cyrillic text, were also placed.
This original example was saved through GUI to fixtures/schematic/editing-demo.pcbcraft.json.
The final build opened original ReferenceB.SchDoc: a fast drag moved imported R9
from Y=180.000006 to 152.500006 mm with its body/labels. Ctrl+Z restored scene
and removed the unsaved marker. The disk source retained SHA256 redacted-source-sha256.

Place/Drawing Tools menus were studied read-only in open Altium.
The user's Altium document already had an unsaved marker when observation began;
this iteration neither edited nor saved it. Group deletion was checked at the
Delete-command level; no user objects were deleted through Windows UI for testing.
Boundaries and unchecked states: schematic-editing.md and features.json.

## Schematic Properties, October 20, 2026 — intermediate iteration

PASS: fmt check, workspace/all-targets clippy with -D warnings, 31 workspace tests,
Windows debug build, workspace/all-targets cargo check for x86_64-unknown-linux-gnu.
Linux GUI was not run in this environment.

New regressions verify 316 px width across multiple frames for Document/Component/
Parameter/Pin with long fonts, models and values; copying retains consistent
displayed and original Designator.
IO/commands: hidden parameters and empty values, original Comment expression,
dependent-text recalculation, separate font, models, case-insensitive changes,
parameter deletion, atomic rejection, Undo/Redo, JSON and native-archive immutability.
Grid/electrical snapping can be enabled independently.

Windows real GUI: Document Options, Component R9, separate Parameter,
model/favorite Power/Resistance/link table, context switching.
R9 Comment ="R_USDD" replaced with 47k; Tab updated the sheet label.
After canvas focus returned, Ctrl+Z restored expression, label and clean state.
Original reference fixtures were opened to observe additional states.
The native user document in Altium was not changed/saved;
ReferenceB.SchDoc SHA256 remained redacted-source-sha256.

These checks **do not imply exact visual parity in all states**.
Remaining differences and unobserved contexts are listed in
interface/schematic-properties.md and features.json. Components/IntLib has not
started: the user's completion condition for Properties is not yet satisfied.

## Components / SchLib / IntLib — October 20, 2026

Following a new user instruction, Components began independently of remaining
Properties differences. The live AD 24.4.1 Components panel was observed with
supplied ReferenceB.IntLib. Map/limitations: interface/components-libraries.md.
Original ALTIUM_RUST_AGENT_PLAN.md preserved.

Checks: fmt --check, workspace/all-targets clippy -D warnings, 37 workspace tests,
Windows debug build, Linux GNU check --all-targets. The Linux check excludes
linking/GUI execution and confirmation of visual metrics.

Tests cover Binary SchLib/IntLib, parts/modes, binary pins, PinFrac, UTF-16
PinWideText, different label fonts, hidden/raw parameters, model links, image
Storage copying, fresh UUIDs/designators, rotation, repeated placement,
cancellation, revision conflict, Undo/Redo, save/reopen without a library and
refusal to write JSON over IntLib. Corrupt CFB/pins/zlib/checksum/auxiliary links
and invalid/excessive counts are rejected. A canvas regression verifies that
hidden parameters are absent from the rendered preview.

Real example: library_inspect reads all 31 symbols without warnings.
Windows GUI: Ctrl+O import, Results: 31, LMV321 search (1 result), 7086 preview,
1396 placement on a separate schematic copy as C7, Esc, Undo (back to clean),
Redo, Undo; library restoration after restart.
The user's open PCBCraft changes were saved separately before restarting:
.tools/ReferenceB-before-components-20261020.pcbcraft.json; original SchDoc not replaced.

Altium was used to read the panel/menus/library list without schematic editing
or saving. ReferenceB.SchDoc SHA256 matches before: redacted-source-sha256.
The supplied IntLib was read only; SHA256: redacted-source-sha256.
User schematics/libraries and extracted files were not added to Git.

Checks confirm the initial working import/placement path, not full compatibility
with every library format or exact Components parity.

## Stable schematic text during zoom — October 21, 2026

Removed recalculation of rounded font metrics at each zoom and jumps caused by
pixel snapping text origins. Labels, pin labels and text blocks are laid out at
fixed font size and scaled with the camera; wrapping is calculated from block
dimensions in document coordinates. UI-text pixel snapping is unchanged.

PASS: 113 workspace tests, fmt --check, all-targets Clippy -D warnings, build.
Two renderer regressions fail on old code and pass after the fix: they compare
actual tessellated glyph-mesh vertices in schematic coordinates across zoom,
9 anchors, 4 orientations and DPI 100/150/200%; line wrapping is checked.
Linux Xvfb/Mesa GUI: editing-demo, ordinary/rotated labels, zoom in/out;
after restoring zoom, the canvas region matches the pre-zoom capture byte-for-byte.
Raster glyphs may look soft at extreme zoom; this does not confirm exact Altium
visual equivalence or Windows GUI behavior.
