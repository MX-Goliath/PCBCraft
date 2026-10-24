# План реализации PCBCraft

### Текущая отладка PCB — ReferenceA, 23.10.2026

По указанию пользователя единственный текущий реальный эталон —
`private-projects/ReferenceA/ReferenceA.PrjPcb`.
Сверять с установленным и запущенным Altium Designer Enterprise 24.4.1.
Исходные файлы не изменять; для сохранения результатов правок использовать копии.

- [x] Подготовлена Windows MSVC-среда Rust 1.90.0, проверены сборка и запуск.
- [x] Воспроизведён и исправлен отказ импорта региона `V7_LAYER=CONNECT`.
- [x] Исправлено скрытие слоёв при native `SINGLELAYERMODESTATE=3` (Off).
- [x] Подключены native range/layer mode/arc centre/board outline для привязки.
- [x] Добавлено откладывание начальной подгонки к пустому viewport.
- [ ] Полная визуальная и функциональная эквивалентность PCB-редактора Altium.

Наблюдения, проверки и оставшиеся расхождения:
`docs/pcb-update/VISUAL_PARITY.md`, `docs/features.json`.

19 октября 2026. Полная программа работ: `ALTIUM_RUST_AGENT_PLAN.md`.
Этот файл задаёт очередность итераций и состояние текущего этапа.

## Обязательный визуальный контракт

По уточнению пользователя интерфейс должен быть точной визуальной копией
доступного Altium Designer Enterprise 24.4.1, на Windows и Linux.
Исходный `ALTIUM_RUST_AGENT_PLAN.md` остаётся основным полным планом;
разделы ниже — только очередность и журнал выполнения его пунктов.
Работы над визуальным соответствием разделов 7 и 25 начинаются сейчас
и продолжаются в каждой итерации, а не только после инженерного ядра.

- [ ] Сопоставлять окна одного редактора, размера, DPI и состояния.
- [ ] Довести меню, quick access, Projects, правую dock-область и вкладки.
- [ ] Воспроизвести Active Bar, Properties, status bar и editor canvas.
- [ ] Проверить общие шрифты, метрики, иконки, цвета и состояния на обеих ОС.
- [ ] Зафиксировать измерения и сравнения; до этого точное совпадение не заявлять.

Связь текущих работ с исходным планом: workspace — §2, координаты — §3,
модель/история — §4, собственный IO — §5, canvas — §6,
оболочка — §7, визуальная сверка — §25, критерии готовности — §26.

## Исходное состояние

В каталоге был только архитектурный план; Rust workspace и Git отсутствовали.
Доступен Altium Designer Enterprise 24.4.1, проект ReferenceB.PrjPcb.
Наблюдены стартовая страница, PCB Editor, File и Properties.
Остальные редакторы и команды требуют собственного обследования.

## Этап 1 — главное окно (текущая итерация)

- [x] Создать Rust workspace: geom, core, io, render, app.
- [x] Отделить документы от состояния представления и интерфейса.
- [x] Создать меню, toolbar, вкладки, дерево документов, Properties,
      Messages, строку состояния, управление видимостью и размером панелей.
- [x] Создание пустого PCB/SCH документа, пример платы, переключение и закрытие.
- [x] Canvas: pan/zoom, fit, сетка, координаты, видимость слоёв, выбор площадок.
- [x] Изменение параметров площадки одной транзакцией, undo/redo.
- [x] Собственный версионируемый JSON, Save / Save As / Open / Recent.
- [x] Запрос Save / Discard / Cancel при закрытии изменённых документов.
- [x] Настройки единиц и сетки, сохранение настроек и размеров панелей.
- [x] Windows-сборка, fmt, clippy, тесты и базовый ручной сценарий в реальном окне — см. `docs/verification.md`.
- [ ] Выполнить Linux CI и визуальный запуск в X11/Wayland.
- [ ] Полный docking, autohide, отдельные нативные окна и восстановление сессии.

### Итерация визуальной сверки главного окна и Properties

- [x] Наблюдать Board и выбранную Pad в открытом Altium без правок проекта.
- [x] Общий title strip, Quick Access, собственные векторные иконки,
      оконные команды через переносимый eframe/winit.
- [x] Properties: поиск, сворачиваемые Selection Filter, Snap Options,
      Board Information, Net Information, Properties и Pad Stack.
- [x] Реальные фильтр выбора pads, оси, настройки привязок и геометрические
      привязки к собственным pads/tracks, без изменения модели рендерером.
- [x] Правка координат/размера/слоя/Designator через историю;
      применение при завершении ввода, ручная проверка X и Undo.
- [x] Встроенные свободные шрифты для Linux, выбор системного Segoe UI
      на Windows, пользовательские пути к шрифтам.
- [x] fmt, clippy, 6 тестов, Windows build и просмотр реального окна;
      cargo check всех targets workspace для x86_64-unknown-linux-gnu.
- [ ] Закрыть оставшиеся расхождения из docs/interface/visual-parity.md;
      точное визуальное соответствие этапа ещё не подтверждено.

Граница этапа: первое работающее окно с собственными документами.
Наличие оболочки не означает функциональное соответствие всей Altium.

## Этап 2 — модель, геометрия и файлы

### Итерация открытия и просмотра Altium schematic (по приоритету пользователя)

- [x] Независимая модель Schematic, исходные записи и архив оригинала; ADR 0002.
- [x] PrjPcb: ссылки на документы, относительные Windows-пути и переносимый поиск регистра.
- [x] SchDoc CFB/ASCII 5.0: чтение записей, кодировки, дробные координаты,
      компоненты/части/display modes и основные графические примитивы.
- [x] Canvas схемы: лист, провода/символы/текст/изображения, zoom/pan/fit,
      выбор компонентов/объектов, Properties и сообщения о неполной поддержке.
- [x] Собственный Save As сохраняет сцену и native-оригинал; защита от записи JSON поверх Altium.
- [x] Собственный минимальный fixture и headless инспектор; чтение нескольких реальных проектов.
- [ ] Довести полную визуальную передачу шаблонов, port/IEEE styles и текста.
- [x] Базовое редактирование сцены через команды/историю — см. итерацию ниже.
- [ ] Полное редактирование, native-запись, электрическая связность и ERC.

### Визуальная сверка ReferenceB — 20 октября 2026

- [x] Разделить точное имя Altium GOST Type A и GOST Type AU; системный
      поиск семейств и свободный Liberation Sans fallback для Linux.
- [x] Размер IEEE Dot, High Current, стрелки LED, раздельные шрифты выводов,
      отступы pin names, GND и NoERC; цвет junction от примыкающего провода.
- [x] Общие Designator/Comment в альтернативных режимах символов;
      восстановлены XP1 и его Comment; pin-owned Net Name остаётся метаданными.
- [x] Вписывание листа, нижняя полоса Editor, рамка/разделители зон,
      системный шрифт и стандартная основная надпись; PgUp/PgDn.
- [x] Сохранить исходные координаты проводов; толщина и привязка осевых
      штрихов к физическим пикселям относятся только к renderer.
