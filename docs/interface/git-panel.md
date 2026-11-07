# Project Git panel

User-requested extension (2026-11-07), separate from the Altium parity work.
The layout follows the existing dock chrome and a GPT-6 Astra consultation
(`reasoningEffort=high`, `serviceTier=default`). These are design decisions,
not observations of Altium's version-control implementation.

## Workflow

- Open **Panels > Git** in the bottom status bar. Panels opens or activates a
  panel; selecting it again does not close it. Git, Projects, View Configuration
  and Library share persistent left-dock tabs and one resizable width (334 logical
  points initially). Close a panel with its header X, middle-click its dock tab,
  or use that tab's Close menu. Other open panels remain available. The selected
  dock tab does not change when switching SCH, PCB, revision or Home documents.
  Contextual panels show an empty state when the document is incompatible.
  Layer Stack Manager uses the same panel commands and tabs. Opening a working
  file or Git comparison returns to the document editor while retaining its
  pending stack draft in the Stackup tab.
  Git follows the active document's project automatically, including when first
  opened. Loaded project membership takes precedence over a sheet's subdirectory;
  free files use their own directory. Unsaved files belonging to a project use
  that project; untitled free documents have no repository context. Home retains
  the last browser, with the persisted folder used only as a startup fallback.
  Revision tabs retain their original repository and project-directory scope,
  including deleted files, so switching back to a diff restores its own project.
  Each project retains its history/filter selection, commit draft and pending
  Git job. A comparison finishing in another project's browser does not steal
  focus; it opens when that project becomes active again.
- The folder button accepts a directory or PrjPcb path, with an optional folder
  browser and shortcuts for open projects. This manual choice lasts until the
  active document changes or Git is reopened through Panels. The tree contains
  SchDoc, PcbDoc, SchLib, PcbLib, PrjPcb and DsnWrk files under that directory.
  Ignored files and unsupported Altium types are excluded. A directory is the
  project boundary; two PrjPcb files in the same directory share that scope.
- **Changes** compares HEAD against saved files, including tracked,
  untracked, renamed and deleted documents. **View > Changed files only** filters the tree.
  **View > Include unsaved editor edits** compares an immutable snapshot of applied
  edits in an open document instead of disk. Such rows have `*`; the review
  banner explicitly identifies the editor buffer. Pending property/placement
  drafts are not applied by opening a review.
- Checkboxes select saved files for the next commit. File selection opens a
  review independently of commit inclusion. A private Git index prevents other
  staged files from entering the commit. Open dirty selected documents must be
  saved first. HEAD changes, conflicts and active merge/rebase/cherry-pick state
  reject the commit. Selected index paths are reconciled after success; unrelated
  staging remains intact. Hooks/signing use the repository's normal Git setup.
- **Push…** names the destination and states that entire commits are sent,
  including files outside the panel filter. First push can select an existing
  remote and establish upstream tracking. Push never uses force. Fetch refreshes
  remote refs. Authentication uses the installed Git setup; interactive terminal
  and credential-manager prompts are disabled in the background job.
- **History** browses local/remote refs and tags without checking out a branch.
  It loads 100 commits at a time. Select a commit, then a file. Merge commits
  expose each parent; root commits compare against an empty base. **File history**
  uses Git's follow-renames log. The branch chooser affects browsing, not HEAD.
- File rows provide **Open working file** and **Open diff** in their context menu.
  An unchanged working-tree row opens its ordinary document directly; changed
  and history rows open a comparison. The review toolbar and document-tab context
  menu also offer **Open working file**. A working file and its historical copies
  have separate identities. Reopening the working file activates its existing
  editor, preserving unsaved edits. Deleted working files disable that action.

## Review workspace

Reviews open as ordinary document tabs beside working SCH/PCB documents. Multiple
files and revisions stay open together. A repeated immutable comparison reuses
its tab; reopening a working-tree/buffer comparison refreshes that snapshot.
Tabs retain independent cameras, layer visibility and display settings. Use the
normal tab strip, Ctrl+Tab / Ctrl+Shift+Tab, middle-click, the Close context menu,
or Ctrl+W / Ctrl+F4. Escape clears the selected change without closing the tab.

The same editor View menu, PCB layer strip, View Configuration, grid dialog,
mirror/single-layer shortcuts and 2D/3D camera controls operate on the active
review tab. View Configuration activates its left-dock tab, with Git still
available in the same tab strip. SCH grid visibility and units remain available.
Changes, Properties, Components and Messages use the same persistent right dock;
selecting Properties is no longer overridden by an active comparison. Changing
the Git repository preserves all open revision tabs and their original file paths.
PCB Ctrl+Home focuses the selected change, including its previous position;
Ctrl+PageDown and V, F / V, D retain the ordinary document/board fit commands.
Historical snapshots cannot be edited or saved over working documents. Editing
commands and placement toolbars are disabled. PCB 3D shows the Before or After
revision with the ordinary 3D viewer; the four-color overlay is a 2D comparison.

