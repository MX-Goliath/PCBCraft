# Валидация интеграции PCB v2 — 21.10.2026

Интегрирован пользовательский пакет `PCBCraft-PCB-integration-v2-2026-10-21.zip`
на исходную базу `bd761734cb5b2fae5461bb5bbec800efd5814bf9`.
Все записи `SHA256SUMS.txt` совпали; исходные Git blob-хеши изменяемых файлов
совпали с manifest пакета. Перенесены overlay, Properties hook, синтетические
fixtures, документация, проверяющий скрипт и уведомления о происхождении кода.

## Исправления при интеграции

- Устранён borrow-after-move в `pcb_tools::edit_component`: mutable-ссылка
  выбирается до перемещения нового значения компонента.
- Обновлён тестовый конструктор приложения для нового `pcb_clipboard`.
- Применён rustfmt, устранены замечания Clippy без отключения проверок.
- Cargo.lock дополнен `geo 0.31.0` и его зависимостями; существующие версии
  зависимостей не обновлялись. `verify_pcb_update.py` использует `cargo fetch
  --locked` вместо перегенерации lockfile при каждом запуске.
- Отчёт `pcb_inspect` отражает экспериментальный writer и считает редактируемые
  определения полигонов отдельно от исходных записей.

## Выполненные автоматические проверки