- [ ] Подтвердить точное пиксельное совпадение всех фрагментов и Linux GUI;
      оставшиеся различия и наблюдения — в docs/interface/visual-parity.md.

Это частичная реализация §5 и §9–10 исходного плана; исходные критерии
редактирования/native roundtrip ещё не выполнены. PCB импорт отложен по
запросу пользователя. Границы совместимости: docs/altium-import.md.

### Базовое редактирование schematic — 20 октября 2026

- [x] Типизированные команды eda-core: Insert/Delete/Transform/Properties/Text/Width/Batch;
      проверка UUID/ревизии, атомарная валидация, Undo/Redo.
- [x] Новая сцена схемы, R/C, wire, net label, text, GND, junction, graphic line;
      preview, подтверждение и Esc без записи незавершённого действия.
- [x] Одиночный выбор, Move мышью/командой, поворот группы компонента;
      Copy/Cut/Paste/Duplicate с заменой UUID/владельцев/шрифтов.
- [x] Properties: Designator/Comment, X/Y в mm/mil, текст, ширина линии/провода.
- [x] Привязка к сетке, электрическим окончаниям выводов, GND/junctions и проводам.
- [x] Собственный JSON сохраняет правки; source_modified помечает исторический
      native-архив. Исходные records/байты не меняются.
- [x] Windows GUI: размещение R1, 10k→47k, Move/drag/rotate/Undo/Redo, Ctrl+C/V,
      провод между выводами, сохранение/перезапуск/открытие; 27 тестов,
      fmt/clippy без предупреждений, Linux GNU all-targets compilation.
- [ ] Групповой выбор, вершины проводов, stretching, автоматические junctions,
      библиотеки/annotation/иерархия/compiler/ERC и native writer.
- [ ] 10 pm native roundtrip, durable journal, operation ID/idempotence,
      восстановление после сбоя и проверка Linux GUI.

Наблюдены Place/Drawing Tools открытого Altium; порядок меню сохранён,
неподдержанные команды отключены. Part…/Active Bar/Properties ещё частичны.
Контракт и управление: docs/schematic-editing.md, ADR 0003; расхождения — features.json.

Типизированные команды и единицы, идентичность, транзакции, preview/cancel,
ревизии, миграции формата и восстановление. Примитивы, spatial index,
hit testing, snapping и редактирование геометрии. Проверить native Altium
контейнеры на минимальных fixtures до обещаний совместимости.

Критерий: создать, разместить, изменить, переместить, отменить, повторить,
сохранить и открыть реальные объекты через GUI и headless-команды.

## Этап 3 — библиотеки и схемы

### Приоритетная итерация Properties schematic, 20.10.2026 — в работе

Условие пользователя: Components и импорт/размещение из IntLib начинать только
после полного совпадения Properties. Условие пока не выполнено; библиотеки
этой итерацией не реализованы. Пример ReferenceB.IntLib указан пользователем
и остаётся входным файлом следующей работы.

- [x] Наблюдать Document General/Parameters, Component General/Pins,
      Parameter/Designator, Text, Net Label, Power Port, Wire, Text Frame,
      Rectangle и Port в AD 24.4.1. Карта — docs/interface/schematic-properties.md.
- [x] Отдельные формы контекстов, 12 фильтров выбора, исходный Comment,
      скрытые параметры, их видимость/показ имени, семейство/размер/стили шрифта,
      цвета, координаты/поворот, настройки листа и таблица вершин.
- [x] Метаданные моделей из SchDoc, таблицы моделей/параметров/ссылок,
      избранные и Show More; это чтение существующих экземпляров в схеме,
      не импорт библиотек и не реализация Components.
- [x] Проверяемые команды модели с историей; сохранение новых данных в своём
      JSON без изменения source archive. Старые документы читаются через defaults.
- [x] Регрессионные проверки: ширина dock при длинных именах/значениях;
      hidden/raw parameters, модели, отдельные шрифты, atomic rejection и история.
- [ ] Полная визуальная сверка всех контекстов, mixed selection, placement,
      специальные объекты, шаблоны, advanced fields, compiled net metadata.
- [ ] Завершить точное совпадение размеров/иконок/типографики и проверить Linux GUI.
- [ ] Затем Components, SchLib/IntLib и размещение библиотечных компонентов.

Символы и выводы, footprints и pads, компоненты, провода, метки,
junctions, ports, compiler связности, ERC, иерархия и аннотация.
Исследовать соответствующие редакторы Altium отдельно.

Критерий: контрольная схема с независимо проверенным netlist и ERC.

## Этап 4 — PCB и первый путь изготовления

Слои и стек, компоненты, pads/vias/tracks/arcs/regions, nets и ratsnest,
ECO, правила и DRC. Ручная трассировка → walkaround → shove,
заливки с thermals. Gerber, Excellon, BOM, pick-and-place.

Критерий: собственная схема → ECO → плата → DRC → выпуск;
независимый reader проверяет полученные производственные файлы.

## Этап 5 — расширенные подсистемы

HDI/rigid-flex, дифпары и tuning, high-speed/xSignals, 3D/STEP,
варианты и BOM, simulation/SI/PI, OutJob, CAM, Draftsman,
multi-board, harness, сервер Workspace. Для каждой подсистемы —
собственные наблюдения, модель, операции, файлы и критерии.

## Этап 6 — соответствие и производительность

Постоянный реестр функций и расхождений. Windows/Linux CI, DPI и ввод,
малые/средние/большие fixtures, release benchmarks, latency и память.
Сроки и производительность не обещать без измерений.

### Приоритет пользователя: Components и библиотеки — 20.10.2026

Новое прямое указание «Начинай components и библиотеки» разрешает этот этап
до полного завершения Properties. Предыдущая условная последовательность
выше сохранена как история; незакрытые расхождения Properties остаются.

- [x] Наблюдать Components AD 24.4.1 с ReferenceB.IntLib: выбор библиотеки,
      поиск, таблица/Results, Details/Symbol/Model и меню.
- [x] Самостоятельное чтение SchLib Binary 5.0 и SchLib внутри IntLib;
      31 компонент предоставленной библиотеки, варианты частей/отображения,
      бинарные выводы, дробные координаты, UTF-16, параметры, шрифты и ресурсы.
- [x] Панель Components, установка локальных библиотек, сохранение списка путей,
      локальный поиск, сведения, предпросмотр и размещение. Существующие ссылки
      SchLib/IntLib в PrjPcb читаются и доступны в дереве.
- [x] Независимые экземпляры: UUID, свободный designator, поворот, повторная
      вставка, Esc, проверка ревизии, история, собственное сохранение/чтение.
- [x] Регрессии импорта/повреждений/лимитов, нескольких частей и режимов,
      ресурсов, сохранения, preview скрытых параметров, размещения и истории.
- [ ] Полное визуальное совпадение Components, drag-and-drop, обновление/
      удаление подключений, редактор библиотек и обновление экземпляров.
- [ ] Упаковка/аннотация многосекционных компонентов, managed/DbLib/SVNDbLib,
      footprint/3D (в PCB-этапе), проверка Linux GUI.