Overlay artwork is cached per review view, with resident GPU buffers when available.
Pan/zoom transforms cached geometry; zoom settling refines text and stroke sizes.
Layer/display settings, semantic filters, revision or library-member changes and
DPI/font changes invalidate the artwork. Change picking uses one canvas response
and cached visible bounds rather than one widget and a board scan per change.
The same cache serves SCH overlays; ordinary Before/After rendering remains native.
The read-only `eda-app` example `diff_performance` reports root-comparison CPU frame
times (including egui tessellation, excluding GPU/presentation) for supplied native
files and `--synthetic-sch` (20,000 added schematic rectangles).

Overlay uses red for removals, blue for translations, orange for parameter or
geometry changes and green for additions. Unchanged artwork uses neutral gray,
so native red PCB layers cannot be confused with deletions. The selected object's
previous position has a dashed contour; a selected move has a blue arrow. Combined movement/parameter edits
keep the movement flag and the orange parameter classification. Filters,
Previous/Next, Focus selected and Fit operate on the semantic change list.
Before and After retain the same camera; an absent revision displays an explicit
empty state. The right dock shows object changes and before/after property data. Physical
coordinates and dimensions have explicit millimetre units; long values can be
expanded and copied from their context menu. The Changes dock has a search field,
type filters, virtualized object rows and a separate Properties section; the
comparison toolbar's overflow menu can reopen this dock or show file history.
Existing review tabs remain captured snapshots after a commit. The toolbar marks
them as snapshots; reopening Changes captures the current files against the new HEAD.
Libraries expose the union of definitions/parts/display modes in both revisions,
including added and removed members. PrjPcb/DsnWrk use decoded text comparison.

## Implementation and limits

- `eda-io::git`: bounded Git CLI jobs without a shell, NUL-delimited file lists,
  literal pathspecs, explicit project scope and blob loading without checkout.
  Symlinks/submodules are not treated as native design documents. Jobs run on
  workers with a 120-second process timeout; errors remain visible in the panel.
- `eda-core::document_diff`: imported-model comparisons in document coordinates.
  Native UniqueIDs take priority. Byte-derived PCB IDs and native source indexes
  are excluded from semantic content; unique content/translation/position matches
  handle records without persistent identities. Ambiguous matches remain additions
  and removals. Translation means moving the whole shape without resizing it;
  rotations, topology and shape changes appear under Parameters.
- `eda-render::document_diff`: camera, tinted native geometry, dashed old bounds,
  movement arrows and canvas selection. The renderer does not mutate documents.
- Diff covers the supported imported 2D model and its exposed metadata. Unknown
  native records, external dependencies and arbitrary 3D asset changes are not a
  full binary/native semantic comparison. “No semantic changes” is explicitly
  limited to supported data. PCB layer colors in Before/After remain native;
  Overlay uses the review palette.
- Git must be on PATH. Remote creation, init/clone, checkout, pull/merge/rebase,
  conflict resolution, stash, destructive resets and force push are outside this
  viewer. No remote authentication or remote-network compatibility claim follows
  from tests with a local bare repository. Linux UI runtime is not verified here.

`cargo run -p eda-io --example git_review_fixture -- <new-directory>` creates a
synthetic native project, two commits with all four change classes, one saved
working edit and an isolated local bare remote. The generator refuses to overwrite
an existing directory and never reads a user design.

## Validation

Automated and actual-window validation results are recorded in PLAN.md. Pixel
parity with Altium is not claimed for this custom panel.

The 2026-11-07 visual revision replaces boxed commit cards and scattered actions
with flat history rows, fixed-height file rows with document icons and right-side
status markers, a compact repository toolbar, Changes/History tabs, and a bottom
commit form. The dock chrome follows the existing Projects/Properties panels.

## Native project regression

The user-supplied Bip_adaptive project was exercised in `local/PDU-git-review`.
Baseline: `05d76c0239afbefd1bbbbf3958786feeddf8b95d`.
Revision committed through the panel: `f0f9357136f0e2d481ee076766a2e3db718c7f06`.
The original folder was verified unchanged by SHA-256.

SCH: move R1 5 mm with connected wires, change C1 to 220nF, remove R11,
add C99. The saved native comparison has seven changes and all four colors.
PCB: move the revision label 2 mm and widen ECALL_SW from 0.3 to 0.4 mm.
Altium's native save also updates some coordinates, dimension strings and board
metadata, and removes two regions; the full comparison has twenty changes.
The viewer exposes these additional changes. Hidden schematic parameters still
participate in comparison but do not enlarge the visible selection frame.

The diagnostic example `git_native_review` requires a disposable copy marked
with `.git/pcbcraft-review`. With no flag it inspects the native documents;
`--compare` compares HEAD with saved files; `--apply` authors the SCH changes
once on a fresh baseline. Board changes for this regression were authored in
Altium on the copy because PCBCraft's native writer refuses unsupported repour /
reindex updates for this board. Viewing and Git commits do not invoke Altium.
Test assets and native source archives are local and are not checked into the
PCBCraft source repository.
