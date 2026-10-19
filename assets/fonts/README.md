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