Карта наблюдений и границы: docs/interface/components-libraries.md.

### Components: визуальная сверка и drag-and-drop — 20.10.2026

- [x] Повторно наблюдать IntLib в AD 24.4.1: Models с белой карточкой,
      raw Comment, электрические маркеры LMV321, References и нижние секции;
      уточнить отличие от ранее наблюдавшегося состояния Symbol/Model.
- [x] Геометрия таблицы/Details/Models при dock 316; сортировка и ширины
      колонок, разделитель, активный фильтр, избранное и Show More/Less.
- [x] Preview с исходными серыми выражениями, скрытыми параметрами,
      переносимыми шрифтами, подбором масштаба и input/output маркерами.
- [x] Перенос строки и карточки на canvas: preview, одна атомарная вставка,
      snap, отмена, guard сеанса/ревизии, Undo/Redo; без Windows-only API.
- [x] Windows GUI: 1396 и 7086, drag C1/C2, Undo/Redo и save/reopen на
      собственной проверочной схеме. 43 workspace tests; Linux GNU all-targets check.
- [ ] Полная пиксельная совместимость всех символов/режимов/DPI, остальные
      pin markers, native dialogs и Linux GUI. Оставшиеся отличия явно
      перечислены в docs/features.json и docs/interface/components-libraries.md.

### Контрольная точка перед публикацией — 20.10.2026

По прямому указанию пользователя разработка приостановлена, текущее состояние
коммитится и публикуется в PCBCraft через remote upstream.

- Реализован черновой connected Drag в eda-core: изменение компонента и
  подключённых wires одной проверяемой транзакцией, ортогональные изгибы,
  фиксированные ответвления/метки, общие узлы, группы и поворот.
- Введены отдельные Drag и Move, меню M → D / M → M, выбор объекта командой,
  отмена по Esc/потере фокуса/выходу за canvas/смене вкладки; preview скрывает
  старую геометрию изменяемых объектов. Move сохраняет разрывающее поведение.
- Наблюдены меню Edit → Move и native Drag корпуса R9 на отдельной копии ReferenceB;
  исходный пользовательский проект для исследования не редактировался.
- Проверены 52 workspace tests, fmt --check, Clippy -D warnings, Linux GNU
  all-targets check. Обновлённая версия PCBCraft ещё не проверена в реальном окне.
- Возобновление: проверить GUI на ReferenceB, углы/поворот/группы и save/reopen;
  завершить сегментное перетаскивание wires, autojunction/слияние и cleanup.
  Полное совпадение всех сценариев редактирования Altium пока не заявляется.

### Интеграция PCB v2 — 21.10.2026

По запросу пользователя интегрирован накопительный PCB-пакет на базе bd76173:
модель/команды PCB, импорт PCB6/ASCII, ручная трассировка, editable polygons,
Properties и экспериментальное native-сохранение с backup и проверкой re-import.
Исправлены ошибки компиляции/Clippy, обновлён lockfile. Пройдены 109 Rust-тестов,
fmt, strict Clippy и Linux all-targets build. Linux GUI: Properties Apply,
перезаливка, маршрут/Undo/Redo, Save с backup и повторное открытие.
Подробности и ограничения: docs/pcb-update/VALIDATION.md и IMPLEMENTATION.md.
Altium interoperability, Windows GUI, реальные платы, DRC/shove/ECO/fabrication
и полное визуальное соответствие остаются непроверенными либо нереализованными.

### Текст схемы при масштабировании — 21.10.2026

По сообщению пользователя устранены скачки метрик/привязок текста при zoom:
фиксированная раскладка glyphs, масштабирование камерой и сохранение subpixel
позиции; текстовые блоки сохраняют переносы строк. Добавлены две регрессии
с реальной тесселяцией, до исправления падающие; 113 тестов, fmt, Clippy и build
проходят. Linux GUI и совпадение canvas после zoom in/out проверены.
Подробности и границы: docs/verification.md и docs/features.json.

### Рост памяти при редактировании — 21.10.2026

Воспроизведён рост до 2,49 ГБ RSS после 200 visibility-правок native-схемы TI.
Убрано дублирование source/records/images между снимками, ограничены Undo/Redo
(128 шагов / 64 МиБ оценки editable snapshots на документ), очищаются устаревшие
текстуры и PCB triangulations. Переиспользуются font slots, clipboard не держит
посторонние изображения, удаление освобождает неиспользуемые image resources.
Восемь memory-проб завершились без неограниченного роста; visibility 600 итераций
держится около 31 МБ RSS в модельной пробе, GUI 200 переключений — около 159 МБ.
122 теста, fmt, strict Clippy и build all-targets проходят. Подробнее: docs/memory.md.

### Возобновление редактирования схем — 21.10.2026

- [x] Сверен native Drag провода в AD 24.4.1 на отдельной копии ReferenceB:
      закреплённые подключения, изгибы и растяжение Т-ответвлений. Изменение
      отменено, пользовательский проект не редактировался.
- [x] `Edit::DragWire`: сегменты/вершины, общая геометрия preview/commit,
      сохранение UUID/стиля, проверка ревизии и атомарный Undo/Redo.
- [x] Маркеры ручек провода; прямой mouse drag и M,D, отмена вне canvas,
      по Esc/потере фокуса/смене вкладки. M,M сохраняет разрывающее перемещение.
- [x] Автоматические точки Т-соединений/выводов по локальной геометрии,
      включая preview. Пересечения середин без узла не соединяются;
      ручные Junction сохраняются, кэш ограничен жизнью View.
- [x] Backspace/Delete возвращает последнюю вершину прокладки;
      щелчок по существующему соединению завершает провод одной транзакцией
      и оставляет инструмент готовым к следующему проводу.
- [x] Windows GUI: connected Drag R1, Drag сегмента с Т-ответвлением,
      Undo/Redo, JSON Save/reopen собственной схемы. 131 workspace tests,
      fmt, strict Clippy; Linux GNU all-targets check без выпуска Linux-билда.
- [ ] Полная сверка на ReferenceB, все угловые режимы, wire-group drag, удаление
      сегментов, overlap cleanup, предупреждение о смене цепи, compiler/ERC
      и новая Linux GUI-проверка. Границы: docs/schematic-editing.md/features.json.


### Групповые операции и сегменты Wire — 21.10.2026

- [x] Наблюдены зелёные маркеры Wire и красные маркеры повторного выбора
      сегмента в AD 24.4.1 на копии ReferenceB. Native Delete и все preferences
      отдельно не воспроизводились; использована официальная документация.
- [x] Смешанные группы компонентов/wires/сегментов: Move, connected Drag,
      поворот, Copy/Cut/Paste/Duplicate/Delete; один шаг истории, проверка
      ревизии и принадлежности примитивов; свободные обозначения внутри группы.
- [x] Выбор сегментов между изгибами/электрическими узлами, в том числе через
      Shift. Удаление сохраняет остальные части; изменённая топология отменяет
      устаревший выбор. Автоматические junctions обновляются.
