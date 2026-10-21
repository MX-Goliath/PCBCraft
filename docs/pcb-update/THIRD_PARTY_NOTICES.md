# Source origin and references

The modified existing files originate from the user's `PCBCraft` repository at commit `bd761734cb5b2fae5461bb5bbec800efd5814bf9`. Existing project license notices remain applicable. This kit is intended to be assembled with that repository, including its original LICENSE and dependency notices.

New modules and scripts in this update are offered under the MIT license, consistent with the repository's root license. A copy is in LICENSE-NEW-CODE.txt.

Public reference used for binary format facts: `tscircuit/altiumts`, MIT-licensed project, observed commit `d9faf03f0ba0165066c82d0352c0d12adcc60409`:
https://github.com/tscircuit/altiumts

The TypeScript library itself is not bundled or executed. The Rust implementation was written separately using observed field/stream layouts. No KiCad engine or parser source is included.

Official Altium documentation was used as an interaction reference:
https://www.altium.com/documentation/altium-designer/shortcut-keys/pcb-editors

Texas Instruments landing pages were located as reference candidates:
https://www.ti.com/tool/TIPD167
https://www.ti.com/tool/TIDA-00691

No Altium binaries, proprietary icons, proprietary fonts, or TI design archives are redistributed. Source URLs and observations do not imply endorsement, tested compatibility, or permission to redistribute third-party reference assets.

## v2 references and dependency addition

Observed serialization layouts:
https://github.com/tscircuit/altiumts/blob/d9faf03f0ba0165066c82d0352c0d12adcc60409/lib/serialization/serialize-altium-pcb-primitives.ts
https://github.com/tscircuit/altiumts/blob/d9faf03f0ba0165066c82d0352c0d12adcc60409/lib/serialization/serialize-altium-pcb-doc-to-binary.ts
https://github.com/tscircuit/altiumts/blob/d9faf03f0ba0165066c82d0352c0d12adcc60409/lib/serialization/serialize-altium-pcb-region.ts

New dependency, resolved on the user's machine rather than bundled:
https://docs.rs/geo/0.31.0/geo/
https://docs.rs/geo/0.31.0/geo/algorithm/buffer/trait.Buffer.html

Official polygon reference:
https://www.altium.com/documentation/altium-designer/pcb/defining-polygons-copper-regions

These references do not imply that Altium opened the generated files. No third-party
binaries, fonts or reference-board archives are included in this source update.


## PCB display format observations — October 21, 2026

Font-kind/charset/style/barcode field offsets were cross-checked with the original
AltiumSharp reader; that reader is not bundled or executed by PCBCraft:
https://github.com/issus/AltiumSharp/blob/master/src/OriginalCircuit.Altium/Serialization/Readers/PcbLibReader.cs

Native rounded-pad percentage semantics were checked against official documentation:
https://www.altium.com/documentation/altium-designer/pcb/pads-vias

The current user's open Altium 24.4.1 instance supplied display observations. No
proprietary font files or Altium executable code were copied into this project.


## Padstack and mask format observations — October 21, 2026

Pad extra-block arrays, hole shape/slot/offset and via diameter offsets were
cross-checked with the same original AltiumSharp reader referenced above. Native
AD 24.4.1 copy saves additionally verified Full Stack records (layer ID followed
by shape byte, X/Y sizes and radius percentage) and the top solder tenting flag.
These are format observations; no reader source or Altium executable code is
bundled. The 3D mesh generation and bounded mask scope evaluator are PCBCraft
implementations using the existing geo/earcutr/egui dependencies.
