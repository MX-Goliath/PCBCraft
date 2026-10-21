# PCB v2: implementation and verification boundary

## Status

Integrated October 21, 2026. Linux Rust compilation, 109 workspace tests, strict
Clippy, all-targets build and representative GUI checks pass; details and exact
limits are recorded in [VALIDATION.md](VALIDATION.md). Altium interoperability
remains unverified. Windows real-board opening and the visual work are recorded
in [VISUAL_PARITY.md](VISUAL_PARITY.md); full visual parity remains unfinished.
No DRC, obstacle avoidance or push-and-shove
was added. A feature implementation is not a claim of full acceptance or parity.

## Layers

* `eda-core::pcb`: nm-coordinate scene, validated snapshot transactions, selection,
  transforms and legacy scene migration. Source bytes remain shared through `Arc`.
* `eda-core::pcb_polygon`: editable polygon definitions, boundaries/cutouts, priority,
  clearance, direct/thermal/none connections, shelving and island policies. `geo 0.31`
  booleans operate in millimetres and output integer nm. Arc/buffer operations are
  approximated; exact Altium fill/rule equivalence is not claimed.
* `eda-core::pcb_route`: an isolated manual routing draft, 45/90/free-angle legs,
  route-leg backtracking, original-track splits and layer-change vias. A finished
  connection commits once. No obstacle solver or differential pair/length tuning.
* `eda-core::pcb_tools`: component transforms/flip, shared endpoints and contour
  handles; connected drag stretches endpoints without preserving route angles.
* `eda-render::pcb`: layer painter/picking, draft route/outline, selection handles,
  bounded triangulation cache. Mirrored/rotated glyph geometry and native view
  preferences are supported; exact Altium stroke fonts and UI parity remain partial.
* `eda-app::pcb_editor` and `pcb_properties_v2`: menus, actions, modal tools,
  draft Apply/Revert properties, clipboard and native Save integration.
* `eda-io::altium_pcb::writer`: edited native record serializer and safe file replace.

## Polygon semantics

The authoritative new definitions are `Board.polygon_pours`. Legacy raw `polygons`
metadata is retained separately. `polygon_model_version` ensures migration runs
once: deleting all definitions is not interpreted as a request to restore them.

Repour removes generated geometry belonging to managed definitions, creates actual
cutout regions and fills in priority order. Lower numeric priority is first. It
clips to the board outline, subtracts foreign-net copper with the per-polygon gap,
subtracts keepouts/cutouts, and creates direct or thermal contacts on matching-net
pads/vias. Poured islands can be removed by connectivity contact or area threshold.

A cutout is an independent region on a layer: it can affect overlapping pours, not
only its owner. `PCBCRAFT_POLYGON_CUTOUT_OWNER` is supplementary ownership metadata;
the native cutout is encoded as `KIND=1` with no native polygon-owner reference.
Unknown readers may ignore the ownership property but still see the cutout region.
This behaviour remains unverified in Altium.

Native thermal/clearance rules are NOT evaluated. New per-polygon choices are
stored in `PCBCRAFT_*` properties and materialized as copper regions. Altium may
repour differently when using its own rule engine. Automatic repour after a geometry
edit may change originally imported fill geometry. Layers with inverse internal
plane semantics, custom padstacks, slots and advanced keepout restrictions are not
fully represented. Do not treat this as fabrication validation.

## Components, pads, tracks and vias

Imported component groups can be translated, rotated, copied and deleted. Component
properties update designator/comment text records with known roles. Flip mirrors
owned geometry and swaps mapped top/bottom layers. A local group can be created
from selected standalone primitives; this is not a PcbLib/IntLib footprint browser,
ECO engine or component-library lifecycle implementation.

Pads support common rectangle, circle/obround, octagon and rounded rectangles
with imported per-layer corner percentages, rotation, drilling, plating and
top/middle/bottom sizes. Radius uses half of the shortest side.
Full-stack arbitrary shapes, drill offsets/slots and individual corner masks are
not editable/rendered completely. Existing custom radius bytes are retained when
moving an otherwise unchanged native rounded pad.