- [x] Локальное объединение перекрытий с сохранением стиля, Т-узлов и обычных
      неподключённых пересечений. Preview и commit используют одну команду.
- [x] 145 workspace tests, fmt --check, strict Clippy, Windows debug build;
      Linux GNU all-targets check без выпуска Linux-билда. Реальное окно:
      выбор/Drag сегмента, Duplicate с перекрытием, Undo/Redo, JSON Save/reopen
      собственной QA-схемы. Смешанные группы и Shift проверены egui-событиями.
- [ ] Рамочное выделение, групповые Properties, все угловые режимы и locked
      semantics, глобальный cleanup/preferences, net-change warning, compiler/ERC,
      native SchDoc writer, полная Altium-сверка и новая Linux GUI-проверка.
      Предыдущий список незавершённого выше относится к прошлой итерации.


### Исправление открытия PCB из ReferenceB — 21.10.2026

- [x] Воспроизведено `Invalid PCB Pad geometry` на проектном
      `ReferenceBBoard.PcbDoc`: NPTH Pad 0 с нулевой медью и drill ~0,45 мм.
- [x] Поддержан этот объект в модели без выдуманной меди: проверка геометрии,
      bounds/picking, отображение контура и физический вырез при repour.
- [x] Бинарный regression fixture, JSON и native round-trip после перемещения,
      сохранение неизвестного хвоста и архива; невалидные нулевые Pad отклоняются.
- [x] Windows GUI: открытие PcbDoc двойным щелчком из проекта, 15389 примитивов,
      69 компонентов. Пользовательский файл не изменялся, прежнее окно с
      несохранёнными правками сохранено открытым.
- [x] 148 workspace tests, fmt --check, strict Clippy, Windows build,
      Linux GNU all-targets check. Подробности: docs/pcb-update/VALIDATION.md.


### Отображение PCB по открытому Altium — 21.10.2026

- [x] Сверены текущая плата, палитра Top/Bottom и Object Visibility в AD 24.4.1;
      настройки файла отделены от глобальной палитры приложения.
- [x] Modern layer identities и V9 names, layer visibility/opacity/snap grid,
      NAMEON/COMMENTON, font kind/bold/italic, зеркальность и стабильный zoom текста.
- [x] Импорт 2D-проекций ComponentBodies, штриховка вырезов/корпусов; парные legacy
      Regions/Bodies сохранены без повторного рисования (57 + 62 пары).
- [x] Направление обхода contour arcs, швы без огромных miter-треугольников;
      индивидуальные радиусы Pad для renderer/pour/picking; проход сверления.
- [x] 159 tests, fmt --check, strict Clippy, Windows build, Linux GNU all-targets
      check. Исходный файл не изменялся; новые Linux-артефакты не собирались.
- [ ] Полное визуальное совпадение: stroke fonts/embedded glyphs, text styles,
      via-span/adaptive labels, точный raster/hatch и PCB Properties/tools/layer bar.
      Наблюдения, реализация и границы: docs/pcb-update/VISUAL_PARITY.md.


### PCB Properties — 21.10.2026

- [x] Наблюдение AD 24.4.1 Board General/Parameters, Component, Track, Pad,
      Via, Text, Polygon; контекстная компоновка вместо общего Apply-редактора.
- [x] Поддержанные поля через команды/History, координаты относительно origin,
      исправленный Comment из Texts6, dirty/Save/Close и независимые drafts.
- [x] Фильтры и реализованные привязки подключены к PCB canvas; Shift+E.
- [x] 316 px layout regression всех контекстов, mm/mils Enter/Escape, stale/
      invalid drafts, Undo/Redo/Save; Windows via 0.7 → 0.8 → Undo/Redo/Undo.
- [x] 169 workspace tests, fmt --check, strict Clippy, Windows debug build,
      Linux GNU all-targets check без Linux сборки артефакта.
- [ ] Полная pixel parity, 3D padstack/Full Stack/templates, masks/rules,
      component area/opacity, advanced text и mixed Properties.
      Статусы и ограничения: docs/pcb-update/PROPERTIES.md.


### PCB Properties: padstack, 3D и маски — 21.10.2026

- [x] На отдельной копии в AD 24.4.1 сверены режимы Pad Stack и строки
      Copper/Hole/Paste/Solder, Slot/offset и Top/Bottom tenting.
- [x] Типизированный послойный stack в eda-core, общая геометрия picking/render/
      pour, Round/Square/Slot, offset, Flip; PCB6 read/write и сохранение opaque tails.
- [x] Интерактивная 3D-геометрия в eda-render: поверхности/отверстия/barrel/маски,
      вращение/zoom/reset без изменения документа; unified stack editor в Properties.
- [x] Маски Manual/Rule, приоритет и ограниченный scope evaluator, tenting/percentage/
      hole edge; редактор существующих mask rules через History и запись Rules6.
- [x] PCBCraft-запись копии реальной платы с изменёнными масками открыта в Altium;
      подтверждён Top Tented при сохранённом Bottom untented. Исходный файл не менялся.
- [x] 188 tests, fmt --check, strict Clippy, Windows build, Linux GNU all-targets check;
      отдельный Linux build не выпускался. Детали: docs/pcb-update/PROPERTIES.md.
- [ ] Полная pixel/material parity, templates/custom apertures/thermal relief,
      modern native independent masks, plane stack и перестройка native references
      после repour. Native Save при неподдержанном remap сохраняет отказ.


### Продолжение PCB: Pad Edges — 22.10.2026

- [x] Получены обновления origin/main до e8f7ed7 без конфликтов.
- [x] В существующих PCB Snap Options включён Pad Edges по сохранённому
      наблюдению AD 24.4.1; настройки привязки не изменяют документ/историю.
- [x] Проекция на реальную внешнюю границу меди с учётом формы, угловых радиусов,
      поворота, Copper Offset, padstack и видимых слоёв; аналитические дуги.
- [x] Регрессии геометрии/радиуса/слоёв/переключателей и настоящих событий
      PCB canvas, включая Ctrl+Shift bypass. Границы: pcb-update/PROPERTIES.md.
- [ ] Новая живая Altium-сверка приоритетов snap/custom shapes и всех DPI;
      остальные незавершённые PCB-этапы выше сохраняются.

Проверены 193 workspace tests, fmt, strict Clippy и Linux debug build.
Linux GUI: переключение Pad Edges с привязкой к краю / возвратом к сетке,
без dirty state. TI PMP23595 импортируется; PMP22650 по-прежнему блокирует
валидация padstack. Новая живая Altium-сверка пока отсутствует.


### Управление PCB — 22.10.2026

По сообщению пользователя исправлен отдельный PCB input path: Ctrl+wheel
обрабатывает egui zoom_delta; pinch/PgUp/PgDn сохраняют anchor под курсором;
pan правой/средней кнопкой, fit document/board/selection и Home. Вместо
безусловных полос введён Small 90 с сохраняемыми настройками курсора/колеса.
Исправлены быстрый drag-release, выбор объекта командами M,M/M,D, отмена
mouse gesture при потере фокуса/смене вкладки, grid-off drag, поворот во время
mouse move и paste. Добавлены регрессии событий и истории. Новая живая сверка
с AD 24.4.1 недоступна; полное совпадение не заявляется.
Карта управления и границы: docs/pcb-update/CONTROLS.md.

