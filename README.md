# PCBCraft

PCBCraft is an open-source, Rust-based EDA platform inspired by Altium Designer's interface and workflows, featuring schematic design, PCB layout, a custom engine.

Самостоятельная EDA на Rust с интерфейсом, исследуемым по Altium Designer.
Текущий этап: импорт проектов и схем Altium, базовое редактирование схем и PCB canvas.

## Запуск

Rust 1.90.0, Windows (обычно MSVC + Windows SDK) или Linux.

```powershell
cargo run -p eda-app
cargo run -p eda-app -- fixtures/controller.pcbcraft.json
cargo run -p eda-app -- fixtures/schematic/minimal.PrjPcb
cargo run -p eda-app -- fixtures/schematic/editing-demo.pcbcraft.json
```

В текущем рабочем каталоге также подготовлен локальный GNU toolchain
в игнорируемом `.tools`. Скрипт использует rust-lld через небольшой адаптер
и локальный LLVM dlltool для Windows import libraries:

```powershell
.\scripts\dev.ps1 run
.\scripts\dev.ps1 build -Release
.\scripts\dev.ps1 test
```

Сборка: `target/debug/pcbcraft.exe` (или `target/release/pcbcraft.exe`).
Обычная dev-сборка использует оптимизацию уровня 2 для интерактивной работы
с платами, сохраняя отладочные символы и проверки. Методика измерений:
[производительность PCB](docs/pcb-update/PERFORMANCE.md).
`.tools` не входит в репозиторий. На другом компьютере установить Rust
обычным способом; основной Cargo workspace не зависит от локального linker.

### Linux

Для Ubuntu/Debian подготовить зависимости оконной системы:

```bash
sudo apt-get install build-essential pkg-config libxkbcommon-dev libwayland-dev libx11-dev libxi-dev libxcursor-dev libxrandr-dev
cargo run --locked -p eda-app
```

Поддержка X11 и Wayland включена. Диалоги файлов используют desktop portal;
в окружении рабочего стола должен быть установлен его backend.
Windows-скрипт `scripts/dev.ps1` для Linux не нужен. CI настроен для обеих ОС,
но фактический локальный запуск пока проверяется только на Windows.

### AppImage на Linux x86_64

В этом рабочем каталоге Rust 1.90.0, rustfmt и clippy установлены локально
в `.tools`. Linux-обёртка автоматически выбирает этот toolchain:

```bash
bash scripts/dev.sh build --locked --release -p eda-app
bash scripts/dev.sh fmt --all -- --check
bash scripts/dev.sh clippy --locked --workspace --all-targets -- -D warnings
bash scripts/dev.sh test --locked --workspace
python3 scripts/build-appimage.py
```

Для повторной упаковки готового release-бинарника:
`python3 scripts/build-appimage.py --no-build`.
Результат и SHA256 находятся в `dist/PCBCraft-0.1.0-x86_64.AppImage`
и `dist/PCBCraft-0.1.0-x86_64.AppImage.sha256`.

Упаковке нужны Python 3.11+, `appimagetool`, `binutils`, `ldconfig`
и оконные библиотеки, включая `libxkbcommon-x11` и `libX11-xcb`.
Путь к appimagetool можно передать через `--appimagetool` или `APPIMAGETOOL`.
Инструмент скачивает AppImage runtime при упаковке; для работы без сети
передайте заранее скачанный runtime через `--runtime-file`.
Системный FUSE для упаковки не требуется.

AppImage содержит приложение, встроенные шрифты и библиотеки X11/Wayland.
glibc, графический драйвер и Vulkan/OpenGL loader используются из системы.
Сборка на современной Arch Linux не гарантирует совместимость со старыми
дистрибутивами. Для такого выпуска нужно собирать на самом старом
поддерживаемом дистрибутиве и проверять результат на целевых системах.
Диалоги открытия/сохранения требуют работающего desktop portal.

Запуск без FUSE:

```bash
./dist/PCBCraft-0.1.0-x86_64.AppImage --appimage-extract-and-run
```

На новой машине установите Rust 1.90.0 через rustup согласно
`rust-toolchain.toml`. Без локального `.tools` обёртка использует Cargo из PATH.

## Возможности первой итерации