Linux x86_64, Debian 13, Rust 1.90.0, четыре параллельных задания:

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo build --locked --workspace --all-targets
python3 scripts/verify_pcb_update.py --report-dir /tmp/pcb-validation-unique
```

Все проверки прошли. Выполнено 109 Rust-тестов: приложение 24, core 34,
geom 1, IO 44, render 6; ошибок, пропусков и ignored нет. Пустые doc-test
группы не включены в это число. Все 22 Python-теста сборщика из исходного
архива также прошли; они проверяют упаковку, а не функциональность Rust.

Проверки включают геометрию и транзакции, routing preview/backtrack/via/splits,
полигоны/вырезы/thermal/priority/islands, JSON-миграцию и регрессии старых схем.
IO-тесты проверяют синтетические binary/ASCII, сохранение неизвестных потоков,
отказ для неподдержанных зависимостей, резервное копирование и атомарную замену.

`pcb_roundtrip --exercise` для `fixtures/pcb-update/editor-v2.PcbDoc` изменил
компонент, Track, Pad, Via, добавил трек и изменил clearance полигона с repour.
Сохранено 16704 байта; собственный читатель восстановил 9 примитивов,
1 компонент, 2 цепи, 1 полигон. Входной fixture не изменён.

## GUI на Linux

Реальное окно wgpu/OpenGL запущено в Xvfb 1440×900 с программным Mesa.
Для контейнерного X11 отключено MIT-SHM; проверка выполнялась на отдельном
дисплее и копии fixture вне репозитория. Клавиатурный фокус задан через
`xdotool windowfocus`.

Проверены отображение платы, площадок, via, текста, дорожки и выреза;
выбор R1 и Component Properties; Comment `1k` → `2k2`, Apply, перезаливка;
Ctrl+S в `.PcbDoc`. Первая `.bak` побайтово совпала с исходным fixture.
Проверены Ctrl+W, маршрут от J1.1 (25,10 мм) к (30,15 мм), Esc,
Undo (новая дорожка исчезает), Redo и Save. После повторного импорта
сохранились Comment `2k2` и две дорожки; после перезапуска они отображаются.

## Границы результата

Проверка выполнялась собственным читателем PCBCraft и синтетическими файлами.
Altium Designer, реальные пользовательские платы и Windows в этой среде
не проверялись. Совместимость native-файлов с Altium и точная визуальная
эквивалентность не подтверждены. DRC, obstacle avoidance, push-and-shove,
ECO и выпуск производственных файлов не реализованы этим пакетом.
Поддержанный поднабор writer и блокирующие ограничения описаны в
[IMPLEMENTATION.md](IMPLEMENTATION.md). Полный этап PCB из PLAN.md не завершён.


## Windows: открытие ReferenceB с NPTH без меди — 21.10.2026

Воспроизведено `Invalid PCB Pad geometry` при открытии из проекта
`ReferenceB.PrjPcb` документа `ReferenceBBoard.PcbDoc`. В Pads6 запись 121
(индексация с нуля) содержит Multi-Layer Pad `0`, PLATED=false, нулевые размеры
top/middle/bottom land и положительный drill 177165 native units
(449999 нм, номинально 0,45 мм). Ошибка была в ограничении модели, а не в
разрешении пути проекта или чтении размеров.

Модель допускает этот ограниченный случай: неметаллизированное сквозное
отверстие без меди. Нулевые SMD-площадки, площадки без отверстия, отрицательные
размеры и нулевые металлизированные площадки по-прежнему отклоняются.
Размеры меди остаются нулевыми; объект не удаляется и не заменяется via.
Bounds и picking учитывают диаметр сверления. Renderer не создаёт вырожденный
медный полигон и показывает контур отверстия; оно остаётся видимым при
переключении слоёв. Полигонная заливка обходит отверстие на обоих слоях,
не используя его как электрическую площадку даже при совпадении net.

Проверено:

- 148 workspace tests, fmt --check, Clippy --all-targets -D warnings,
  Windows debug build и Linux GNU all-targets cargo check.
- Независимо сформированный бинарный CFB с такой площадкой: импорт,
  JSON Save/load, перемещение, native serialize/re-import и сохранение opaque
  хвоста записи и оригинального source archive. Реальная плата не сохранялась.
- Регрессии валидации/границ/picking и обход отверстия заливкой на Top/Bottom.
- CLI реальной платы: 15389 примитивов, 69 компонентов, 20 цепей, 3 полигона;
  ни один Pad не пропущен (123 записи).
- Реальное окно Windows: открытие проекта, двойной щелчок по PcbDoc в Projects,
  появление платы и вкладки без ошибки импорта. Старое окно с несохранёнными
  правками оставлено открытым; исходный PcbDoc не изменён.

Это проверка открытия и указанного типа отверстия. Точность всех слоёв,
текста, padstack/slots, DRC и совместимость изменённой реальной платы с Altium
здесь не подтверждались. Linux GUI и новые Linux-артефакты не запускались.


## Windows: визуальная сверка PCB — 21.10.2026

Подробности и незавершённое: [VISUAL_PARITY.md](VISUAL_PARITY.md). Полное
совпадение с Altium не заявляется. Проверки этой итерации: 159 workspace tests,
fmt --check, строгий Clippy, Windows debug build и реальное окно платы; Linux
GNU all-targets check без Linux GUI/выпуска артефакта. Модель теперь содержит
15513 записей, включая 124 записи 2D body projections. Оба представления каждого
корпуса/Region сохранены для native IO, но не накладываются на экране повторно.

Отдельные регрессии проверяют native layer overrides/fallback/tails, сохранность
неизменённых 3D body streams, отказ от записи изменённой проекции без 3D transforms,
скрытые component strings, native radius percentages, font flags, glyph geometry
на трёх DPI, hatch clipping и clockwise arc seams без огромных GPU-треугольников.
Реальная плата в native формате не перезаписывалась, совместимость изменённого
реального PcbDoc с Altium пока не подтверждена.


## Windows: PCB Properties — 21.10.2026

[PROPERTIES.md](PROPERTIES.md) разделяет native observations, реализацию,
проверки и остающиеся ограничения. Контекстные формы, origin-relative fields,
Comment, supported snap switches и работа с History. 169 workspace tests,
fmt --check, strict Clippy all-targets, Windows build, Linux GNU all-targets
check. Реальное окно: Board/XP1/Track/Via, диаметр 0.7 → 0.8 mm и Undo/Redo/Undo.
Исходная плата не записывалась. Linux GUI не запускался. Полное совпадение
Properties/3D padstack/правил/всех состояний не заявляется.


## Windows padstack / 3D / mask continuation — 2026-10-21

- 188 workspace tests: 38 app, 70 core, 1 geom, 56 IO, 23 render. Expanded table
  and 316 px layout, mask-only pour preservation, unchanged-mode geometry,
  slots/offsets, per-layer selection, Flip and bounded rule priorities/scopes,
  native stack/Rules6 preservation and refusal of unsupported lossy writes.
  Meshes include finite triangles, real cutouts, blind-via span and fast drag.
- Commands: `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`,
  Windows `cargo build -p eda-app`, Linux
  `cargo check --workspace --all-targets --target x86_64-unknown-linux-gnu`.
  The repository dev.ps1 supplies the local GNU toolchain/linker.
- Native research modified/saved only `.tools/pcb-padstack-reference.PcbDoc`;
  its untented snapshot is `.tools/pcb-padstack-untented.PcbDoc`. User source
  `ReferenceBBoard.PcbDoc` was never written. Both original user application windows
  and the old schematic research state were preserved.
- `.tools/pcb-padstack-roundtrip.PcbDoc`: generated by PCBCraft from the real
  source with Pad 2 Top Tented and an existing SolderMaskExpansion rule changed
  to 0.15 mm. AD 24.4.1 opened it without an error dialog. Visual inspection of
  Pad 2 confirmed top tenting on, bottom off; rule fields were reimport-checked,
  not exhaustively inspected in Altium. This is a bounded interoperability check.
- A copper geometry edit + repour on the same real board was correctly refused
  by native save: UniqueIDPrimitiveInformation remapping is not implemented.
  No destination bytes were returned or source changed. This limitation remains.
- Real Windows PCBCraft table/preview modes, rotation, Top Tented update and Undo/Redo/Undo were inspected; no edited QA board
  was saved to the user path. Linux compilation only; no Linux GUI or artifact.

The earlier Properties/display validation sections describe earlier milestones.
Current gaps and superseding padstack implementation are in PROPERTIES.md.