Валидация управления: 210 workspace tests, fmt и strict Clippy. В настоящем
Linux GUI проверены Ctrl-wheel туда/обратно, pan, V,F, перенос + Space,
Undo/Redo, Esc и сохранение точных координат/угла в PcbDoc; Ctrl+W добавляет
проводник с ожидаемой сетью и концами. Исправлен обнаруженный конфликт
ScrollArea двух Description компонента. Тестовые платы находятся вне репозитория.
Отмена также подавляет события нажатия/отпускания мыши в том же кадре до
отпускания кнопки; добавлена отдельная регрессия этой гонки событий.
Локально собран PCBCraft-0.1.0-controls-20261022-x86_64.AppImage (x86_64,
glibc >= 2.39, вне Git). Проверены запуск пакета, zoom, выбор компонента,
отсутствие конфликта Description и Esc + отпускание мыши с побайтово
неизменным сохранённым PcbDoc.

### Cross Probe между схемой и платой — 22.10.2026

- [x] Tools > Cross Probe / T,C, постоянный режим, Ctrl+click для разового
      перехода; Esc/правая кнопка завершают режим.
- [x] Компоненты по native ID с fallback на designator, pins/pads по номеру;
      цепи по связности проводов/выводов, junction и labels.
- [x] Область поиска PrjPcb, открытие противоположных документов по запросу,
      выбор при нескольких вкладках и сообщения о неоднозначности.
- [x] Только состояние вида: центрирование, слои, выделение/подсветка;
      модель, dirty state и Undo не меняются.
- [x] Автотесты и Linux GUI для обоих направлений, pin и net.
- [ ] Компиляция иерархических цепей, harness/bus/variants и split-view;
      точная живая сверка поведения и масштаба с AD 24.4.1.

Описание и границы: docs/pcb-update/CROSS_PROBE.md. Тестовый проект использует
только собственные fixtures; файлы TI остаются вне Git.
Проверка на локальном TI PMP23595: 164/167 компонентов имеют соответствие,
для 36 repeated-channel компонентов добавлен явный выбор физического
экземпляра. Native SOURCEUNIQUEID принимает оба разделителя пути, / и \.
Повторяемые цепи без однозначного PCB net остаются неподдержанными;
иерархический компилятор не подменяется совпадением названия.
Итог: 219 workspace tests, fmt, strict Clippy и debug build проходят.
Linux GUI дополнительно проверен выбор физического экземпляра CHANNEL1/CHANNEL2.

### PCB 3D Layout Mode — 22.10.2026

- [x] Клавиши 3/2 и меню View, отдельная камера, orbit/pan/zoom, Top/Bottom/Fit.
- [x] Объёмная плата с толщиной, сквозными отверстиями/вырезами и геометрией
      наружных слоёв; проекции корпусов с высотой/standoff и fallback по pads.
- [x] Depth picking, выбор/Properties, поворот вокруг component origin, Undo,
      Cross Probe и переключатель видимости компонентов.
- [x] Кэш сцены/текстуры, неизменность модели при навигации, регрессии быстрых
      жестов/окклюзии/возврата 2D и незавершённых правок.
- [ ] STEP/Parasolid meshes, точные материалы/маски/шелкография, collision,
      rigid-flex и полная живая сверка AD. Корпуса явно упрощённые.

Описание и ограничения: docs/pcb-update/3D_VIEW.md.

Проверка 3D: 228 workspace tests, fmt и strict Clippy проходят. Локальный
release AppImage проверен на TI PMP23595: выбор/Properties, orbit/zoom,
вид снизу со сквозными отверстиями и возврат в 2D. TI файлы вне Git.

### Layer Stack Manager — 22.10.2026

Предыдущие изменения 3D зафиксированы отдельным коммитом eb513d9.

- [x] Design > Layer Stack Manager, Stackup/Via Types/Impedance, Properties,
      поперечное сечение, мм/mil, Apply/Discard и атомарный Undo.
- [x] Изменение/добавление/порядок слоёв, материалы, Dk/Df, толщины;
      стабильные copper IDs и проверки ссылок при удалении.
- [x] Through/Blind-Buried/Micro presets с диапазонами, диаметром/отверстием,
      использование при размещении via.
- [x] Профили одиночного/дифференциального импеданса, аналитическая оценка,
      подбор ширины и применение одиночной ширины для трассировки.
- [x] JSON persistence, сохранение исходного архива, согласованная толщина 3D.
- [ ] Native stack serialization/custom-data import, полевой решатель,
      rigid-flex/backdrill, auto symmetry, native rules и точная живая сверка AD.

Подробные ограничения: docs/pcb-update/LAYER_STACK_MANAGER.md.

Проверка LSM: 236 workspace tests, fmt, strict Clippy и debug build. Linux GUI:
слои/via/импеданс 50 Ω/Apply/Undo; TI PMP23595 — исходные 6 слоёв и материалы,
2.2082 мм; открытие/Apply без изменений сохраняет clean state.

### Schematic Annotation — 22.10.2026

- [x] Tools > Annotation > Annotate Schematics..., Annotate и отдельный ECO.
- [x] Проектные листы/область выделения, порядок обхода и листов, Start Index,
      сохранение заблокированных номеров, сброс/дубликаты и предпросмотр.
- [x] Общий номер многосекционных корпусов, совместимая упаковка новых секций,
      защита от коллизий/устаревшего проекта и валидация всех листов до Apply.
- [x] Обновление видимого/скрытого текста, Unique ID неизменны, Undo по листам.
- [ ] Живая сверка AD, compiled channels, advanced sorting/part swapping,
      back annotation/quiet mode/native SchDoc writer и общий project Undo.

Поведение и ограничения: docs/ANNOTATION.md.

Проверка Annotation: 247 workspace tests, fmt и strict Clippy. Linux GUI:
меню/предпросмотр/ECO/Undo/Redo/JSON; TI PMP23595 — 5 листов, 167 компонентов,
сохранение существующих номеров и Validate полной перенумерации без Apply.

### PCB View Configuration and connections — 22.10.2026

- [x] Previous schematic annotation committed and pushed to main: 84951bf.
- [x] Right dock View Configuration, L/Panels/menu/layer-bar entry points.
- [x] Layer visibility/colors, physical ordering, object transparency, labels,
      single-layer dim/hide, grid/system colors and basic 3D view controls.
- [x] Physical copper islands and minimum spanning connection guides; native
      polygon NET inheritance, plated/via spans, holes and stale pour handling.
- [x] Bounded background calculation/cache with stale-result rejection.
- [ ] Exact AD visual/functional verification: official docs blocked by proxy;
      native profile persistence, draft objects, via span markers and full 3D
      settings remain incomplete. See docs/pcb-update/VIEW_CONFIGURATION.md.

