# UI fonts

Selawik Regular and Semibold, version 1.01, from the Microsoft Selawik project:
https://github.com/microsoft/Selawik

Release archive: https://github.com/microsoft/Selawik/releases/download/1.01/Selawik_Release.zip

Original font files `selawk.ttf` and `selawksb.ttf` are included without changes
as `Selawik-Regular.ttf` and `Selawik-Semibold.ttf`. License: SIL Open Font
License 1.1; full copyright, reserved font names and terms in OFL-Selawik.txt.

Windows reads the locally installed Segoe UI/Segoe UI Semibold when available;
those proprietary fonts are not redistributed. Linux uses bundled Selawik.
Optional environment variables PCBCRAFT_UI_FONT and PCBCRAFT_UI_FONT_SEMIBOLD
select user-provided font files. Missing/unreadable files fall back to Selawik.
This keeps layout independent of the Linux desktop's default fonts; pixel
equivalence of the two font families is not asserted.

# Schematic fonts

`liberation/` contains unchanged Liberation Sans Regular, Bold, Italic and
BoldItalic, internal version 1.07.4, distributed in pdfjs-dist 5.6.205's
`standard_fonts` (https://github.com/mozilla/pdf.js/tree/master/standard_fonts).
The full SIL OFL 1.1 notice is in `liberation/LICENSE_LIBERATION`.
Cyrillic glyph coverage was checked. These fonts provide a portable fallback
for schematic text when Arial is unavailable.

Document families are resolved through fontdb by actual family name, using
the optional `PCBCRAFT_SCHEMATIC_FONT_DIR`, then installed system fonts.
`Altium GOST Type A` and `GOST Type AU` are separate families. The former is
absent on the observed Windows machine; Arial matches the reference fallback.
Proprietary Altium/Windows fonts are not bundled. Exact Linux rasterization
still requires a real Linux GUI comparison.
