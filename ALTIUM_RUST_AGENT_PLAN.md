# Agent plan: independent Rust EDA with an Altium Designer interface

Date: October 19, 2026.

User clarification of October 19, 2026: the application must visually be an exact
copy of the available Altium Designer. Sections 7 and 25 define the required
Windows/Linux interface target; visual comparison starts with the first shell.
`PLAN.md` stores the current queue and results without replacing this full plan.
Unfinished criteria below remain unfinished.

## Accepted architectural decision

Create a standalone application with its own Rust core. The document model,
electrical connectivity, ERC, rules, DRC, interactive routing, pours and commands
belong to this project. KiCad, its IPC, internal structures and PNS are not used
as the execution engine. This supersedes the earlier research recommendation to fork KiCad.

Use general GUI, geometry, serialization, indexing and numerical libraries through
project-owned interfaces. Rust is the implementation choice. Demonstrate performance
advantages through measurements on specified projects and hardware.

Primary platforms: Linux and Windows. Keep the core independent of Win32 to allow
later macOS builds. Altium runs in a separate Windows session as a research
reference. The finished program performs engineering operations independently.

All checkboxes below represent future work. For an item with multiple requirements,
check it only after satisfying all of them. Split items into tasks when necessary,
preserving the original criteria.

## 0. Organize agent work

- [x] Create `AGENTS.md`, `PLAN.md`, `docs/architecture.md`, `docs/decisions/`, `docs/interface/`, `fixtures/` and `benchmarks/`. Record the independent Rust-core decision and third-party library boundaries.
- [ ] Maintain one feature registry. Record each feature's information source, implementation state, checks, known differences and dependencies. Track `documented`, `observed`, `specified`, `implemented`, `verified` and `unsupported` separately.
- [ ] Implement complete operations through model, command, interface and result verification. A button or `success` returned without changing the project does not complete a task.
- [ ] If the reference is temporarily unavailable, continue independent core tasks and documented features. Mark pending observations; do not present assumptions as measured Altium behavior.
- [ ] Separate observer, developer and verifier roles. Only one process controls a particular Altium GUI session. Parallel agents work on different modules after shared contracts are fixed.
- [ ] After each stage, preserve a buildable commit, launch instructions, minimal example project and results of substantive checks. Do not extend a feature while its basic-mode criteria fail.

## 1. Capture the reference and map the interface

- [ ] Record Altium version/build, extensions, available editors, language, theme, DPI, window size, units, grids, panels and keyboard settings in `reference_profile.json`.
- [ ] Create `ui_inventory.json` with `editor`, `surface`, `observed_path`, `caption`, `shortcut`, `enabled_when`, `selection_context`, `tool_state`, `command_id`, `evidence` and `verification_status`.
- [ ] Inspect the main screen and editors separately. Record actual menus, panels, context actions, property tabs and modes. File, Edit, View, Place, Design, Tools, Route and Reports below are exploration categories, not guaranteed paths in every build.
- [ ] Observe every command with an empty document, valid selection, mixed selection, locked objects and an unfinished operation. Record availability, defaults, confirmation and cancellation.
- [ ] Collect screen, input events and structural project state before/after actions. Check DelphiScript and available object interfaces; use `eda-agent` only after verifying required operations on the selected build.
- [ ] Create minimal reference projects for wires/labels, component libraries, a simple PCB, rule priorities, routing obstacles, pours and variants. Do not investigate every feature on one large board first.
- [ ] Check whether the recorder affects the document. Separate reads from compile, repour, model updates and other actions that may affect measurements.