Validation: 255 workspace tests; fmt, strict Clippy, debug build. Linux GUI:
fixture connection lines and view toggles; TI PMP23595 Layers & Colors and
3D View Options. No TI project files committed or modified.

### View Configuration — живая сверка AD 24.4.1, 22.10.2026

- [x] Сверены Layers & Colors, System Colors, 2D/3D View Options,
      Object Visibility и Section View на исследовательской копии PcbDoc.
- [x] Левая панель 330 px, независимые Properties справа, вкладки/строки/отступы,
      дерево ролей и пар механических слоёв, Used On и настроенные пустые слои.
- [x] Независимые visibility/draft/opacity, матрица Apply/Cancel, цвета,
      зеркальный вид снизу, подписи и режимы соединений; быстрые жесты мыши.
- [x] Native 3D colors/opacity/background, перспектива, масштаб толщины,
      материалы и прозрачность, независимый single-layer и dim/highlight.
- [ ] Полное совпадение: профили/import/export, Section View, native color picker,
      docking/autohide, категории без модели/рендерера, text draft, via-span,
      native range/parent matrix editing и точные состояния/метрики Linux.

Статус остаётся partial; наличие отключённых элементов не считается реализацией.
Границы и проверки: docs/pcb-update/VIEW_CONFIGURATION.md, docs/features.json.
Проверка: 267 workspace tests, fmt --check, strict Clippy, Windows debug build,
Linux GNU all-target cross-check. Linux GUI/артефакты не запускались и не собирались.

### PCB editor performance — 22.10.2026

- [x] View Configuration committed separately: 3402caa.
- [x] Read-only CPU probe on the 15,513-object research PcbDoc: original warm
      2D frame ~150 ms, first synchronous 3D frame ~14.8 seconds.
- [x] Revision/preview-bound geometry caches, broad-phase picking, shared
      previews and generation-based connection results; linear selection lookup.
- [x] Bounded background scene/image preparation, stale-result rejection,
      matching depth-buffer picking, cached triangle sort inputs.
- [x] Optimized normal dev profile with debug symbols/assertions retained.
- [x] Updated warm 2D CPU paint+tessellation ~22–23 ms; first 3D image ~0.87 s
      in background, UI request ~10 ms and subsequent waiting frames <1 ms.
- [x] Windows GUI: Fit Board, via selection/move/Undo, scroll, 2D/3D zoom/switch.
- [x] 273 workspace tests, fmt --check, strict Clippy, Windows dev build and
      Linux GNU all-targets cross-check; no Linux distributable built.
- [ ] Continuous 60 FPS software 3D, large-board repour/model-copy costs and
      Linux native UI/performance verification remain open.

Measurements, method and limits: docs/pcb-update/PERFORMANCE.md.

### PCB release latency — 22.10.2026

- [x] Isolated synchronous auto-repour as the release stall: ~1.4 s on the
      15,513-object / three-polygon board.
- [x] Commit position immediately, reuse matching preview, avoid cloning the
      replaced board; measured ready-preview commit ~9 ms + worker snapshot ~11 ms.
- [x] One background repour per document; stale results rejected, newer edits
      coalesced, application deferred during gestures/Properties/model dialogs.
- [x] Typed derived-geometry update through core History: validation/revision
      checks, one Undo per move, Redo preserved, saved-content state retained.
- [x] 280 tests, fmt --check, strict Clippy, Windows GUI via move/Undo/Redo,
      Windows dev build and Linux all-target cross-check.
- [ ] Full Altium pour algorithm parity and native Linux latency measurements;
      explicit repour/native save rebuilds remain synchronous.


### Document tabs and editor bottom strips - 23.10.2026

- [x] Observe live AD 24.4.1 PCB/SCH strips on the original ReferenceA project.
- [x] Shared 26 px document tabs, file icons, Home Page, guarded context close.
- [x] 22 px PCB layer strip, swatches, copper ordinals, mechanical order, LS and scroll controls.
- [x] Schematic Editor/sheet strip; unsupported compiled view disabled.
- [x] Windows GUI switching documents, active copper layer, LS and scrolling; view-only catalog regressions.
- [x] 288 workspace tests, fmt check, strict Clippy and Windows dev build.
- [ ] Exact icon/raster/overflow/DPI parity, custom layer sets, batch close and split windows.

Observation and limitations: docs/interface/document-bars.md.

### Projects panel — 23.10.2026

- [x] Observe live AD 24.4.1 ReferenceA tree, Settings/Output Jobs, Generated, variants and nested sheets.
- [x] Shared 554 px dock, 16 px tree rows, source file icons and sheet numbering.
- [x] Source/generated metadata separation, variants/BOM flags and best-effort sheet references.
- [x] Stable expansion/selection, independent search, guarded document activation/close.
- [x] Fix zero-sized startup collapsing Projects and retain width across PCB/SCH.
- [x] Windows GUI on ReferenceA, 292 workspace tests, fmt check, strict Clippy and dev build.
- [ ] Exact raster/DPI parity, native status/menus, docking and compiled project behavior.

Observation and limitations: docs/interface/projects-panel.md.

### Interactive Routing — 23.10.2026

- [x] Observe live AD 24.4.1 routing panel, Ctrl+W, start/Tab/Escape and conflict list on a separate ReferenceA PCB copy.
- [x] Independent core Ignore/Stop/bounded Walkaround; analytic track clearance and bounded arc approximation.
- [x] Atomic via-span checks, Backspace restoring layer/cache, preview/commit geometry agreement.
- [x] Prevent blocked targets from automatically finishing; preserve pours for empty previews.
- [x] Native-style routing Properties sections, initial unconditional width/via rule values, Shift+R and keypad layer switching.
- [x] Windows GUI on NetDA1_34: route leg, via, backtrack, mode, pause, finish and Undo; original source hashes unchanged.
- [x] 301 workspace tests, strict Clippy, fmt check and Windows dev build.
- [ ] Full parity: scoped/matrix design rules, shove/HugNPush, arc corners, gloss/loop removal, autocomplete and exact UI/DPI/Linux behavior.

Observation, timing and limitations: docs/pcb-update/INTERACTIVE_ROUTING.md.

### PCB component selection by mounting side — 23.10.2026

- [x] Fix opposite-side component primitives intercepting selection on Top/Bottom; use owner side even for multilayer pads and respect owner locks.
- [x] Share filtering between click, drag, box selection and Select All; prioritize active-layer geometry in hit ordering.
- [x] Regression coverage for overlapping top/bottom components through real canvas pointer events and editor box/drag paths.
- [x] Windows ReferenceA GUI: DA1 selects on Top, G1 on Bottom; selecting DA1's location on Bottom no longer selects DA1. Source PCB not saved or modified.
- [x] Workspace tests, fmt check, strict Clippy and Windows dev build pass.
- Scope: active outer copper layers in component selection mode; full Altium selection preference equivalence remains unverified.

### Layer Stack Manager restoration — 23.10.2026