- Меню и toolbar, вкладки, Projects, Properties, Messages, статус и слои.
- Изменение размеров и видимости панелей; сохранение раскладки egui.
- PCB/SCH документы, Home, пример платы, поиск по открытым документам.
- Zoom колесом, pan средней/правой кнопкой, fit двойным щелчком или Home.
- Properties: Board/Pad, поиск, сворачиваемые секции и таблица привязок.
- Выбор прямоугольных pads; правки Properties применяются при завершении ввода;
  Ctrl+Z / Ctrl+Y.
- Фильтр выбора pads, оси и привязки к вершинам tracks, центрам/вершинам/краям pads.
- Open/Save/Save As/Save All собственного JSON v1, recent documents.
- Защита несохранённых изменений при закрытии вкладки или окна.
- Preferences: единицы, сетка; G — сетка, Q — единицы.

Открываются `.PrjPcb` и `.SchDoc` (CFB Binary / ASCII Version 5.0): дерево
файлов проекта, символы, выводы, провода, текст, параметры, встроенные изображения,
выбор компонентов/объектов и Properties. Файлы схем загружаются по выбору из дерева.
Подробные границы совместимости: [импорт Altium](docs/altium-import.md).
Базовое редактирование схем: размещение R/C, проводов, меток, GND, текста,
линий и junctions; перемещение, поворот, копирование, удаление, Properties,
Undo/Redo и сохранение в собственный JSON. Управление и границы:
[редактор схем](docs/schematic-editing.md).
Панель Components: импорт локальных `.SchLib` Binary 5.0 / `.IntLib`, поиск,
предпросмотр и размещение библиотечных символов. Откройте библиотеку через
Ctrl+O или меню Components → Install Library…. Space — поворот, Esc — завершение.
[Поддержка и ограничения библиотек](docs/interface/components-libraries.md).
Импорт PCB и экспериментальное редактирование `.PcbDoc`: компоненты, pads/vias/tracks,
ручная трассировка, полигоны с перезаливкой и native Save для поддержанного подмножества
ASCII/PCB6. Пример: `cargo run --locked -p eda-app -- fixtures/pcb-update/editor-v2.PcbDoc`.
Управление, ограничения и результаты проверок: [PCB](docs/pcb-update/IMPLEMENTATION.md),
[валидация интеграции](docs/pcb-update/VALIDATION.md).
Native-запись схем, полный docking, обход препятствий/shove, ERC/DRC, редактор библиотек
и выпуск производственных файлов запланированы. Неактивные инженерные меню
не выполняют операции. Altium не нужен для запуска PCBCraft.

Save / Save As импортированной схемы создаёт собственный `.pcbcraft.json`,
сохраняя исходный SchDoc целиком; запись JSON поверх native-файла запрещена.
Для проверки импорта без GUI: `cargo run -p eda-io --example altium_inspect -- path/to/file.SchDoc`.

## Проверки и план

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

С локальным toolchain: `scripts/dev.ps1 fmt --all -- --check`,
`scripts/dev.ps1 clippy --all-targets -- -D warnings`.

[План](PLAN.md), [архитектура](docs/architecture.md),
[наблюдения интерфейса](docs/interface/altium-observations.md),
[реестр функций](docs/features.json), [проверки](docs/verification.md).

Для локальной проверки исходников Linux из подготовленного Windows toolchain:
`scripts/dev.ps1 check --locked --all-targets --target x86_64-unknown-linux-gnu`.
Предварительно добавить target через rustup. Это проверка компиляции без
линковки и запуска Linux-приложения.

Интерфейс использует установленный Segoe UI на Windows и встроенный Selawik
на Linux. Пути к своим шрифтам можно задать через `PCBCRAFT_UI_FONT` и
`PCBCRAFT_UI_FONT_SEMIBOLD`. Лицензия и происхождение — [assets/fonts](assets/fonts/README.md).
Шрифты схем отделены от интерфейса: Times/Arial/Courier/GOST читаются из
установленных шрифтов Windows либо каталога `PCBCRAFT_SCHEMATIC_FONT_DIR`.
На Linux без этих файлов применяется свободный fallback; метрики могут отличаться.

История Undo/Redo ограничена 128 шагами и бюджетом оценки снимков 64 МиБ на документ;
для больших схем шагов может быть меньше. [Проверка памяти и ограничения](docs/memory.md).
