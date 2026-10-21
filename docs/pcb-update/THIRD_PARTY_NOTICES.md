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