Interface organization and observation sources: [Altium design environment](https://www.altium.com/documentation/altium-designer/design-environment), [QuickNav Panels](https://www.altium.com/documentation/altium-designer/quicknav/panels), [scripting examples](https://www.altium.com/documentation/altium-designer/scripting/script-example-analysis), [eda-agent](https://github.com/salitronic/eda-agent).

## 2. Create a minimal Rust workspace

- [ ] Start with `eda-core`, `eda-geom`, `eda-io`, `eda-render`, `eda-app` and `eda-automation` modules. Extract separate rules, connectivity, routing, pour and CAM crates as contracts emerge; do not create dozens of empty packages in advance.
- [ ] Pin compatible Rust toolchain/dependency versions in `rust-toolchain.toml` and `Cargo.lock`. Align egui, eframe, egui_dock and wgpu versions; do not independently upgrade each to its highest number.
- [ ] Use egui/eframe for the shell and integrate an original PCB/SCH canvas through a compatible wgpu backend. Keep one coordinated event loop and one graphics-device owner.
- [ ] Test the GUI stack early with a dense Properties table, docking, multiple native windows and different DPIs. A floating canvas panel and a separate OS window have different requirements; implement missing support before constructing the interface at scale.
- [ ] Add Linux/Windows builds, formatting, clippy and core checks without GUI launch. Use release configuration for performance measurements.
- [ ] Add `tracing` logging, Criterion measurements and `proptest` property-based checks for geometry, transactions and persistence. Do not create fragile snapshot tests for every decorative UI change.
- [ ] Define dependency boundaries. `eda-core` does not depend on GUI, windows, GPU or Python. UI/MCP call commands; the renderer reads the model view without modifying it.

### Initial dependencies

| Task | Candidates | Usage condition |
|---|---|---|
| Desktop GUI | `egui`, `eframe`, `egui_dock` | Custom styling and complex-panel verification; compatible versions |
| GPU | `wgpu` | Shared GUI integration; computational model is not stored in GPU buffers |
| Polygon operations | `i_overlay` | Project-owned wrapper, range/contour/result checks |
| Spatial search | `rstar` | Conservative bounding boxes and index-arithmetic checks |
| Tessellation | `lyon` | Display preparation; result is not authoritative DRC geometry |
| Documents | `serde`, `serde_json`, `uuid`, optionally `slotmap` | Stable external IDs separate from temporary storage keys |
| OLE containers | `cfb` | Container only; project code parses Altium records |
| Parallel computation | `rayon` | Immutable-snapshot jobs and deterministic result merging |
| Diagnostics and checks | `tracing`, `criterion`, `proptest` | Measurements and substantive invariants |

Links: [egui](https://github.com/emilk/egui), [wgpu](https://github.com/gfx-rs/wgpu), [egui_dock](https://github.com/anhosh/egui_dock), [i_overlay](https://docs.rs/i_overlay/latest/i_overlay/), [rstar](https://docs.rs/rstar/latest/rstar/), [lyon](https://github.com/nical/lyon), [lyon_tessellation API](https://docs.rs/lyon_tessellation/latest/lyon_tessellation/), [serde](https://github.com/serde-rs/serde), [serde_json](https://github.com/serde-rs/json), [uuid](https://github.com/uuid-rs/uuid), [slotmap](https://github.com/orlp/slotmap), [cfb](https://github.com/mdsteele/rust-cfb), [rayon](https://github.com/rayon-rs/rayon), [tracing](https://github.com/tokio-rs/tracing), [criterion](https://github.com/criterion-rs/criterion.rs), [proptest](https://github.com/proptest-rs/proptest).

## 3. Numbers, units and project-owned geometry

- [ ] Introduce `Coord`, `Length`, `Angle`, `LayerId` and unit conversions. Prevent implicit mixing of screen pixels, mm, mil, degrees and internal coordinates.
- [ ] Select the exact `Coord(i64)` scale in a separate ADR. Initial candidate: 10 pm, where 1 mm is 100 000 000 units and 2.54 nm is 254 units. Check suitability against input formats, coordinate range and arithmetic cost.
- [ ] Bound the coordinate domain and define rounding for each operation. Use checked wide arithmetic for intermediate calculations; prove predicate ranges and provide a wider fallback where `i128` is insufficient.
- [ ] Implement points, vectors, segments, circles, arcs, rectangles, contours with holes and local-to-world transformations.
- [ ] Implement distances, intersections, orientation, containment and transformations with analytical reference cases. Separately test tangencies, zero lengths, nearly coincident points, range limits and self-intersections.
- [ ] Wrap the polygon library with a project-owned API. Check coordinate-type constraints, winding, holes, offsets and result validity. Integer types do not automatically prevent overflow.
- [ ] Define error bounds for converting circles, arcs and offsets into polygons. DRC uses analytical predicates or bounded approximations with conservative limits; test cases near error boundaries separately.
- [ ] Add a spatial index. Check the index's own area/distance arithmetic; use wider types or conservative outward bounding-box rounding where needed.
- [ ] For rendering, convert camera-relative coordinates to a suitable GPU format. Zoom, DPI and tessellation do not change original document coordinates.

Altium binary-coordinate scale reference: [import-format description](https://dev-docs.kicad.org/en/import-formats/altium/index.html). This is a format-information source; KiCad code/core are not used. Check the library's integer-range constraints against [i_overlay](https://docs.rs/i_overlay/latest/i_overlay/).

## 4. Document model, commands and history

- [ ] Implement Project, Schematic, Board, library definitions, component instances, pins, pads, nets, layers and references. Allow incomplete but structurally valid projects.
- [ ] Separate persisted-object UUIDs from internal indices and generational handles. Moving, renaming, undo and synchronization must not replace object identity.
- [ ] Separate document, project, camera, selection, panel and active-tool states. Zoom/selection changes are not electrical-model changes.
- [ ] Create typed commands and queries. Commands contain unit-aware arguments, document, expected revision and operation ID; results contain changes, diagnostics and new revision.
- [ ] Define `operation_id` idempotence for timeout retries. The same ID/arguments returns the recorded result without a duplicate object or undo step; reject a different payload with the same ID. Specify journal retention duration/scope, including session recovery.
- [ ] Make changes in one transaction. Check reference integrity before commit; errors leave no half-operation. Reject structural errors; report permissible engineering violations as diagnostics.
- [ ] Implement undo/redo over committed changes. Undo restores data but creates a new monotonically increasing revision; old background results do not become current again.
- [ ] Implement a separate preview model. Mouse movement changes provisional state; commit creates one history step; cancel destroys preview without document changes.
- [ ] Bind preview to its source revision. If another command changes the document, recalculate or reject commit while preserving the other committed edit. Rejected commits leave document/history unchanged.
- [ ] Add state queries and canonical snapshot export. GUI, CLI and MCP must produce identical changes for the same command and inputs.

## 5. Saving, recovery and early format verification

- [ ] Define an original documented format with schema version, stable IDs, resource references and migrations. Do not use Rust-structure memory layout as a long-term format.
- [ ] Preserve exact numbers without intermediate JSON `f64` conversion. For limited-precision clients, agree on an integer representation such as decimal strings.
- [ ] Implement atomic writing with platform-specific replacement handling, dirty state, crash recovery and clear unavailable-resource messages.
- [ ] Check save/reopen, interrupted writes, unknown document versions and broken references. Unsupported data must not silently disappear.
- [ ] Run an early native experiment on a few Altium objects: read container/records, retain original fields, modify a supported object, write and open the result in the reference.
- [ ] Research native IO alongside the first canvas. Reference unavailability does not block the standalone program; resolve precision/representability questions before fixing relevant file-schema parts.
- [ ] Track independent support levels: import, native writing, preservation after editing and cross-format exchange. Define validity of retained unknown records after related objects change.

## 6. First working canvas

- [ ] Implement camera, pan/zoom, grid, axes, coordinate status, units and exact screen-to-world mapping. Check different DPI and desktop scaling.
- [ ] Draw lines, arcs, contours, holes, pads, labels and selection. Separate document layers from graphics display order.
- [ ] Implement hit testing against real objects with zoom, overlap and type filters. Snapping returns an exact model point, not only a visual indicator.
- [ ] Add culling, tessellation caching, geometry batching and partial GPU-buffer updates. Measure benefits before complicating storage structures.
- [ ] Create a first runnable example: open a document, add/select objects, edit parameters, move, undo, redo, save and reopen with identical data.
- [ ] Close this stage only when GUI and headless-command results match. All subsequent tools must use this model rather than separate canvas demonstration objects.

## 7. Altium shell: File, View, Projects and Preferences

- [ ] Reproduce observed layout: menus, toolbars/Active Bar, document tabs, Projects, Properties, Messages, status bar and panel management.
- [ ] Implement observed docking, resizing, tabs, floating and autohide. Persist layouts and restore windows while accounting for monitor-count changes.
- [ ] Implement project-document creation, opening, addition, exclusion and renaming; Save, Save As, Save All, Close and unsaved-change warnings.
- [ ] Implement recent projects, workspace-session recovery, project properties and diagnosable path errors. Distinguish excluding a document from deleting its file.
- [ ] Implement observed Preferences categories: units, grids, colors, editor behavior, keyboard and library paths. Record application/project/document ownership of settings.
- [ ] Make Properties contextual: document settings for empty selection, selected-object properties and active interactive-operation settings. Fields must change the real model or preview.
- [ ] Bind every action to `CommandId`. Menu entries, buttons, context actions and shortcuts call one handler with shared availability conditions.

## 8. Edit, selection, keyboard and object lists

- [ ] Implement observed single/rectangle selection, adding/removing objects, overlap cycling and type filters.
- [ ] Implement move, drag, rotate, mirror, delete, duplicate, copy and paste. Distinguish connection-preserving operations from free movement.
- [ ] Implement precise coordinate/parameter input while a tool is active. Handle confirmation, Esc, mode switching and repeated placement according to reference states.
- [ ] Implement mixed Properties values and bulk selected-object edits. Invalid edits must not silently apply to only part of a selection.
- [ ] Implement SCH/PCB List, Filter and search with object navigation. Visibility, masking, selectability and DRC participation are separate properties.
- [ ] Check keyboard behavior with focus in canvas, tables, input fields and dialogs. Global commands must not intercept text intended for field editors.

## 9. SCH Library: symbols and pins

- [ ] Implement library documents, symbol lists, graphics, pins, parameters, designator, comment and units.
- [ ] Implement electrical pin types, hidden pins, orientations and multipart components. Separate the shared logical component from its placed parts.
- [ ] Link footprints, SPICE and 3D models through project-owned references. Pin-to-pad mapping supports explicit correspondence and multiple pads per logical pin.
- [ ] Implement symbol copying, renaming, checking and saving, project-instance updates and conflicting-change reports.
- [ ] Verify that instance editing and library-definition editing have distinct, explicit effects; updates preserve intentional instance overrides.

## 10. Schematic: Place, Properties, Tools and Navigator

- [ ] Implement sheet settings, component placement, text, graphical objects, designators and instance parameters.
- [ ] Implement wires, junctions, net labels, power ports, ports, sheet symbols/entries, buses and logical signal harnesses according to observed contracts.
- [ ] Create an original schematic compiler. Distinguish geometric crossings from electrical connections; account for name scope, hidden pins and unfinished nets.
- [ ] Add hierarchy, repeated channels and annotation. Do not conflate sheet-definition identity with sheet/component instances.
- [ ] Implement ERC, Messages, Navigator and cross-probe. Diagnostics reference real objects and explain violation causes.
- [ ] Check the connectivity graph as sets of logical pins in nets. Separately cover crossings without junctions, identical local names on different sheets and repeated channels.

## 11. PCB Library and Pad/Via Library

- [ ] Implement footprint libraries, local coordinates, component origin, layers, dimensions, designator/comment and model references.
- [ ] Implement round, rectangular, rounded and custom pads, plated/unplated holes, slots and per-layer shapes.
- [ ] Implement solder/paste mask, overrides, tenting and pad/via templates. Separately inspect/implement required Pad Via Library operations, including corresponding `.PvLib` compatibility.
- [ ] Implement bottom-side transformations with correct layers, holes and orientations. Text fields and graphics follow their own reference rules.
- [ ] Add footprint wizards after defining parameters and verifiable output geometry. Generated footprints save and edit through ordinary commands.
- [ ] Check pin-to-pad mapping, project footprint updates and placement preservation. Identical numbers do not automatically describe all connections.

## 12. PCB: placement and basic objects

- [ ] Implement board outline, cutouts, components, pads, vias, tracks, arcs, regions, polygons, text and dimensions with observed Properties panels.
- [ ] Create a minimal two-layer stack: conductive-layer order/roles, two sides, physical thickness and plated through vias with explicit spans. This minimum precedes multilayer routing and first manufacturing output; section 18 adds the advanced stack.
- [ ] Add nets, net classes, component classes, rooms, locks and selection filters. Preserve object membership during moving and renaming.
- [ ] Implement component moving, rotation, alignment, distribution and side changes. Check local/world coordinates separately.
- [ ] Build original actual copper connectivity. Matching NetId means an intended connection but does not prove a physical path between pads.
- [ ] Implement ratsnest and updates after copper changes. Account for vias, actual spans and conductive layers, excluding nonconductive crossings.
- [ ] Check placement/editing on boards with unfinished routing and DRC violations. Such projects remain editable and saveable.

## 13. Design: schematic/PCB synchronization, ECO

- [ ] Link logical schematic components to physical PCB instances using stable identity and explicit mapping.
- [ ] Build project comparison and change lists: add, remove, rename, replace footprint, change pin/pad mapping or nets.
- [ ] Implement an ECO window with prevalidation, selective application and actual-result reports. Errors leave no unaccounted partial changes.
- [ ] Preserve valid placement, manual properties and routed portions during updates. Renaming does not become an unjustified delete/add pair.
- [ ] Verify idempotence: resynchronizing without new changes offers no unnecessary ECO. Check rename, remove, alternate footprint, undo and save/reopen.

## 14. Design Rules, Constraint Manager and DRC

- [ ] Implement an original typed query parser/evaluator: object types, nets/classes, layers, regions, logic and units.
- [ ] Implement object/object-pair rule application, priorities, overrides, inheritance and explanations of effective-rule selection.
- [ ] Create a classic rule tree and object-centric constraints as separate representations of observed modes. Do not substitute a table without inheritance/persistence for Constraint Manager.
- [ ] Start DRC with shorts, clearance, unconnected, width, drill and board boundaries. Extend to masks, annular rings, hole distances and remaining observed rules.
- [ ] Add full/incremental DRC. Bind results to document, rule and derived-data revisions; discard stale background results.
- [ ] Implement violations panel, filters, check settings and error navigation. Separate checking from tool policies that prohibit creating particular violations.
- [ ] Check overlapping rule scopes and tolerance boundaries: just below, exactly at and just above a value. Compare full/incremental results on the same snapshot.
- [ ] Implement `.rul` and `Constraints.xml` only after the original rules format and semantic correspondence checks. Track changed but unsaved constraints separately.

## 15. Route: original interactive routing

- [ ] Implement tool state machine: start, width/layer selection, movement, angle, preview, segment confirmation, finish and cancel.
- [ ] Implement manual single-track routing on one layer. Check snapping, width, copper shape, NetId, endpoint-object connections and a single undo step.
- [ ] Add obstacle stopping and explicit allowed-violation policy. Final diagnostics match the chosen tool mode.
- [ ] Implement single-layer walkaround with original pathfinding and clearance API. If no solution exists, leave the document unchanged and explain why.
- [ ] Implement existing-track drag preserving intended connections. Handle fixed objects, branches, corners and impossible moves.
- [ ] Implement shove on one layer first. Perform displacement chains in preview, bound search work and completely roll back failed attempts.
- [ ] Add multilayer routing after stackup and valid via-span models. Check electrical connectivity through transitions and neighboring-layer constraints.
- [ ] Implement differential pairs: polarity, joint geometry, gap, paired vias, uncoupled sections and each actual track's length.
- [ ] Add multi-routing, geometric length/skew tuning, gloss and retrace as independent commands.
- [ ] Add delay tuning after the verified delay model in section 20. This later mode does not block geometric tools or the first working board.
- [ ] Check the router against independent simple geometric references and connectivity invariants in addition to shared clearance code. One bug must not both construct a route and certify it.

## 16. Polygons, planes, stitching and shielding

- [ ] Separate polygon definitions from computed copper. Persist outline, net, priority, thermal/island parameters and pour settings independently of cache.
- [ ] Implement obstacle/cutout subtraction, rule-based clearances, holes, polygon intersections and calculation order.
- [ ] Implement thermal relief, direct connect, island removal and resulting copper connectivity to pads/vias.
- [ ] Implement observed repour, shelve/restore and local updates. Rule/stack changes invalidate dependent copper.
- [ ] Add internal planes, stitching and shielding after verifying connectivity and via spans. Commands create ordinary editable objects.
- [ ] Check area, topology, net membership and DRC. Different vertex order or region partitioning is acceptable when engineering results match.

## 17. First complete board-creation workflow

- [ ] Assemble this scenario from completed basic capabilities in sections 8–16. Hierarchy, complex shove, differential pairs, plane modes and other extensions do not block the first simple board; unfinished tasks remain unchecked in the plan.
- [ ] Prepare a verification project with an original schematic, library components and two-layer PCB. Record component count/features in the fixture.
- [ ] Complete schematic creation, ERC, ECO, placement, single-track routing, vias, polygon and DRC through the real interface.
- [ ] Implement initial Gerber, Excellon, BOM and pick-and-place export from the project's model. Outputs contain actual project data.
- [ ] Parse exports with an independent reader and compare outline, copper, holes, units, layers, positions and rotations with the source model.
- [ ] Exercise undo/redo for substantive operations, close, reopen and regenerate outputs without unexplained changes.
- [ ] Record a working build/scenario. This is a checkpoint before expanding expensive subsystems; full functional coverage remains the final goal.

## 18. Layer Stack Manager, HDI and rigid-flex

- [ ] Implement materials, thicknesses, copper/dielectric layers, physical Z coordinates and the observed stack table.
- [ ] Add blind/buried/microvias, backdrill, allowed layer pairs, transition sequences and drill checks.
- [ ] Implement substacks, flex regions, bend lines and radii. Link board regions to physical stack and generated geometry.
- [ ] Implement stack-setting and impedance-profile import/export for an explicitly declared format subset.
- [ ] Check stackup integration with DRC, router, 3D, delays and manufacturing outputs. Table changes must update all dependent results.

## 19. 3D Layout and ECAD/MCAD

- [ ] Implement a separate 3D Board view with the same IDs, coordinates, layers, holes and components as the 2D editor.
- [ ] Add component models, local transforms, footprint links and accurate top/bottom placement.
- [ ] Implement observed camera, sections, visibility, selection and 2D/3D cross-probe operations.
- [ ] Separate preview meshes from exact exchange geometry. Give STEP a project-owned adapter; an external geometry library does not replace the Rust board model.
- [ ] Add mechanical collision/height checks. Check unfolded/folded rigid-flex views and model updates after source-board edits.

## 20. High-speed, xSignals and impedance

- [ ] Implement electrical paths, xSignals/xNets and constraints. Distinguish object geometric length from the selected pin-to-pin path length.
- [ ] Add transition lengths and pin-package contributions with explicit units/provenance. Check branches and series components.
- [ ] Implement Rust models of common transmission lines with declared applicability. Compare analytical cases and independent solvers using identical materials/assumptions.
- [ ] Add impedance/delay matrices/profiles only after numerical verification of the calculation path. Panels must not return preset values as computed results.
- [ ] Implement delay tuning and related DRC constraints over verified parameters. Update results after stack, path and component-model changes.

## 21. Variants, BOM and components

- [ ] Implement fitted/not fitted, alternate components, parameters and observed repeated-channel variants.
- [ ] Implement BOM aggregation, quantity, grouping, columns, substitutability and parameter provenance display.
- [ ] Align variants across BOM, placement, 3D and assembly drawings. Do not treat an assembly variant as arbitrary independent PCB fabrication geometry.
- [ ] Add library search/local indexing, reference checks and updates. Handle missing models and conflicting versions.
- [ ] Integrate prices, stock and lifecycle through separate data adapters; display provenance/date and keep local projects usable offline.

## 22. Simulation and SI/PI

- [ ] Implement symbol/pin/computational-model mapping, simulation input generation and analysis settings.
- [ ] Define the computational-backend contract: inputs, model versions, parameters, launch, cancellation, errors, units and results. An external solver, if used, is a separate adapter; the EDA core remains project-owned Rust code.
- [ ] Start with verifiable DC, AC and transient examples, then add sweep, measurements and observed analysis workflows.
- [ ] Implement result viewing: traces, axes, cursors, units and numerical export. Displayed plots come from an executed calculation.
- [ ] Implement SI/PI as separate tasks with explicit extraction model, ports, materials and boundary conditions. Set different accuracy criteria for DC voltage drop, current density, heating and dynamic PDN.

## 23. Reports, OutJob, CAM and Draftsman

- [ ] Extend Gerber/Drill generators, assembly outputs, IPC-2581 and ODB++ for verifiable format versions. Track each reader/writer independently.
- [ ] Implement OutJob: sources, parameters, containers, variants, validation, execution order, errors and actual output-file lists.
- [ ] Implement CAM documents, layer/drill import, display, measurement and editing according to researched operations. CAM owns data rather than being merely a board image.
- [ ] Implement Draftsman documents, sheets, templates, views, dimensions, callouts, tables and formatting parameters.
- [ ] Link Draftsman views/dimensions to project objects. Board changes update geometry, variants and tables with lost-reference diagnostics.
- [ ] Verify independent opening of manufacturing outputs, units, polarity, apertures, masks, drill spans, rotations and quantities. Successful job status without verified files does not complete output generation.

## 24. Multi-board, harness and original Workspace

- [ ] Implement multi-board logical models, child projects, connector mapping, system connectivity and change synchronization.
- [ ] Implement multi-board assembly, transforms, mechanical relationships, collisions and system BOM from a consistent assembly model.
- [ ] Implement physical harnesses separately from schematic logical signal harnesses: wires, cables, connectors, splices, bundles, lengths, wiring and layout.
- [ ] Link harness wiring, layout and manufacturing documents. Check length, connection and BOM updates when sources change.
- [ ] Create a project-owned Workspace server for revisions, lifecycle, permissions, comments, conflicts and release records. Local project operations do not depend on server availability.
- [ ] Implement external integrations through documented adapters and supported-operation lists. Check concurrent changes, offline/reconnect and preservation of committed edits.

## 25. Performance and visual refinement

- [ ] Create small, medium and large benchmark projects with fixed objects, settings and seeds. Record CPU, GPU, RAM, OS, resolution, DPI, commit and release profile.
- [ ] Measure opening/saving, pan/zoom, selection, route-preview latency, full/incremental DRC, repour, peak memory and document size.
- [ ] Set initial interaction targets for a chosen medium board, e.g. 60 FPS and p95 route preview under 50 ms. These are design goals for a specified configuration, not reported results.
- [ ] Remove whole-document scans each frame, full geometry rebuilds and blocking UI-thread calculations. Optimize profiler-identified hot spots.
- [ ] Parallelize independent snapshot computations with rayon. Preserve deterministic result order, revision checks and cancellation of long jobs.
- [ ] Compare panel dimensions, spacing, fonts, colors, cursors, disabled states, scrolling, docking and tool transitions with the reference at agreed DPI.
- [ ] Check Windows/Linux separately: keyboard, clipboard, paths, drag/drop, scaling, multiple monitors, fonts and available GPU backends.

## 26. Completion rule for each interface feature

- [ ] Identify the observed editor, interface path, availability context and corresponding project-owned `CommandId`.
- [ ] Implement correct model changes/error messages, plus preview, cancel and undo/redo where applicable.
- [ ] Check save/reopen and dependent-data updates. State after undo has a new revision and does not accept old caches.
- [ ] Execute a substantive reference scenario and additional edge cases. Distinguish visual, electrical, geometric, file and numerical equivalence.
- [ ] Use independent references or invariants for algorithms. Do not change expected results merely to match the implementation.
- [ ] Record evidence, coverage, limitations and known differences. Check tasks only after meeting their criteria, without claiming universal parity from a limited example set.

## Next work package for the first launch

- [ ] Complete the minimum scope of sections 0–6 and the shared shell in section 7 without filling the interface with dummy features.
- [ ] Deliver runnable Linux/Windows builds, sources, a test document and instructions reproducing the first scenario.
- [ ] Demonstrate a real workflow: create document → add objects → edit parameters → move → undo/redo → save → close → reopen with identical data.
- [ ] Continue sections 8–17 to the first complete schematic/board workflow. Develop sections 18–25 after satisfying their engineering dependencies.

## Additional primary sources for Altium behavior

- [Traditional rules](https://www.altium.com/documentation/altium-designer/pcb/defining-scoping-managing-design-rules)
- [Constraint Manager](https://www.altium.com/documentation/altium-designer/sch-pcb/constraint-manager)
- [DRC](https://www.altium.com/documentation/altium-designer/pcb/drc)
- [Interactive routing](https://www.altium.com/documentation/altium-designer/pcb/routing/interactive)
- [Layer Stack Manager](https://www.altium.com/documentation/altium-designer/pcb/defining-layer-stack)
- [Variants](https://www.altium.com/documentation/altium-designer/design-variants)
- [Draftsman](https://www.altium.com/documentation/altium-designer/draftsman)
- [Multi-board design](https://www.altium.com/documentation/altium-designer/multi-board-design)
- [Harness design](https://www.altium.com/documentation/altium-designer/harness-design)
- [Gerber specifications and test data](https://www.ucamco.com/en/gerber)

Use the previous research package for sources, inventory and scenarios. Do not
carry its KiCad architectural guidance into this plan. No observation of a specific
installation, application implementation or performance measurement was performed
while preparing this document.