Track/via geometry, net assignment, property Apply/Revert, moving/copying/deleting,
shared-endpoint handles and route transitions are wired to history. Constraints,
DRC, shove/walkaround and dynamic connectivity/ratsnest solving are not implemented.

## Native saving

Binary PCB6 CFB input remains binary. ASCII input remains ASCII. Cross-format
conversion of imported documents is refused rather than silently dropping unknown
records. Source-less scenes have a minimal binary output path; Altium acceptance
of newly created containers has not been independently checked.

Binary supported families: Board6, Nets6, Components6, Polygons6, Tracks6, Arcs6,
Pads6, Vias6, Fills6, Texts6, Regions6, ShapeBasedRegions6 and WideStrings6. Common
primitive polygon reference was corrected from byte offset 9 to offset 5.

Original CFB is used as the container; unrelated streams are preserved. Known
records are patched using their original subrecord tails. Region extensions are
retained after their decoded geometry; changing unmodelled extension semantics is
not supported. Existing property order/duplicate unknown fields are retained where
possible. New records are appended and represented tables/references are remapped.

The writer refuses dependency-changing cases it cannot safely handle. In particular:

* moving/deleting a component with an unmodelled component-body stream;
* deletion/reindexing with dependent UID, primitive metadata, class, dimension,
  connection, smart-union or board-region sections requiring unsupported remaps;
* modified files containing unsupported polygon definitions or unresolved source
  component/net references;
* some modern V7/V8 layer overrides, full-stack pad resizing, cavity/outline region
  edits, native rule edits/creation and unsupported short-string characters.

These guards mean **native Save will not succeed for every edit of every Altium
board**. JSON remains the fallback. Arbitrary unknown dependencies are not proven
lossless merely because bytes were preserved. A successful own-reader round-trip
is necessary for this writer but not sufficient to establish external compatibility.

All output is serialized and re-imported in memory before touching the target.
Modelled counts, table attributes and geometry are compared (up to 3 nm for integer
native quantization). Existing destinations receive a non-overwriting backup and
an atomic replace; race checking is best-effort, not cross-process file locking.
No automatic backup is substituted for verification in Altium.

## Tests and integration validation

Core Rust tests cover polygons, thermal/direct contact, cuts, priority, islands,
shared endpoints, flip, routing checkpoints/splits/net association and controller
transactions. IO tests cover source-preserving edits, unknown streams/tails,
quantization, native re-import, backup/atomic failure, deleted polygon migration,
Unicode properties and selected refusal paths.

The supplied archive was verified against SHA256SUMS.txt; its 22 Python assembler
tests pass. Those tests exercise package handling, not Rust algorithms.
The authoring-kit report describes the pre-integration state; [VALIDATION.md](VALIDATION.md)
records checks executed after integration.
`verify_pcb_update.py` uses the committed Cargo.lock with --locked, generates a
machine-readable local report, and never reports a
missing compiler or refused native edit as a passed test.

## References

* Altium official PCB editor shortcuts and polygon documentation.
* `tscircuit/altiumts` source at `d9faf03f0ba0165066c82d0352c0d12adcc60409` for observed
  native stream layouts and supported serialization fields; no runtime dependency.
* `geo` 0.31.0 docs for BooleanOps, Buffer, Area and Intersects.
* `cfb` public API docs and the project's existing pinned dependency.
* TI TIPD167 and TIDA-00691 are reference candidates only. No TI archives or real
  `.PcbDoc` boards were downloaded/tested in this environment. Included fixtures
  are synthetic and must not be presented as TI designs.


### Padstack and mask continuation (2026-10-21)

The prior display/properties limitations are superseded for the implemented
Simple/TMB/Full pad/via stack, Slot/Square drills, per-layer offsets/radii,
portable 3D preview with software depth testing and bounded mask-rule editing. See PROPERTIES.md for
observed native states, supported binary fields, 188 checks and explicit gaps.
Native mask-only real-board copy interoperability was observed in AD 24.4.1;
copper repour requiring unsupported reference remaps still refuses native Save.
