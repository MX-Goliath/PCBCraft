# PCBCraft

Самостоятельная EDA на Rust с интерфейсом, исследуемым по Altium Designer.
Первая итерация: главное окно, собственные документы и небольшой PCB canvas.

## Запуск

Rust 1.90.0, Windows (обычно MSVC + Windows SDK) или Linux.

```powershell
cargo run -p eda-app
cargo run -p eda-app -- fixtures/controller.pcbcraft.json
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

Схема пока отображается как пустой лист; её электрического редактора нет.
Полный docking, native Altium IO, routing, ERC/DRC, библиотеки и выпуск
производственных файлов запланированы. Неактивные инженерные меню
не выполняют операции. Altium не нужен для запуска PCBCraft.

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
