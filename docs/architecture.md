# Architecture

`eda-app → eda-render / eda-io / eda-core → eda-geom`

- `eda-geom`: document coordinates in integer nanometers.
- `eda-core`: Document, Pad, Track, Layer, Units, Schematic, validation and history.
- `eda-io`: the application's JSON v1 format, size limits, data validation,
  writing through a temporary file in the same directory and atomic persist.
- `eda-render`: camera, read-only canvas, hit testing and view state.
- `eda-app`: eframe/wgpu, shell, shared command handlers for
  menus/toolbars/shortcuts, dialogs, Properties and settings.

The core does not depend on egui, Win32 or Altium. The UI submits changes through
History::commit. History currently stores snapshots of small documents; replace
these with typed changes later, preserving atomicity and revision semantics.

eframe 0.32.3 and egui 0.32.3 are pinned together. A single event loop and eframe's
wgpu backend are used; no separate GPU device is created.
The canvas currently uses egui Painter; the next stage will add custom drawing
batches through the same backend. egui_dock will be added when docking is
implemented; current panels support resizing/hiding.
Messages and Properties share one right-hand area with bottom tabs;
this is a custom layout using egui panels, not yet full docking.

The visual target is an exact copy of Altium 24.4.1, as clarified by the user.
An independent model and format do not justify changing the familiar interface
layout. Current differences: docs/interface/visual-parity.md.

Tabs store independent cameras, selections and histories. The saved snapshot
determines dirty state, so undoing back to the saved content removes `*`.
Settings and panel geometry are persisted by eframe; documents and unsaved
sessions are not yet restored automatically.

`eda-io::altium` reads INI projects and CFB/ASCII schematics. The Schematic model
stores a typed scene, components, original records and an archive of the source.
The parser does not depend on GUI/Win32. The view stores independent selection
filters for components, wires, text and graphics. The renderer reads the document;
selection and zoom changes do not alter imported data.
CFB, encodings and zlib are handled by portable Rust libraries.
Images are decoded with limits; textures are cached by content.
Concave outlines are tessellated with earcutr solely for display.

Basic schematic commands live in `eda-core::schematic_edit`; Request checks
the document/revision, and preview/commit use the same validated implementation.
The GUI stores the tool and Properties draft in schematic_editor.rs;
eda-render returns input in document coordinates. The original native archive
is unchanged; `source_modified` distinguishes it from the current scene.
Contract and limitations: docs/schematic-editing.md and ADR 0003.

Connectivity, DRC, routing and a schematic compiler are not yet implemented.
Pin types and metadata are imported, but engineering semantics are not checked.
Pads/Tracks in the example are independent geometric primitives without nets/footprints.

Linux and Windows are equal target platforms. Paths are stored in PathBuf;
the file layer uses standard Rust APIs and tempfile; platform dialogs are
provided by rfd. X11/Wayland are explicitly enabled. Local GNU linker setup on
Windows is confined to scripts/dev.ps1 and the ignored .tools directory.
The shared title bar and window commands use the egui viewport API;
Win32 has not been added to UI code. Windows system fonts are read optionally;
Linux and fallback rendering use Selawik embedded in the binary under the OFL.
Properties is separated into properties.rs, chrome into chrome.rs, and icon
drawing/fonts/shared theme into theme.rs. Geometric snapping remains View state
in eda-render and does not change document coordinates.
Linux and Windows checks are configured in .github/workflows/check.yml;
visual runtime verification is required separately on each operating system.

Primary API sources: [eframe 0.32.3](https://docs.rs/eframe/0.32.3/eframe/),
[egui_dock 0.17.0](https://docs.rs/egui_dock/0.17.0/egui_dock/).