- [x] Commit and push preceding work: 83f2b7c on origin/main.
- [x] Review existing manager and official behavior documentation.
- [x] Implement independent symmetric material edits, paired layer operations and atomic invalid-operation rejection.
- [x] Add plane insertion, isolated draft Undo/Redo and stack-specific Save/Close command handling.
- [x] 310 workspace tests, fmt check, strict Clippy and Windows dev build; original ReferenceA hashes unchanged.
- [x] Live AD 24.4.1 reference after unlock: Stackup, dielectric Properties, Via Types and empty Impedance on original ReferenceA without edits.
- [x] Colored stack grid, imported overlay rows, total thickness including masks, stack/board tab navigation and contextual menus.
- [x] Real ReferenceA window: symmetry, paired insertion, Undo, PCB/Stackup switching with retained draft; 311 workspace tests, strict Clippy, fmt and Windows build pass.
- [ ] Final pixel parity and advanced Layer Stack Manager coverage remain incomplete.
- [ ] Full restoration remains open: document tabs/workspace behavior, exact layout, material library, native stack interoperability, whole-stack mirroring, backdrill, rigid-flex and solver/rule bindings.

Evidence and limits: docs/pcb-update/LAYER_STACK_MANAGER.md.

- 2026-10-23: Layer Stack Properties follow-up: observed copper/mask fields in AD; added persistent fabrication metadata, type-dependent property panels, collapsible sections and field search; constrained via spans and impedance references. Advanced native import, roughness and populated-profile parity remain partial.

- 2026-10-23: Committed and pushed all Layer Stack work as 91a4110. PCB cursor fix: draw the crosshair at the raw screen pointer, independent of snapped tool coordinates; add rendered-cursor regression for motion inside one grid cell and mirrored view.


### PCB Rules and DRC — 23.10.2026

- [x] Observe live AD 24.4.1 Rules and Design Rule Checker on ReferenceA without saving reference changes.
- [x] Native-field rule drafts, query/priority/matrix resolution, atomic history and safe native roundtrip.
- [x] Routing width/layer/via/clearance/outline integration and scoped polygon clearances.
- [x] Background online/batch checks, stale-result rejection, limits, violations and marker navigation.
- [x] Regression coverage for geometry, scopes, routing, rules/history, UI bounds and native preservation.
- [ ] Complete rule-family coverage and query language; native checker report/persistence and advanced routing behavior.
- [ ] Exact UI/DPI/Linux parity; Rule Wizard and Query Builder.

Evidence, validation and explicit remaining scope: docs/pcb-update/RULES.md.

- 2026-10-23 Rules follow-up: native Simple/Advanced polygon connections drive pours; added mask sliver and non-text silk/mask checks, shared cutout clearances, zero-edge crossing, independent routing violations, via manufacturing checks, HTML reports, per-file checker preferences and native new/copy priorities. Full rule-family/query/UI parity remains open; details and validation in docs/pcb-update/RULES.md.


### PCB Rules visual follow-up — 24.10.2026 (in progress)

- [x] Pull the configured upstream/main updates; no origin remote is configured.
- [x] Observe live AD 24.4.1 ReferenceB rule summaries, priorities, Query Builder/Helper,
  Wizard, checker and supported constraint forms; further exploration uses a copy.
- [x] Implement bounded query/wizard drafts, native catalogue order, summary pages,
  fixed dialog regions, per-layer preferred widths and imported numeric class members.
- [x] Fix numeric input buffering, child widget IDs, duplicate native rule identities
  and Unicode encoding shadows; original PCB hash remains unchanged.
- [x] Linux all-target compilation.
- [ ] Complete equal-size/DPI visual comparison and all related Rules surfaces.
- [x] 352 workspace tests, strict all-target Clippy, fmt check and Windows bins/examples build.
- [ ] Resume live comparison: after manual closure of the Panels popup,
  Computer Use reported a physical Escape stop. Latest drag/ID/Unicode GUI
  verification remains pending; no completion claim.

Evidence, exact implemented scope and remaining differences: docs/pcb-update/RULES.md.

### ReferenceB adaptive PCB 3D correction — 24.10.2026

- [x] Reproduce the user's ReferenceBBoard.PcbDoc in Windows and inspect the same
  source in AD Enterprise 24.4.1, including both sides, bodies, materials and
  native 3D View Control commands; source never saved in either editor.
- [x] Correct solder-mask/copper/paste depth and apertures; copper polygon
  cutouts no longer drill the laminate. Remove legacy duplicate geometry from 3D.
- [x] Read embedded STEP using independent Rust Truck/ruststep, preserve AP214
  face colours and share meshes. Native Y/Z mounting reflection fixes the large
  asymmetric bottom connector; validated move/rotate/flip follow history.
- [x] Render planar text, correct bottom camera/Flip, perspective depth,
  translucent shared edges and camera-space lighting. Add native-order camera
  menu commands; unsupported Fold/Unfold stays disabled.
- [x] Read-only reference audit: 16 models / 10,293 triangles / 62 placements,
  no missing meshes, maximum saved-projection XY bound error 0.000122 mm.
- [x] 361 workspace tests, strict all-target Clippy, formatting, Windows dev
  build and Linux GNU all-target compilation. Original source SHA-256 unchanged.
- [x] Live Windows top/bottom/flipped-isometric/model picking and locked-object
  protection checked after changes.
- [x] Unlocked bottom component Space/Undo in the live 3D window; native source
  never saved. Existing unsupported InPad repour query remains outside this fix.
- [ ] Exact pixel parity: native lighting/specular/antialiasing, font metrics,
  exact camera/Fit values, equal viewport/chrome/docks and Linux runtime GUI.
- [ ] Parasolid/textures/transformed assemblies, section/collision/rigid-flex
  and native writing of changed imported 3D placements remain unsupported.

Evidence, independent dependencies, regression coverage and explicit limitations:
docs/pcb-update/3D_VIEW.md and docs/pcb-update/THIRD_PARTY_NOTICES.md.

### ReferenceM PCB opening hang, 24.10.2026

- [x] Reproduce the reported 2D PCB opening hang with a memory-limited read-only
  release probe; GDB identifies unbounded STEP trimming-curve subdivision on NaN.
- [x] Reject non-finite converted surfaces before tessellation and bound boundary
  curve subdivision/evaluations. Preserve source archives and warn on fallback.
- [x] Accept native unassigned `POURINDEX=-1` using stable source order, while
  retaining the exact property and rejecting other invalid unsigned indices.
- [x] Regression tests, workspace tests, strict all-target Clippy and fmt check.
- [x] Unchanged ReferenceM: 41,475 items, load ~0.9 s, headless peak ~270 MiB;
  rebuilt AppImage rendered the board in the real Linux KDE/Wayland window.
- [ ] General STEP conversion coverage, complete resource isolation for arbitrary
  CAD inputs and full-frame large-board performance remain open.

Evidence and limits: docs/pcb-update/PERFORMANCE.md.

### ReferenceM 2D cursor and pan performance, 24.10.2026

- [x] Separate 2D paint/tessellation from loading and GUI document comparisons:
  ~92,000 shapes / 1.37M vertices per frame, ~11 ms per saved-content comparison.
- [x] Cache saved-content checks by History revision; explicitly refresh on Save
  and retain live pending-edit checks. Stop sending unchanged window titles.
