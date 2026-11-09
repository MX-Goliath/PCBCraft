# PCBCraft

PCBCraft is an open-source EDA platform written in Rust, with its own engine for
schematic design and PCB layout. Its interface and workflows are studied against
Altium Designer. The current stage covers Altium project and schematic import,
basic schematic editing and a PCB canvas.

Contributor: [MX-Goliath](https://github.com/MX-Goliath).

Desktop updates use GitHub Releases. See [release packages and update channel](docs/updates.md)
for Windows installer/portable, Linux DEB/AppImage and macOS Intel/Apple Silicon.

## Running

Requires Rust 1.90.0 and Windows (usually MSVC + Windows SDK) or Linux.

```powershell
cargo run -p eda-app
cargo run -p eda-app -- fixtures/controller.pcbcraft.json
cargo run -p eda-app -- fixtures/schematic/minimal.PrjPcb
cargo run -p eda-app -- fixtures/schematic/editing-demo.pcbcraft.json
```

A local GNU toolchain is also prepared in the ignored `.tools` directory in this
working copy. The script uses rust-lld through a small adapter and a local LLVM
dlltool for Windows import libraries:

```powershell
.\scripts\dev.ps1 run
.\scripts\dev.ps1 build -Release
.\scripts\dev.ps1 test
```

Build output: `target/debug/pcbcraft.exe` (or `target/release/pcbcraft.exe`).
The normal development build uses optimization level 2 for interactive board
editing while retaining debug symbols and checks. Measurement procedure:
[PCB performance](docs/pcb-update/PERFORMANCE.md).
`.tools` is not part of the repository. On another computer, install Rust normally;
the main Cargo workspace does not depend on the local linker.

### Windows x64 portable package

```powershell
python scripts/build-windows-portable.py
```

Add `--no-build` to package an existing release build. The ZIP is written to
`dist/PCBCraft-0.0.2-windows-x64-portable.zip`, with a SHA256 checksum alongside it.
Packaging requires Python 3.11+ and the Visual Studio redistributable CRT; its
path can be supplied with `--crt-dir`. Users do not need to install the application:
extract the entire folder and run `pcbcraft.exe`. If `portable.flag` is present
next to the executable, settings are saved in `Data/app.ron` in the same folder.

### Linux

Install the window-system dependencies on Ubuntu/Debian:

```bash
sudo apt-get install build-essential pkg-config libssl-dev libxkbcommon-dev libwayland-dev libx11-dev libxi-dev libxcursor-dev libxrandr-dev
cargo run --locked -p eda-app
```

X11 and Wayland support are enabled. File dialogs use the desktop portal;
its backend must be installed in the desktop environment.
The Windows script `scripts/dev.ps1` is not needed on Linux. CI is configured for
both operating systems, but local runtime verification currently covers Windows only.

### Linux x86_64 AppImage

In this working copy, Rust 1.90.0, rustfmt and clippy are installed locally in
`.tools`. The Linux wrapper automatically selects this toolchain:

```bash
bash scripts/dev.sh build --locked --release -p eda-app
bash scripts/dev.sh fmt --all -- --check
bash scripts/dev.sh clippy --locked --workspace --all-targets -- -D warnings
bash scripts/dev.sh test --locked --workspace
python3 scripts/build-appimage.py
```

To repackage an existing release binary:
`python3 scripts/build-appimage.py --no-build`.
The output and SHA256 checksum are written to `dist/PCBCraft-0.0.2-x86_64.AppImage`
and `dist/PCBCraft-0.0.2-x86_64.AppImage.sha256`.

Packaging requires Python 3.11+, `appimagetool`, `binutils`, `ldconfig` and window
libraries, including `libxkbcommon-x11` and `libX11-xcb`.
Pass the appimagetool path with `--appimagetool` or `APPIMAGETOOL`.
The tool downloads the AppImage runtime during packaging; for offline use,
provide a previously downloaded runtime with `--runtime-file`.
System FUSE is not required for packaging.

The AppImage includes the application, embedded fonts and X11/Wayland libraries.
It uses the system glibc, graphics driver and Vulkan/OpenGL loader.
Building on a recent Arch Linux does not guarantee compatibility with older
distributions. For such a release, build on the oldest supported distribution
and verify the result on target systems.
Open/save dialogs require a working desktop portal.

Run without FUSE:

```bash
./dist/PCBCraft-0.0.2-x86_64.AppImage --appimage-extract-and-run
```

On a new machine, install Rust 1.90.0 through rustup as specified in
`rust-toolchain.toml`. Without a local `.tools`, the wrapper uses Cargo from PATH.

## First-iteration features

- Menus and toolbar, tabs, Projects, Properties, Messages, status and layers.
- Panel resizing and visibility; persistent egui layout.
- PCB/SCH documents, Home, sample board and search across open documents.
- Wheel zoom, middle/right-button pan, fit by double-click or Home.
- Properties: Board/Pad, search, collapsible sections and snapping table.
- Rectangular pad selection; Properties edits apply when input is completed;
  Ctrl+Z / Ctrl+Y.
- Pad selection filter, axes and snapping to track vertices and pad centers/vertices/edges.
- Open/Save/Save As/Save All for the application's JSON v1 format, recent documents.
- Unsaved-change protection when closing a tab or window.
- Preferences: units and grid; G toggles the grid, Q switches units.

Opens `.PrjPcb` and `.SchDoc` (CFB Binary / ASCII Version 5.0): project file tree,
symbols, pins, wires, text, parameters, embedded images, component/object selection
and Properties. Schematic files load when selected in the tree.
Detailed compatibility boundaries: [Altium import](docs/altium-import.md).
Basic schematic editing includes placing R/C components, wires, labels, GND, text,
lines and junctions; moving, rotating, copying, deleting, Properties, Undo/Redo and
saving in the application's JSON format. Controls and limitations:
[schematic editor](docs/schematic-editing.md).
The Components panel imports local `.SchLib` Binary 5.0 / `.IntLib` files and
supports searching, previewing and placing library symbols. Open a library with
Ctrl+O or Components → Install Library…. Space rotates; Esc finishes placement.
[Library support and limitations](docs/interface/components-libraries.md).
PCB import and experimental `.PcbDoc` editing include components, pads/vias/tracks,
manual routing, polygons with repouring and native Save for the supported
ASCII/PCB6 subset. Example: `cargo run --locked -p eda-app -- fixtures/pcb-update/editor-v2.PcbDoc`.
Controls, limitations and verification results: [PCB](docs/pcb-update/IMPLEMENTATION.md),
[integration validation](docs/pcb-update/VALIDATION.md).
Native schematic writing, full docking, obstacle avoidance/shove, ERC/DRC, a library
editor and manufacturing outputs are planned. Disabled engineering menu entries
do not perform operations. Altium is not required to run PCBCraft.

Save / Save As for an imported schematic creates a `.pcbcraft.json` file,
preserving the complete original SchDoc; writing JSON over a native file is prohibited.
To check import without the GUI: `cargo run -p eda-io --example altium_inspect -- path/to/file.SchDoc`.

## Checks and plan

Private projects, paths and original reports must stay out of Git.
Storage rules and pre-commit checks: [repository privacy](docs/privacy.md).

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

With the local toolchain: `scripts/dev.ps1 fmt --all -- --check`,
`scripts/dev.ps1 clippy --all-targets -- -D warnings`.

[Plan](PLAN.md), [architecture](docs/architecture.md),
[interface observations](docs/interface/altium-observations.md),
[feature registry](docs/features.json), [verification](docs/verification.md).

To check Linux sources locally from the prepared Windows toolchain:
`scripts/dev.ps1 check --locked --all-targets --target x86_64-unknown-linux-gnu`.
Add the target through rustup first. This checks compilation without linking or
running the Linux application.

The interface uses installed Segoe UI on Windows and embedded Selawik on Linux.
Custom font paths can be set with `PCBCRAFT_UI_FONT` and
`PCBCRAFT_UI_FONT_SEMIBOLD`. License and provenance: [assets/fonts](assets/fonts/README.md).
Schematic fonts are separate from interface fonts: Times/Arial/Courier/GOST are
loaded from installed Windows fonts or `PCBCRAFT_SCHEMATIC_FONT_DIR`.
Linux uses a free fallback when these files are absent; metrics may differ.

Undo/Redo history is limited to 128 steps and an estimated snapshot budget of
64 MiB per document; large schematics may retain fewer steps.
[Memory verification and limitations](docs/memory.md).