- [x] Cache tessellated artwork and keep wgpu buffers resident for camera pans;
  preserve screen-anchored hatching, live tools, clipping and font-atlas ownership.
- [x] Spatial candidates for exact picking/snapping, including internal pad lands
  and original-board snapping during previews.
- [x] Real GPU image comparison against egui; cache/invalidation, preview,
  spatial snapping and saved-state regressions; fmt/Clippy/workspace checks.
- [x] Real Linux window: Fit Board, cursor movement and middle-button pan.
  Warm CPU frame medians ~6–11 ms / ~5–7 ms under concurrent CPU load;
  callback submission probe ~2.8 / ~2.5 ms. Rebuild the AppImage.
- [ ] Cache rebuild spikes on zoom/configuration/content changes,
  editing-preview costs, presentation FPS, Windows runtime and Altium parity.

Measurements and limitations: docs/pcb-update/PERFORMANCE.md.

### ReferenceM 3D orbit performance, 24.10.2026

- [x] Reproduce slow software refresh after an orbit change: ~782 ms on ReferenceM.
- [x] Resident wgpu 3D vertices/styles, hardware depth and cached image target;
  pan/zoom update the camera, opaque faces need no sorting. Sort translucent
  faces with stable linear radix passes on orientation/thickness changes.
- [x] Current-camera BVH ray picking, with perspective/flip/thickness and hidden
  material checks; retain selection of parent components and material styling.
- [x] GPU pixel comparisons vs software, buffer reuse, full-canvas handoff,
  stable depth ties and BVH-vs-raster regression checks.
- [x] Read-only completed-GPU probe on 2.18M triangles: orbit median 6.12/p95
  8.18 ms, pan 0.55/zoom 1.44 ms; native eframe automatic navigation probe.
- [x] Inspect the native application's own captured GPU frame; run 376 workspace
  tests, fmt/strict all-target Clippy and rebuild the Linux AppImage.
- [ ] First-scene latency (~3.7 s), arbitrary-board memory budgets, presentation
  FPS on other hardware/Windows and full native pixel/STEP support parity.

Measurements and limitations: docs/pcb-update/PERFORMANCE.md.

### ReferenceM intermittent pan stalls, 24.10.2026

- [x] Reproduce long-pan boundary stalls: 16/360 CPU frames >33 ms, maximum 158 ms.
- [x] Keep complete visible-layer artwork at a fixed document anchor; all camera
  translations reuse meshes/GPU buffers. Keep grid and origin live, with grid
  outline intersections computed per row. Cull offscreen hatch generation.
- [x] Invalidate glyphs on font-definition changes, rather than egui UI-atlas
  replacement; preserve the artwork's private atlas and DPI invalidation.
- [x] Regressions: offscreen objects revealed by long pans vs fresh GPU images,
  live grid/origin, atlas replacement/font changes and scanline boundary rules.
- [x] Release sweep at 15/150/1500 px/mm: no CPU frames >33 ms after initial build.
  Real XWayland long-pan batches: CPU medians ~1–2.7 ms, maximum 7.75 ms;
  no artwork rebuilds during short or long pan. Sampled GUI RSS ~672 MiB.
- [x] Workspace tests, strict all-target Clippy and formatting; rebuild and
  launch the AppImage. Package through a temporary file so a running old image
  does not prevent replacing the artifact.
- [ ] Cold scene construction on zoom/content/display changes, whole-scene
  memory at extreme zoom, editing previews, presentation FPS and Windows runtime.

Measurements and limitations: docs/pcb-update/PERFORMANCE.md.

### Oblique 3D transparency stripes, 24.10.2026

- [x] Reproduce incorrect blending of two tilted translucent sheets with
  opposite triangle diagonals; centroid ordering failed by 28/255 per channel.
- [x] Replace CPU triangle ordering with per-pixel front-to-back depth peeling,
  opaque depth rejection and RGBA16Float accumulation; retain resident geometry.
- [x] Reuse two depth targets and one layer image, with eight depth layers per
  pixel and a two-ULP separation between layers to avoid self-blending.
- [x] Check sheet ordering across triangle seams in orthographic/perspective
  cameras; retain material, opacity, selection, picking and cache regressions.
- [x] Capture and inspect ReferenceM at a fixed oblique angle in a native window;
  the large stripe/triangle patches disappear. Completed GPU orbit median 3.66/
  p95 4.20 ms, zoom median 5.07 ms on the local RX 7900 XTX.
- [x] Workspace tests, strict all-target Clippy/fmt and rebuilt Linux AppImage.
- [ ] More than eight translucent layers per pixel, software-fallback centroid
  transparency, antialiasing and full native pixel parity remain limited.

Measurements and limitations: docs/pcb-update/PERFORMANCE.md.

### PCB visible-layer cycling with mouse side buttons, 24.10.2026

- [x] Back/Forward presses over the focused 2D canvas select the previous/next
  visible layer in tab order, with wraparound and live visibility changes.
- [x] Skip system display categories; handle hidden active layers and empty or
  single-layer strips. Ignore releases, holds, panels and modal dialogs.
- [x] Send layer requests through the editor's existing route-layer handler,
  preserving via checks, previews and Undo. Normal switching only changes view.
- [x] Real egui input and controller regressions; native ReferenceM window with
  application-local synthetic pointer events and inspection of its GPU capture.
- [x] Formatting, strict workspace/all-target Clippy and 382 workspace tests.
- [x] Rebuild the Linux AppImage, verify its checksum and launch ReferenceM without
  renderer errors. Reference PCB/project hashes remain unchanged.
- [ ] Physical side-button input and matched Altium/Windows runtime comparison.

Controls and validation: docs/pcb-update/CONTROLS.md.

### ReferenceM routing startup failure, 24.10.2026

- [x] Reproduce route startup abort on `InLayerClass('L3_L4')`; inventory the
  board's clearance scopes, layer classes and native RoomDefinition contours.
- [x] Support layer-class membership, native layer aliases, straight room
  containment and InPoly/InPolygon; retain errors for unknown predicates and
  unsupported room geometry. Normalize native retraced room closing edges.
- [x] Resolve room-dependent clearance for the proposed segment, with cached
  obstacle geometry and conservative bounds covering matrix clearances.
- [x] Query/geometry and post-placement room/matrix regressions; read-only
  ReferenceM preview/place/finish/Undo probe on AR_QSPI_CS, 0.254 mm width.
- [x] Full native window with application-local synthetic mouse events: start,
  copper preview, placement/completion in one transaction and exact Undo;
  inspect the application's GPU screenshot. No source file writes.
- [x] Formatting, strict workspace/all-target Clippy and 389 workspace tests.
- [x] Rebuild the Linux AppImage, verify its checksum and launch ReferenceM without
  renderer errors; reference project/PCB hashes remain unchanged.
- [ ] Curved room scopes, complete query language/DRC, physical input and new
  Altium/Windows runtime comparison remain outside verified coverage.

Cause, implementation and validation: docs/pcb-update/INTERACTIVE_ROUTING.md.
