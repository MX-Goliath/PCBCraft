# План реализации PCBCraft

### Приватность репозитория — 04.11.2026

Имена частных эталонов заменены нейтральными обозначениями ReferenceA и далее.
Личные пути, контрольные суммы исходных проектов и личные авторские email
удалены также из истории Git. Синтетические fixtures и полный журнал плана
сохранены. Добавлены правила исключения частных файлов и проверка приватности
в CI; локальные отчёты по умолчанию создаются в target. См. docs/privacy.md.

История удалённых main/master и локальных служебных ссылок очищена; прежние
объекты удалены из локальной базы Git. Проверки всей достижимой истории,
негативные сценарии сканера (индекс, UTF-16, метаданные коммитов), cargo fmt,
строгий Clippy и cargo test --workspace пройдены. Содержимое исходников после
переписывания истории совпадает с проверенным деревом.
Старый SHA ещё доступен через GitHub API вне веток: удаление серверных копий
требует отдельного обращения в GitHub Support; локальный текст подготовлен,
обращение не отправлялось.

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


### PCB 2D zoom stalls, 25.10.2026

- [x] Reproduced synchronous whole-scene rebuild on every zoom frame using
  read-only ReferenceM, 41,475 items. Parent baseline: zoom CPU mean 49.56 ms,
  p95 54.55/max 128.62 ms, 119/120 frames above 33 ms.
- [x] Pan/zoom transform resident buffers. Exact detail refines after 120 ms
  idle on one background worker with independent fonts/DPI/context and GPU
  preparation. Scale/content keys reject stale results; new content retains an
  old running job until it exits. Live overlays and current-camera picking stay
  active. Tests compare refined and fresh geometry, GPU scale pixels/buffer
  reuse, stale jobs, disconnection and content changes during refinement.
- [x] Release zoom CPU mean 7.62/p95 14.48/max 18.56 ms, zero >33 ms of 120;
  completed GPU median 2.46/p95 5.59/max 9.09 ms, refinement publication max
  6.29 ms. Native window zoom/settle and screenshot inspected; warm CPU median
  2.20/p95 13.38/max 51.60 ms, six >33 ms of 1530 under concurrent compilation.
  Source PCB hash unchanged. These are not presentation FPS.
- [x] `cargo fmt --all -- --check`, strict workspace/all-target Clippy and
  `cargo test --workspace` pass. Linux AppImage rebuilt, SHA256 sidecar checked,
  packaged app opened the source PCB in the real desktop session. Source hash
  remains unchanged.
- [ ] Cold content/display rebuilds, including incoming connection results,
  remain synchronous. Snapshot cost, arbitrary-board/extreme-zoom memory bounds,
  Windows runtime and exact Altium visual/performance parity remain open.
  Gesture antialiasing/labels use scaled previous detail until refinement.
  See `docs/pcb-update/PERFORMANCE.md` for reproduction and limits.


### PCB performance, 25.10.2026

- [x] Замерены движение и перестройка 3D на ReferenceA и ReferenceM.
- [x] Переиспользуемый проверенный черновик перемещения, неизменяемые снимки
  для фоновых задач, частичная перерисовка и обновление пространственного индекса.
- [x] Подготовка преобразования 3D на экземпляр модели; кэш геометрии основания,
  масок и незатронутых объектов, индекс вырезов и ускорение построения BVH.
- [x] 398 тестов, fmt, строгий Clippy; реальные окна 2D/3D, снимки и GPU readback;
  release-сборка и обновлённый Linux AppImage.
- [ ] Первые тяжёлые кадры и глобальная перестройка основания/масок большой платы
  остаются дорогими. Постоянные 60 FPS для всех операций не подтверждены.

Измерения и границы проверки: `docs/pcb-update/PERFORMANCE.md`,
`docs/pcb-update/performance-2026-10-25.json`. Исходные платы не изменены.

### Перетаскивание дорожек PCB, 25.10.2026

- [x] Исправлен обычный drag дорожки, ранее разрывавший соединения. Drag M,D
  и ручки концов свободных медных дорожек используют маршрутизатор eda-core.
- [x] Перестраиваются соседние сегменты, сохраняются контакты площадок/via,
  Т-ответвления и пересечения прямых дорожек. Углы 45°/90°, обход препятствий,
  проверка зазоров, заблокированных объектов и атомарный отказ.
- [x] Commit совпадает с предпросмотром, одна операция Undo/Redo; отмена и
  ошибка убирают черновик. Изменение preview/commit запрашивает перерисовку.
- [x] 414 workspace tests, fmt check, строгий all-target Clippy и dev-сборка
  на закреплённом Rust 1.90.0. Новых регрессионных проверок 16.
- [x] Реальное Linux X11 окно, primary-button drag без модификаторов:
  дорожка 20 мм между двумя площадками перемещена на 5 мм, видны оба подвода
  в preview и после отпускания. Save, Undo и точный Redo проверены по JSON;
  площадки неизменны. Использован собственный временный документ.
- [ ] Shove, gloss, удаление петель, смешанные группы, произвольные дуговые
  контакты и совпадение с Altium требуют дальнейшей реализации/проверки.

Подробности: `docs/pcb-update/INTERACTIVE_ROUTING.md`, `pcb.track_drag` в
`docs/features.json`. Нового наблюдения Altium и сборки AppImage не было.

### Выбор и перемещение компонентов ReferenceM, 25.10.2026

- [x] На пользовательской ReferenceM определены две причины: отсутствие выбора
  середины корпуса и блокировка целого компонента из-за закреплённых деталей.
  Из 37 корпусов с такими деталями 31 сам не заблокирован.
- [x] Выбор корпуса по видимым границам посадочного места, независимый фильтр
  Components, общая логика для щелчка и drag, обновление выделения/Properties
  сразу после щелчка. Сохранён выбор по стороне монтажа.
- [x] Move/Drag/Rotate/Flip/Delete целого незаблокированного компонента сохраняют
  флаги блокировки дочерних объектов. Самостоятельное редактирование закреплённой
  детали и перемещение заблокированного компонента остаются запрещены.
  Заблокированный компонент доступен для просмотра и явной разблокировки.
- [x] 420 тестов workspace, fmt, строгий Clippy/all-targets на Rust 1.90.0.
  Read-only probe на полной исходной плате: R189/U10/C62, выбор и move/rotate.
- [x] Реальное окно AppImage на собственной копии U10: выбор корпуса, drag,
  JSON Save, Undo/Redo; сохранены обе блокировки и все ID дочерних объектов.
  Обновлён AppImage, проверен SHA256. Исходный PcbDoc не сохранён, хеш прежний.
- [ ] Выбор по точному произвольному контуру, полное совпадение предпочтений
  Altium, Windows runtime и задержки drag на полной плате ещё не проверены.

Подробности и границы проверки: `docs/pcb-update/CONTROLS.md`,
`pcb.component_selection_editing` в `docs/features.json`.

### Общая область камеры 2D/3D, 25.10.2026

- [x] По выбору пользователя режимы передают центр области платы и масштаб,
  сохраняя выбранный угол 3D. Панорамирование и zoom работают в обе стороны.
- [x] Передача ждёт готовой сцены и реального размера viewport. Возврат во время
  подготовки сохраняет исходную камеру. Отложенный Fit не переходит между режимами.
- [x] Повторные переключения сохраняют точные параметры без накопления сдвига;
  изменение размеров панелей не меняет масштаб. Вид с ребра сохраняет опорную
  точку платы. Модель и Undo не меняются при навигации.
- [x] 429 workspace tests, fmt, строгий all-target Clippy на Rust 1.90.0.
- [x] Реальное окно Linux AppImage на отдельной копии U10: pan/zoom, наклон 3D
  и пять повторных переключений. Возврат 2D и наклонного 3D совпал по всем пикселям.
  AppImage обновлён, SHA256 проверен. Исходный ReferenceM не изменён.
- [ ] Полное совпадение камеры/проекции Altium и Windows runtime не проверены.

Подробности: `docs/pcb-update/3D_VIEW.md`, `pcb.shared_camera` в
`docs/features.json`.

### Трассировка ReferenceB и индекс препятствий, 25.10.2026

- [x] Воспроизведён отказ запуска на полной ReferenceB (15 513 объектов): отсутствовали
  CompParameterValue/Like и HasPad; параметры PrimitiveParameters не читались.
- [x] Импорт базовых параметров отдельно от native metadata; проверка владельцев,
  границ и дубликатов. Восстановление старого JSON по native UNIQUEID, сохранение
  параметров в модели/JSON; отказ неподдерживаемой нативной записи изменений.
- [x] CompParameterValue/Like, HasPad и wildcard HasFootprint; проверки приоритета
  зазоров, ошибок и неизвестных условий. На ReferenceB найдены все 24 pads с ID=216.
- [x] Пространственный индекс вместо повторных полных обходов препятствий;
  сохранён порядок, room/matrix зазоры, допуск 1 нм и отказ по ошибке правила.
- [x] Windows, 40 001 дорожка: медиана Stop 4,0011 -> 0,0084 мс, Walkaround
  1,9503 -> 0,0056 мс. Подготовка 12,234 -> 27,768 мс. Это CPU solver, не FPS.
- [x] 438 workspace tests, fmt --check, строгий all-target Clippy, Windows build.
  Реальное окно полной ReferenceB: NetC13_2, сегмент, Finish, Undo до чистого документа.
  Исходный PcbDoc не сохранён, SHA256 прежний.
- [ ] Копии/валидация и графика route preview, дорогие расстояния до больших
  заливок, полный язык правил/варианты, shove/gloss и точное совпадение Altium.
  PCB Filter Altium 24.4.1 вернул ошибку скрипта; live query parity не доказан.

Данные и границы: `docs/pcb-update/routing-performance-2026-10-25.json`,
`docs/pcb-update/INTERACTIVE_ROUTING.md`, `docs/pcb-update/PERFORMANCE.md`.

### Предпросмотр трассы без пересборки всей платы, 25.10.2026

- [x] Закрытый валидированный draft RouteSession; повторно используемый immutable
  preview, стабильные ID активных сегментов, проверка новой геометрии при hover.
  Полная валидация place/via/backtrack/History и неизменность удержанных снимков.
- [x] Пропуск одинаковых запросов; отдельный geometry snapshot для connectivity,
  без удержания Arc редактора и повторной полной копии платы на каждом движении.
- [x] Исходное дерево/контуры сохранены, разрезанные дорожки refit; tail меняет
  длину и состав независимо от stationary artwork. Повторное использование
  геометрии исходной платы при входе; чужой preview/revision сбрасывают кэш.
- [x] Общий document anchor live/cached tessellation устраняет сдвиг на пиксель.
  GPU readback linear/sRGB, masks/highlight/draft/mirror/single-layer/via/backtrack:
  <=1 уровень канала без dithering, <=3 с production dithering на перекрытиях.
- [x] ReferenceB, 60 CPU samples: медиана 57,5230 -> 5,7226 мс, p95 64,6967 -> 9,0993.
  Меняющиеся хвосты: 6,3729/9,2021 мс. Первый кадр до 90,7778 мс, 1/60 >33 мс.
  Повторные замеры после source reuse записаны отдельно; это не presented FPS.
- [x] Финальный source reuse: 7,1928/9,6066 мс, максимум 72,3750 мс;
  меняющийся tail: 7,6839/10,3159, максимум 62,3378 мс. Полная перерисовка
  в том же финальном executable: 83,2955/103,6451, максимум 112,6602 мс.
  Разброс между прогонами сохранён в данных; cold frame всё ещё 1/60 >33 мс.
- [x] 449 workspace tests, fmt, strict all-target Clippy, Windows build;
  Linux GNU all-target compilation. Windows GUI: place/Backspace/Finish,
  Undo/Redo/Undo до чистой платы; source PcbDoc не сохранён, SHA256 прежний.
- [ ] Устранение cold artwork/snapping и задержек place/Finish/repour,
  произвольные большие платы/длинные трассы, Linux runtime и полное совпадение
  функционала, поведения и интерфейса Altium остаются в исходном охвате.

Данные: `docs/pcb-update/route-preview-performance-2026-10-25.json`.

### Общий индекс исходной платы для привязки preview, 25.10.2026

- [x] Вместо второго полного display cache хранится Arc пространственного
  дерева исходной платы. Source frame передаёт готовое дерево; refit preview
  отделяет копию и не меняет исходные координаты/индексы привязки.
- [x] Истечение по revision, spatial-only fallback до первого source frame;
  Off пропускает поиск объектов, сохраняя сетку; Ctrl+Shift возвращает raw point.
- [x] Реальные canvas events: All/Current/Off, grid/bypass, cold/warm preview,
  исходный конец дорожки и отсутствие snap к собственному preview. Проверки
  multilayer pad, split refit, удержанных снимков, fallback и смены revision.
- [x] 451 workspace tests, fmt, strict all-target Clippy, Windows build,
  Linux GNU all-target compilation. Windows ReferenceB: Off -> Current, NetC13_2,
  движение/размещение/Finish/Undo до чистого документа, возврат Off.
  PcbDoc не сохранён; SHA256 исходного файла прежний.
- [ ] Дополнительное ускорение полного кадра этим изменением не доказано:
  baseline median/p95/first 4,8551/6,4533/51,1992 мс, после изменения
  6,7688/9,2336/65,6447; varied 7,7865/10,9781/42,9343. Неуправляемые
  частота CPU/background scheduling, разные состояния GUI; все данные сохранены.
  Cold artwork/model, итоговая presentation latency и точность Altium открыты.

Данные: `docs/pcb-update/route-snapping-performance-2026-10-25.json`.

### Повторное использование неподвижного artwork при старте трассы, 25.10.2026

- [x] Группы board/layer/type/masks/details/polygon outlines в прежнем порядке
  рисования. Source capture содержит места для route tail; stale copper и
  разрезанные исходные tracks перестраивают затронутые группы, остальные
  mesh/atlas/GPU scenes разделяются. Revision/style/font/DPI/scale проверяются.
- [x] Камера обновляется и для сохранённой GPU scene без mesh zero; одинаковые
  uniforms повторно не пишутся. CPU refinement remap выполняется один раз на
  общий mesh; weak-reference тест проверяет освобождение старых GPU resources.
- [x] Полный ReferenceB: 151/155 групп сохранены при stale переходе; final artwork
  1,99/2,62 мс. Final combined regular median/p95/first 7,2775/10,5462/37,1903,
  varied 8,1933/10,3123/33,3280 мс. Ранние first 21,6751/22,0493 тоже сохранены;
  частота/background не контролировались, это CPU submissions без GPU/FPS.
- [x] 452 workspace tests, fmt, strict all-target Clippy, Windows build,
  Linux GNU all-target compilation. GPU linear/sRGB, pads/labels/masks/holes,
  split/stale/via/backtrack/highlight/draft/mirror и камера без mesh zero;
  <=1 уровень канала без dither, <=3 с production dither.
- [x] Windows GUI: Fit/refinement, NetC13_2 route motion/place/Backspace/place,
  Finish/Undo до чистой платы. Entry 155/155 (3,52 мс), stale 151/155 (5,39 мс).
  Исходный PcbDoc не сохранён, SHA256 прежний.
- [ ] Finish/repour/Undo и смена глобальных состояний всё ещё строят все группы;
  в native log artwork до 67,69 мс. Cold model/validation, split-группы, другие
  платы, RAM/VRAM, Linux runtime и весь исходный функционал/Altium parity открыты.

Данные: `docs/pcb-update/artwork-groups-performance-2026-10-25.json`.


### 2026-10-26 — продолжение PCB Rules после обновления main

- Продолжено с `9c60e1c`; существующие Wizard/Query Builder и ускорение маршрута
  сохранены. Добавлен рабочий Hole Size Percent: поля режима и сводка, отдельное
  хранение абсолютных/процентных значений, проверки диапазона, общая проверка
  DRC/interactive via, атомарный Apply/Undo, нативный PcbDoc roundtrip.
- База процента для прямоугольных/разных по слоям площадок пока является явно
  отмеченной гипотезой реализации (меньший номинальный размер каждого слоя).
  Без новой сверки в AD полное соответствие этих случаев не заявляется.
- Проверка отверстий кеширует геометрию и отсекает дальние пары; сравнение с
  полным перебором включает повёрнутые пазы, приоритеты и области правил.
- Профилирование исходного ReferenceA выявило реальную причину 49-секундного DRC:
  три полные окружности шелкографии с внутренним радиусом 1 нм. Общий buffer
  строил каждую почти 8 секунд дважды. Для полных окружностей построение заменено
  прямым кольцом/диском с сохранением отверстия и 256 сегментами; частичные дуги
  используют прежний путь. Проверены площадь, границы, очень тонкие кольца и диски.
- ReferenceA: 49.7204 s до -> 1.0234 s после; повтор под нагрузкой Clippy/roundtrip
  2.4197 s. Во всех случаях 335 нарушений с теми же счётчиками по типам,
  22 диагностических сообщения, без усечения. Это не доказательство совпадения
  координат всех маркеров. Roundtrip: 3,331,072 bytes, все поля rules совпали.
- Исходный ReferenceA не записывался; SHA256
  `redacted-source-sha256` сохранён.
- Новые экземпляры PCBCraft не запускались. Проверка настоящего окна заблокирована
  ошибкой Computer Use `window.app must be a non-empty string and window.id must
  be an integer >= 0`, повтор выбора окна не помог. Новая визуальная проверка
  остаётся незавершённой. Подробности и оставшиеся расхождения: `docs/pcb-update/RULES.md`.
- Проверки: fmt и строгий Clippy прошли; workspace — 458 unit tests passed,
  1 failed. Все 7 новых тестов прошли. Сбой существующего GPU-теста
  `routing_tail_matches_full_gpu_pixels_and_keeps_buffers_across_bends_vias_and_backtrack`
  (Rgba8Unorm, frame 13, dither=true: 4 вместо допуска 3) воспроизведён и на
  неизменённом `9c60e1c` в `target/rules-head-validation`. Допуск не ослаблен,
  рендерер не менялся; полностью зелёный workspace-прогон не заявляется.


### 2026-10-26 — продолжение Rules: Layer Pairs

- В AD 24.4.1 на исходном ReferenceA повторно осмотрен Manufacturing → Layer Pairs:
  одно поле области, Constraints и Enforce layer pairs settings, подпись слева
  от checkbox. Пара исходного файла: LAYERPAIR0LOW=TOP / HIGH=BOTTOM.
- Реализованы редактирование ENFORCE, сводка, мастер и Online/Batch-проверка,
  единое разрешение правил для DRC и interactive via. Проверяется физический
  диапазон слоёв с поддержкой обратного направления; размер via — отдельное правило.
- Определения берутся из отредактированного typed stack; без него читаются
  исходные legacy drill pairs. Пустая таблица соответствует существующему default
  Thru в manager. Неизвестные/неполные таблицы и несколько substacks дают явную
  диагностику. Запрещённый переход не меняет трассу и backtrack.
- 5 новых core-тестов прошли. ReferenceA: 335 нарушений, 21 диагностика, 1.0222 s,
  без усечения. ENFORCE roundtrip: 3,331,072 bytes, все rules равны после импорта.
- Windows build и строгий Clippy прошли; полный workspace: 463 pass, 1 existing
  GPU dithering mismatch (тот же исходный сбой 9c60e1c, допуск не менялся).
- В реальном PCBCraft проверены форма, переключение Enforce и Cancel без
  применения изменений. Единственный экземпляр перед обновлением сборки закрыт;
  затем запущен один новый. Исходный ReferenceA не сохранялся.
- Полное соответствие Rules пока не достигнуто: outer-dialog/font/icon metrics,
  native blind/buried via-type presentation/save, multiple substacks и technology
  metadata остаются в RULES.md. Altium modal Cancel дважды дал timeout; дальнейший
  ввод в Altium прекращён. Сама форма успешно осмотрена без редактирования.


## 2026-10-26 — Schematic ↔ PCB / ECO (в работе)

- Наблюдены обе команды Design на исходном ReferenceA в AD 24.4.1. Прямой ECO:
  Add Components (12), Add Pins To Nets (22), отдельные Check/Done, missing
  footprint/pin errors при Validate. Обратный comparator: 47 differences,
  34 предлагаемых ECO, первая группа Remove Pins From Nets (22).
  Execute/Save в исходном Altium не нажимались.
- Добавлен независимый design_sync в eda-core: сравнение, Unique ID, multipart,
  flat net compilation, реальные компоненты/площадки/корпуса, metadata и nets,
  обратные свойства и bijective net rename. Ошибки зависимостей и stale sources
  не подменяются успешными операциями; успешные действия имеют History/Undo.
- App: Design Import/Update, Project Show Differences, выбор платы внутри
  проекта, группировка ECO, Validate/Execute, фильтр ошибок, report. Выполненные
  строки исключены из повторного применения; оставшиеся можно проверить снова.
- Binary native writer дополнен базовыми параметрами компонентов, Unicode и
  source links с обратным чтением; вариантные/системные namespaces сохраняются.
  Пять направленных native parameter tests прошли.
- Первые тесты после добавления description показали неполный fixture; описание
  добавлено в fixture. Тест DisableSync исправлен на реальный скрытый параметр,
  поскольку ParameterValue нового имени создаёт значение без presentation object.
- Охват и незавершённые функции перечислены в docs/pcb-update/DESIGN_SYNC.md,
  feature имеет partial. Native SchDoc writer, библиотеки корпусов, иерархия,
  wire back annotation, classes/rooms/variants и точное UI-соответствие не готовы.
- Итоговые проверки и реальное окно: проверяются ниже после текущей сборки.

- GUI сверка обнаружила сжатие таблицы egui и отсутствующий glyph ✓ в шрифте.
  Исправлены удержание ширин, высота строк, backdrop и рисованные Check/Done.
  В первой проверке успешно выполнены 51 применимая операция в памяти, потом
  один Undo вернул чистый PCB. Исходный native файл не сохранялся.
- Новые компоненты теперь упаковываются по реальным bounds корпусов рядом с
  платой; их положение не зависит от числа уже размещённых элементов схемы.
- Диагностика BQ1 обнаружила 1 nm разрыв: отдельное округление native position
  и pin length. Импортёр теперь округляет абсолютный electrical endpoint один
  раз и выводит из него length_nm. Состав цепей не использует расширенный допуск.
  Тест 4 направлений × 2 знака прошёл; искусственный 1 nm зазор не соединяется.
- После исправления ReferenceA: 60 изменений / 49 применимых, 1.3786 s; Add Components
  (12), Add Pins To Nets (22), ложные Remove Pins C2/C20/BQ1 исчезли. Это не
  равенство всем 47 native comparator differences: настройки сравнения и
  источник footprint geometry пока отличаются. Copy of ReferenceA: 97 / 88, 2.3110 s.

- Итоговая проверка после native endpoint fix: fmt --check и строгий workspace
  all-target Clippy прошли, Windows dev build прошёл. cargo test --workspace:
  488 pass / 1 fail. Единственный сбой — исходный GPU routing pixel test,
  Rgba8Unorm/frame13/dither=true, max channel difference 4 при допуске 3;
  он ранее воспроизведён на неизменённом 9c60e1c. Допуск не ослаблялся.
  Дополнительно workspace doc-tests прошли (0 примеров). Новые ECO/IO тесты
  проходят; лог target/eco-native-endpoints-workspace.txt.
- На последней Windows-сборке проверены обе точки входа: SCH Update с выбором
  PCB и прямой PCB Import, ширины/группы/Check/Done, Only Show Errors и report.
  Исходный PCB SHA256 по-прежнему redacted-source-sha256.
  Один процесс PCBCraft; модальное окно Altium закрыто без Execute/Save.

- На последнем executable после исправления native pin rounding повторены
  Validate/Execute: 49 применимых операций, корректные Check/Done и Report.
  Home показывает новые корпуса рядом с платой; однократный Undo возвращает
  чистый исходный ReferenceA. Последний процесс PCBCraft PID 9212, единственный живой.

- При дополнительной GUI проверке после Undo команда Home/Fit Document дала
  пустой viewport. View → Fit Board восстановила исходную видимую плату, без
  dirty-state. Причина не установлена; внесено в DESIGN_SYNC.md как открытое
  расхождение. Последний оставленный вид — Fit Board, исходный ReferenceA без изменений.

## 2026-10-26 — продолжение ECO: библиотеки корпусов и native SchDoc

- Добавлена запись поддержанных свойств в ASCII/CFB SchDoc: designator/comment,
  description/type, base parameters, текущие/новые модели корпуса, имена меток.
  Исходный контейнер и нетронутые payloads сохраняются; результат повторно
  импортируется и сравнивается со сценой до атомарной замены destination.
  Неизменённый archive возвращается побайтно, неподдерживаемая геометрия и
  неизвестные index dependencies не сохраняются под видом поддержанных.
- Сверка отдельной копии ReferenceA в AD24 поймала ошибку, которую не ловил наш import:
  глобально добавленные записи за старым Weight не были видны в Altium.
  Исправлены Weight по фактическому числу записей, вставка внутри owner tree и
  OWNERINDEX remapping. В AD24 подтверждены новый корпус, Unicode-параметр,
  comment и description WA1. Родной ReferenceA не сохранялся и не выполнялся в Altium.
- Многострочный диагностический параметр показывал `~1` буквально в Properties.
  Пока native encoding не подтверждён, изменённые многострочные параметры
  отклоняются при SchDoc Save; JSON сохраняет их. Это ограничение, не паритет.
- Независимый PCB6 PcbLib/IntLib reader: площадки, слои, stack/masks, графика,
  тексты и доступные встроенные модели. Неизвестный tag/count/encoding не создаёт
  частичный корпус. ReferenceLibraryA: 29 корпусов с pads и 4 явных graphical/no-pad
  отказа. TPS22929 PcbLib и IntLib: по 3 корпуса; STEP assembly transforms
  предупреждаются как неподдержанные. Установка PcbLib проверена через File Open
  в реальном PCBCraft; исходная плата оставалась чистой.
- App ECO использует проектные и установленные локальные библиотеки. Копирование
  footprint из существующего экземпляра больше не скрывает missing-library error.
  На исходном ReferenceA теперь 60 изменений / 26 применимых, 1.0121 s; все 12 missing
  footprint операций ошибочны, как в наблюдённом Altium. Предыдущие 49 применимых
  относились к явному board-reuse эксперименту; CLI оставляет его отдельным флагом.
- В GUI повторены Validate/Execute (26 в памяти), Undo одним действием и настоящий
  Fit Document (Ctrl+PageDown). Плата остаётся видимой и чистой. Прежняя запись
  журнала ошибочно называла Home командой Fit: Home центрирует по указателю.
  Доказательств ошибки bounds/cache после Undo не получено.
- Уточнён обратный native ECO: после Remove Pins From Nets (22) идут Remove
  Component Class Members (12), а не удаление символов. Validate Altium снова дал
  failed-to-activate; Execute не вызывался, окно закрыто Close. Общие свойства/
  net renames, которые умеет core, не выдаются за native back annotation: в app
  они отключены, остаются designator/comment/footprint. Классы, компилированные
  pin changes и swap-механизм пока не восстановлены.
- ECO отображает BQ1…TP9…TP17…WA1 в natural order, не меняя execution dependencies.
  Добавлены проверки native reverse policy, библиотечной зависимости в app,
  owner/Weight/legacy Unicode и фактической ECO-установки геометрии из контейнеров.
  До последнего guard многострочных параметров: 500 pass / 1 прежний GPU fail,
  fmt и строгий Clippy прошли. Финальные результаты последней сборки — ниже.

- Финальный проход после guard, стабильных Unique ID и переноса нерелевантных
  library warnings из ECO в Messages: fmt --check, строгий all-target Clippy и
  Windows build прошли. Workspace: 500 pass / 1 прежний GPU fail (тот же
  Rgba8Unorm/frame13/dither=true/max-channel=4, допуск 3 не менялся).
  Лог target/eco-completed-stage-tests.txt. Повторные SchDoc save сохраняют
  идентичные новые ID/байты; тесты проходят для ASCII и CFB.
  SchDoc SHA256 redacted-source-sha256,
  PcbDoc SHA256 redacted-source-sha256.
  Полное соответствие остаётся partial: Comparator/ECO options, классы/rooms/
  directives/rules/variants, Component Links, hierarchy/buses/hidden pins/swaps,
  полный native save и точная визуальная проверка всех состояний не завершены.

### 2026-10-26 — локальная Windows portable release

- По запросу пользователя собран Windows x64 MSVC release с Cargo.lock.
  Добавлен portable.flag: настройки eframe сохраняются в Data/app.ron рядом
  с исполняемым файлом. Без маркера используется обычное хранилище системы.
- scripts/build-windows-portable.py создаёт ZIP, SHA256, build-info, лицензии
  зависимостей и шрифтов, проверяет PE imports и добавляет redistributable CRT
  из Visual Studio. Системные DLL и графический драйвер не копируются.
  Архив проверяется testzip; пользовательские настройки в архив не включаются.
- Результат: dist/PCBCraft-0.1.0-windows-x64-portable.zip, 12 284 166 байт.
  SHA256 redacted-source-sha256.
- Распакованный exe запущен из другого рабочего каталога (TEMP), единственным
  экземпляром. В реальном окне открыты исходные ReferenceA SchDoc и PcbDoc, после
  штатного закрытия создан локальный Data/app.ron. Документы не сохранялись.
- fmt --check, строгий workspace/all-targets Clippy и release build прошли.
  Workspace tests: 500 pass / 1 прежний GPU fail (routing pixels differ by 4,
  frame13/dither=true). Логи target/windows-portable-*.txt. На чистой Windows
  без инструментов разработки запуск отдельно не проверен.

### 2026-10-26 — зависание при открытии сложных PCB

- Сообщённая пользователем плата находится на другом устройстве и здесь
  недоступна. На локальной ReferenceCRevision2 воспроизведено отдельное зависание
  до первого кадра: robust_triangulation, BOURNS_INDM135125X65N_SRP1265A.STEP,
  shell 2353. Исходный probe потребил более 56 CPU seconds без результата.
- Ограничения исходных edges не покрывали внутренние trimming PCurves.
  Добавлен адаптер поверхностей с общим для модели бюджетом 250 000 вычислений,
  трёхсекундным deadline и ограниченным surface grid. Ошибка модели даёт
  предупреждение и прежнюю body projection, STEP сохраняется в source archive.
  Результаты неудачного импорта также кэшируются; после 15 s модельной библиотеки
  не запускается следующая модель. Это кооперативные ограничения, не изоляция
  произвольного parser/topology кода в отдельном принудительно завершаемом процессе.
- Открытие через File, Recent, Projects tree и аргумент запуска перенесено
  в один worker; очередь ограничена и объединяет одинаковые пути. Окно загрузки
  показывает файл, фазу и время; Cancel останавливает очередь и запрещает
  публикацию результата. До выхода отменённой задачи новый worker не создаётся.
- Реальный portable release: наблюдалось окно ReferenceC на фазе 3D model 20/141,
  2.7 s; Cancel убрал загрузку без публикации частичного документа. Следующий
  клик Recent успешно открыл и отобразил ReferenceA PCB. Приложение штатно закрыто,
  одновременно запускался только один экземпляр. Первый полный ReferenceC GUI import
  вернулся с отдельной ошибкой Invalid PCB Region geometry, окно осталось рабочим.
- Другие крупные локальные файлы выявляют независимые ошибки совместимости:
  padstack land, parameter group owner, PLANE2. Их исправление не имитировалось
  отключением валидации. Подробности и границы проверки в PERFORMANCE.md/features.json.
- Portable ZIP пересобран; Diagnose.cmd включает import/STEP tracing в
  Data/diagnostic.log для проверки на другом устройстве. Настройки/проекты
  пользователя в ZIP не включены. ReferenceC SHA256 остаётся
  redacted-source-sha256,
  ReferenceA PCB SHA256 redacted-source-sha256.
- fmt --check, строгий Clippy и release build прошли; workspace: 506 pass /
  1 прежний GPU fail, тот же routing frame13/dither=true/channel difference 4.
  Проверены реальный Truck PCurve, curved/planar grids, invalid/deadline budgets,
  отмена импорта, очередь и сохранение worker slot. Логи target/complex-pcb-*.
  Точный случай пользователя на другом устройстве остаётся непроверенным.

### 2026-10-26 — ложный Invalid PCB Region geometry на корректных платах

- Уточнена диагностика core: source stream/index, размер внешнего контура и
  индексы/размеры непригодных внутренних контуров вместо одной общей ошибки.
  На ReferenceCRevision2 найден Regions6 record 401: внешний контур 2386 точек,
  71 interior; interior 32 сводится к одной точке (250987781,70407355) nm.
  Та же особенность есть в record 546 и дублирующих ShapeBasedRegions6.
- Нативный декодер исключает из редактируемой геометрии только interiors,
  содержащие менее трёх различных целочисленных nm-точек, с предупреждением.
  Строгая core/JSON проверка не ослаблена; реальные отверстия, в том числе
  треугольник 1 nm, остаются. Неисправные внешние контуры, NaN и усечённые
  блоки по-прежнему отклоняются. Исходные native bytes остаются в archive.
- Два новых CFB regression tests: legacy/modern region families, пустые,
  одноточечные, повторяющиеся и двухточечные interiors, обычное и минимальное
  отверстие. Unmodified native save побайтно равен original; после добавления
  track оба исходных region streams, включая их opaque tails, побайтно сохранены.
- Read-only probes: ReferenceC 3.0.7 — 27202 items / 1116 components / 24 layers,
  11125.64 ms; ReferenceC 3.0.1 — 18128 items / 790 components / 23 layers,
  17676.31 ms под параллельной сборкой, с предупреждением о 3D time budget.
  В pcb_frame_profile добавлен --load-only; это время без первого render.
- Новый portable release открыл ReferenceC 3.0.7 в реальном окне, плата отображается,
  выбор и PageUp zoom работают, документ чистый. Приложение закрыто без Save;
  одновременно был только один экземпляр. SHA256 ReferenceC 3.0.7 остаётся
  redacted-source-sha256.
- fmt --check, строгий Clippy и release build прошли; 508 workspace tests pass /
  1 прежний GPU fail (frame13/dither=true/max-channel=4). Первый test build
  упёрся в Windows file lock запущенного pcb_frame_profile; после завершения
  probe повторный workspace run выполнен. Логи target/region-*.
- ZIP пересобран: SHA256
  redacted-source-sha256.
  Исправлен подтверждённый вариант interiors, а не все возможные native-region
  виды/топология. Иные padstack/layer/parameter ограничения остаются отдельными.

### 2026-10-27 — точный ReferenceGWorkspace/ReferenceGRevision1.PcbDoc

- Воспроизведена ошибка на указанном пользователем файле (21 056 512 bytes):
  Invalid PCB Region geometry, Regions6 record 11, outer vertices=2.
  Read-only разбор исходных streams показал 13 legacy вершин с двумя различными
  x на одном y и повторами, а не повреждённую рамку записи. Modern counterpart
  содержит те же вершины с дополнительным замыканием. Проблема возникала при
  dedup/удалении closing vertex в импортёре.
- Если нормализация внешнего контура оставляет <3 вершин, исходная нативная
  последовательность сохраняется с предупреждением. Core validation размеров
  и координат не ослаблена. is_empty_region проверяет точную коллинеарность;
  только такие записи не дают материал/соединения, вырезы, artwork, picking и
  влияние на Fit. Signed-area=0 не используется как критерий пустоты.
- Regression CFB legacy/modern: byte-identical unmodified Save, точные исходные
  region streams после добавления track. Core: remnant не замыкает разорванную
  цепь, неколлинеарный контур с signed-area cancellation не отбрасывается.
  Render: пустой контур не видим и не выбирается. pcb_regions — bounded
  диагностический example для region framing/координат без импорта STEP.
- Exact read-only import: 8494.63 ms, 20261 items, 1271 components, 26 layers,
  98 meshes. Исходник не менялся: SHA256
  redacted-source-sha256.
- fmt --check, строгий workspace/all-target Clippy, release build проходят.
  Workspace: 511 pass / 1 прежний GPU routing fail (Rgba8Unorm/frame13/
  dither=true/channel difference 4). Логи target/ReferenceG-gaz-*.
- Новый portable ZIP собран и проверен упаковщиком, SHA256
  redacted-source-sha256.
  Работающий старый portable exe не перезаписывался/не перезапускался: в реальном
  окне есть несохранённый Controller.pcbcraft.json, активен ReferenceK.SchDoc. Поэтому
  GUI нового exe на этой плате не проверен; второй экземпляр не запускался.
  Для применения исправления нужен пользовательский перезапуск новой сборки.

### 2026-10-27 — повторный удалённый Region-сбой и аудит PCB-импорта

- Пользователь подтвердил текст Invalid PCB Region geometry, но проблемная плата
  отсутствует на этом компьютере. Точное воспроизведение этого файла невозможно;
  устранение именно его сбоя не объявляется проверенным.
- Короткие нативные внешние контуры с 1/2 вершинами замыкаются повтором исходной
  вершины без создания площади. Полностью пустые Region хранятся как source-only
  записи empty_region_records с исходным CFB в архиве; фиктивная геометрия не
  создаётся. Пустой outer с реальными holes, NaN/Inf, выход за координатные пределы,
  oversized counts и обрезанные payload по-прежнему отклоняются.
- Native Save сохраняет содержащие пустые слоты семейства побайтно, включая
  расположенную после пустого слота обычную region-запись. Правки такого семейства
  и native reindexing блокируются с предложением JSON. Проверены byte-identical
  Save, JSON recovery, посторонняя track-правка и запрет потери пустых записей.
- В локальной подборке из 18 различных по SHA256 файлов до правок импортировались
  14: ReferenceM требовал поддержку UID-only PrimitiveParameters, ReferenceGTest — PLANE2;
  ReferenceD/ReferenceE содержали дуги нулевой толщины. Добавлено однозначное разрешение владельца
  по PRIMITIVEID и сохранение UID-only заголовков без дублей; PLANE1..16; нулевые
  дуги без подмены ширины и без меди/connectivity. Отрицательные ширины запрещены.
- Ошибки примитивов содержат поток/запись/byte offset; ошибки model validation —
  item/layer/source, для padstack также размеры/форма/drill. Загрузка файла
  дополняет сообщение фазой и путём. Логгер включает реальный модуль pcbcraft.
- fmt --check и строгий workspace/all-target Clippy проходят. Workspace tests:
  516 pass / 1 прежний GPU routing fail (frame 13, Rgba8Unorm, dither=true,
  difference 4). Все 98 IO тестов проходят. Логи target/import-audit-*.
- UI не менялся и второй экземпляр не запускался. Работающий portable PID 45684
  оставлен без перезапуска и без перезаписи его executable.
- Итоговый read-only прогон: 18/18 файлов импортированы (до правок 14/18).
  Все SHA256 до/после совпадают. Машиночитаемый отчёт:
  docs/pcb-update/import-compatibility-2026-10-27.json. Прогон выполнялся параллельно
  с release build и не является замером пользовательской задержки открытия.
- Проверка source-only ссылок использует индекс обычных записей, а не повторный
  перебор всех items для каждой пустой записи. Финальные fmt/Clippy/workspace
  проверки после этой правки повторены; 516 pass и тот же GPU fail.
- Portable release собран и CRC/SHA проверены. Отдельная копия архива:
  PCBCraft-0.1.0-windows-x64-portable-2026-10-27-import-fix.zip,
  12 317 060 bytes, SHA256
  redacted-source-sha256.
  build-info.json включает UTC-время сборки и SHA256 exe. Старый запущенный
  экземпляр не заменялся; пользователю требуется открыть новый ZIP отдельно.

### 2026-10-27 — только тёмная тема интерфейса

- Отдельной светлой палитры и переключателя в приложении нет. Однако egui по
  умолчанию следует теме ОС, а set_visuals меняет только текущий стиль.
- theme::apply теперь явно выбирает Theme::Dark до настройки палитры/типографики.
  Это применяется при запуске и Restore Default Layout и исключает автоматическое
  переключение интерфейса вслед за светлой темой ОС.
- Внешний вид существующей тёмной палитры не менялся; работающий экземпляр с
  пользовательскими документами не перезапускался. Portable ZIP этим изменением
  не пересобирался.
- Проверки: fmt и строгий Clippy проходят; workspace tests — 516 pass / 1 прежний GPU routing fail (difference 4 при допуске 3).

### 2026-10-27 — иерархическая схема ReferenceG, продолжаемый этап

- Перед началом изменения импорта/portable/темы закоммичены: 68eb55e. Новые
  изменения иерархии остаются в рабочем дереве; отдельного push пользователь
  в этом запросе не поручал.
- Источник: ReferenceGWorkspace/ReferenceG.PrjPcb. Импортированы все 9 SchDoc, верхний
  Main содержит 8 Sheet Symbols, 410 Entries и 410 wires; дочерние листы —
  410 Ports. Keys ранее отклонялся из-за четырёх 13-точечных Bezier: теперь
  поддержаны цепочки кубических сегментов 3n+1.
- Sheet Symbol/Entry получили типизированную модель и parent identity,
  принадлежащие подписи имени/файла, корректные единицы DistanceFromTop
  (100 mil, _FRAC1 / 1e6), однократное округление абсолютной координаты.
  Перемещение/resize/редактирование имени, групповое копирование/удаление,
  перетаскивание с проводами проходят через core History.
- Native SchDoc writer сохраняет поддержанные изменения существующих Symbol,
  Entry, подписей и wires; проверяет parent ID и точные связи после повторного
  импорта. Добавлены ASCII/CFB roundtrip, Unicode filename, group move/delete
  и copy identity regression tests. Вставка новых native hierarchy пока не
  поддержана; размещение и синхронизация Entries отключены.
- Навигация Tools > Up/Down Hierarchy и Ctrl+double-click связывает блок/Entry
  с дочерним листом/Port и Port с родительским Entry. Используется только
  текущий проект, относительный путь владельца, загружаются только необходимые
  листы. Неоднозначные экземпляры отклоняются явно. PrjPcbStructure задаёт
  исходные номера листов в Projects и вкладках, Main открывается первым.
- 27 октября Computer Use восстановился после остановки по Escape. На живом
  Altium 24.4.1 просмотрены U_Connector и Entry KL30, 4 формы Kind; Up/Down
  Hierarchy открыл Connector.SchDoc на KL30. В PCBCraft проверено отображение
  Main (все 8 блоков), выбор блока и переход Entry ACC → Connector Port ACC.
  По наблюдениям исправляются Properties и порядок меню; Parameters Symbol,
  неотрисованные формы Kind, Device/Managed и прочие неподдержанные действия
  показаны отключёнными. Оригиналы SchDoc в Altium не изменялись.
- Компилятор соединяет Entry/Port по явному owner/имени; повторные локальные
  designators разных листов больше не замыкают pins между собой. Тест с двумя
  дочерними R1 и одинаковыми именами локальных Ports проверяет разделение.
  Multichannel/buses/harness не объявлены поддержанными.
- Полная электрическая проверка ReferenceG пока не завершена: compile сообщает
  конфликт GND/1V8; 68 Entries не совпадают точно с концами wires (например
  ширина блока даёт x=199999600 nm против wire x=200000006 nm). Автоматического
  объединения по близости нет. Полное соответствие Altium не заявляется.

- Дополнительный read-only trace нашёл путь ложного GND/1V8 объединения:
  ReferenceJ record 24078 GND → wire 27710 → C386-1 record 24353 → другой
  C386-1 record 22702 → wire 27585 → 1V8 record 22759. Это два отдельных
  односекционных компонента с одинаковым локальным designator. Компилятор
  теперь явно отклоняет такую неоднозначность до объединения pins; отдельный
  regression проверяет отказ и сохранение поддержки настоящих разных parts.
  Полная compiled annotation/Component Links для этого проекта ещё не восстановлена.
- Первый полный прогон после навигации: 520 pass / тот же прежний GPU routing
  failure (Rgba8Unorm frame 13, dither=true, difference 4). После проверки
  дубликатов и исправления ширины таблицы Sheet Entries выполняется повторный
  финальный прогон. Строгий Clippy и fmt проходят.

- Финальный повтор: fmt --check и строгий workspace/all-target Clippy проходят;
  cargo test --workspace — 521 pass, 1 прежний GPU fail с теми же параметрами.
  Dev executable собран; Computer Use проверил в финальном окне Main [1],
  U_Connector Properties, исправленные колонки Entries и выбор KL30 из таблицы.
  Новая сборка оставлена в одном экземпляре, без записи исходного проекта.
  Логи: target/hierarchy-{clippy,workspace-tests,build,navigation-tests,trace}.txt.

- Последняя живая сверка дочернего Connector/KL30 обнаружила старый Port bug:
  Output рисовался вправо вместо влево, Unspecified имел стрелку, высота 2.5 mm
  подменялась 2.54 mm, игнорировались BorderWidth/Alignment/fill. Исправлены
  импорт и отрисовка; Alignment показан в Properties вместо неподходящего Style.
  Учитываются обе концевые точки Port в connectivity RTree, cross-probe,
  wire handles/junctions, snapping и drag; I/O type не меняет электрические точки.
  Разные типы net identifiers не соединяются только по одинаковому имени.
  Официальная справка (поведение I/O и Direction, net scope):
  https://www.altium.com/documentation/altium-designer/schematic/multi-sheet-hierarchical-designs
  https://www.altium.com/documentation/altium-designer/schematic/creating-circuit-connectivity
- После Port: fmt --check и строгий Clippy проходят. Полный workspace прогон:
  523 pass / один прежний GPU routing fail (difference 4). Native KL30 far-end
  roundtrip и горизонтальный/вертикальный Port snap/drag со всеми I/O проходят.
  Port Direction preference, legacy styles, полные project naming/net-scope
  настройки и локализация power ports остаются отдельными расхождениями.

- В окне финальной сборки повторен Entry KL30 → Connector/KL30: открылись
  только Main [1] и Connector [7]; стрелка Output направлена влево, Gnd без
  стрелки, Properties показывает 12.5 × 2.5 mm, Small border, Right alignment.
  Повторный read-only ReferenceG probe теперь останавливается на первом дубликате VD12
  в ReferenceL.SchDoc до расчёта связей (в ReferenceJ также есть ранее найденный C386).
  Все 9 листов загружаются; исходные документы не записывались.

### 2026-10-27 — Synchronize Sheet Entries and Ports

- Эталон: живой AD Enterprise 24.4.1, Design > Synchronize Sheet Entries
  and Ports и all-matched состояние исходного ReferenceG. Диалог 864×511, три списка,
  вкладки блоков, фильтр unmatched, направленный перенос, unlink, add/delete
  и немедленный Undo в изменённом документе. Поведение дополнительно сверено
  с официальной Multi-sheet & Hierarchical Designs (Synchronizing the Ports
  and Sheet Entries):
  https://www.altium.com/documentation/altium-designer/schematic/multi-sheet-hierarchical-designs
- eda-core сравнивает Name/I/O Type/Harness Type, готовит транзакции переноса
  без изменения геометрии существующего объекта и проводов; размещает
  отсутствующие Ports и Entries группой на выбранной стороне владельца.
- Приложение: весь проект или один Sheet Symbol, три списка и вкладки,
  Ctrl/Shift/drag/Select All, unlink/relink, перенос в обе стороны, add/delete.
  Preview не меняет документ; click/Enter размещает, Esc/right-click отменяет
  и возвращает диалог. История отдельная для каждого листа; session/revision
  guards отклоняют устаревшее размещение. При добавлении Entries камера
  фокусируется на нужном блоке. Harness Type редактируется в Properties.
- Native SchDoc: новые записи 16/18, существующие Name/IO/Harness и геометрия
  Port, удаление, tree insertion/Weight/owner remap, стабильный UID при повторном
  Save и rename без перезагрузки source archive. Text frames 209 и Additional
  drawing regions 225 сохраняются; неизвестные index dependencies по-прежнему
  блокируют небезопасную запись. Проверка точных электрических касаний
  выполняется до допустимой нормализации округления координат.
- Исходный ReferenceG: все 410 Entry/Port пар совпали. В отдельном каталоге
  target/entry-sync-qa-20261027-a сохранены копии девяти SchDoc. В живом PCBCraft
  Add Ports создал QA_ENTRY_ONLY, P → E согласовал KL30 (Output → Input),
  Add Sheet Entries создал QA_PORT_ONLY на U_Connector. Диалог показал all
  matched; Save All записал обе копии. Свежий импорт с диска подтвердил
  412 пар / 0 unmatched. Исходные пользовательские SchDoc не записывались.
- GUI QA обнаружила раздувание Links по горизонтали, конфликт Esc и deadlock
  context input/memory при открытии подменю. Исправлены вертикальная компоновка,
  обработка Esc и получение состояния popup до input closure; регрессии
  проверяют длинный список, Esc, клики подменю, перенос в обе стороны,
  unlink/relink, отмену, stale guards и Undo. Native тесты также проверяют
  ASCII/CFB/Unicode, insert/update/delete и сохранность Additional payload.
- Полное визуальное совпадение не объявляется: остаются native icons,
  типографика и изменение ширины колонок. Live Altium reopen этой QA копии
  не подтверждён: файловый диалог возвращал тайм-ауты activate_window/click;
  после recovery дальнейший ввод в Altium остановлен. Linux не проверялся.
  Metadata Harness Type не означает готовность bus/harness compilation.
- Финальные проверки синхронизации: cargo fmt --all -- --check и строгий
  workspace/all-target Clippy проходят. cargo test --workspace --no-fail-fast:
  534 pass / один прежний GPU routing fail (Rgba8Unorm, frame 13, dither=true,
  difference 4). Все 6 новых app interaction tests, 3 core sync tests и
  11 native writer tests проходят. Dev executable собран.
  Логи: target/entry-sync-{fmt,clippy,workspace-tests,build,reopened-qa}.txt.
- Последующая живая проверка: контекстный вызов одного U_Connector,
  отображение его matched-списка, Esc и глобальные 8 вкладок работают;
  исходный Main остаётся без изменений. Быстрое протягивание дополнено
  обработкой press/move/release в одном кадре; один interaction test
  проверяет как объединённые, так и раздельные события.

- После закрытия блокирующего файлового окна Computer Use восстановился.
  AD24.4.1 открыл SheetSync_QA.PrjPcb и обе сохранённые схемы: all matched,
  8 вкладок; U_Connector/KL30 — Input. В Links присутствуют QA_ENTRY_ONLY и
  QA_PORT_ONLY, оба Input. Новый Entry виден на Main, новый Port — на Connector
  в 70 / 122.5 mm. Это живая проверка native reopen, не только наш reimport.
- По живому диалогу уточнены 864×511, размеры/отступы трёх списков, строки
  17 px, кнопки переноса и вкладки. Колонки теперь изменяют ширину;
  interaction regression проверяет resize без случайного выбора строки.
  Иконки нарисованы собственными векторами; точное растровое совпадение
  и все настройки размещения не подтверждены.
- Properties созданного Port в Altium подтвердил ALIGNMENT=0 как Center
  (1 — Left, 2 — Right). Исправлены renderer и Properties. TEXTCOLOR теперь
  независим от COLOR: импорт отсутствующего поля даёт native black,
  новые синхронизированные Ports получают красную подпись, native save
  сохраняет отдельный цвет. ASCII/CFB regression проверяет раздельные
  изменения подписи/контура и выравнивание. FONTID=2 и SYSTEMFONT=2 в копии
  корректно ссылаются на Altium GOST Type A; менять таблицу шрифтов не нужно.
  В живом Altium Font combo пуст и у исходного KL30, поэтому это состояние
  не свидетельствует об ошибке FONTID созданного Port.
- Повторная проверка после правок Port: 536 unit tests проходят; остаётся
  единственный прежний GPU routing/dither fail (frame 13, difference 4 при
  допуске 3). Допуск не изменён. Новый renderer regression проверяет фактические
  позиции подписей Left/Center/Right и независимый цвет текста.
- Окно новой сборки проверено на исходном ReferenceG: диалог 864×511, all matched,
  затем все восемь вкладок и заполненные Links. Быстрое CU-перетаскивание
  разделителя выявило потерю изменения ширины при объединённых событиях;
  добавлено отслеживание исходного press и последней координаты, тест
  расширен на press/move/release в одном кадре. Документы остаются чистыми.
- Финальная Windows-сборка: быстрый CU drag границы Links с x1242 на x1260
  действительно изменяет ширину, строки не выбираются. Fmt и строгий Clippy
  проходят, workspace сохраняет 536 pass / один прежний GPU fail. Запущен один
  PCBCraft на исходном ReferenceG, пользовательские схемы не записывались.


### 2026-10-27 — hierarchical schematic/PCB ECO on ReferenceG

- Implemented: removed the app's blanket hierarchy rejection. Current editable
  Sheet Symbol filenames resolve relative to their owners; full symbol/component
  UID paths and sheet/component pin identities prevent cross-sheet collisions.
  Duplicate/unannotated source can be reviewed without losing stable linked
  components; unsupported source nets are protected from silent destructive edits.
- Implemented: project scope/naming flags, Entry/Port union, localized power,
  multipart logical reference folding, native Compile Masks (expanded/collapsed,
  fully enclosed objects and strict mask boundaries), legacy/modern component
  kinds, preserved PCB Layer metadata, native ComponentLink URL `=`/`{}` codec.
  Native nonzero HierarchyMode and NameNetsHierarchically=1 are explicitly disabled
  pending observation; repeated sheet/multichannel instances and bus/harness/hidden
  pins remain unsupported. A masked Sheet Symbol requires whole-child exclusion
  and is diagnosed, not partially compiled.
- Observed: native ReferenceG fractional hotspots differ from attached wires by 10–1580 nm.
  Implemented: project compiler-only 2540 nm hotspot-to-segment tolerance; drawing
  and native source geometry stay unchanged, bare interior crossings stay separate.
  This is a corpus-calibrated compatibility rule, not a proven universal AD constant.
- Observed: AD24.4.1 exported the original ReferenceG project as Protel netlist,
  478 distinct names and 2647 pin memberships. Independent compilation matches
  all memberships/names ignoring case, with no missing native pins. Lowercase
  name ordering resolves TR_WK vs TRANSP_WK and Uni_*_PU_* aliases. Root-sheet
  spelling is deterministic for equal folded names. Exact case is audited below.
- Observed: the native ECO displayed four Add Rooms only, with no Execute Changes.
  The exported netlist omits single-pin HSM_OE/Shutdown present in the PCB. Exact
  native ECO/comparator-option agreement is therefore NOT established. The app
  retains explicit review/selection; unrelated floating source pins missing in
  the footprint no longer produce a spurious Unknown Pin disconnect.
- Implemented: scalar ECO operations preflight then mutate private staged state;
  geometric edits clone only affected documents. Final full validation and History
  guards remain. Original ReferenceG comparison+validation fell from minutes of per-row
  full-project clones to about 1.8 seconds in the dev profile.
- Verified: isolated target/hierarchy-eco-qa-20261027-c. DD1 unlocked only in the
  in-memory QA copy; forward comment + TCK→QA_TCK, reverse child-sheet comment,
  unchanged nontext PCB items/copper and net index, exact Undo/Redo, all nine native
  SchDoc saves/reimports and native PcbDoc save/reimport. Source project hashes
  checked independently. Regression tests cover identities, masks/boundaries,
  power scope, naming, tolerance, parameter namespaces, kind versions and staging.
- GUI limitation: later CU activation failed (`failed to activate captured window`);
  fresh captures of Altium and PCBCraft were black. Asked the user to unlock the
  session/close the old PCBCraft. A separate dev executable was prepared in
  target/hierarchy-qa/pcbcraft.exe without launching another instance. Final live
  window verification remains pending. No native source project was saved.
- Final check results and reference audit follow below.

- Safety follow-up: original ReferenceG's remaining four single-pin-removal rows are
  explicitly unavailable in project review, with the native ECO uncertainty
  explained. Export omission is not treated as authorization to disconnect PCB
  pins. A regression ensures no changed document is staged from such rows.
- Reference spelling audit: 79 memberships across 3V3_MCU/PROG/MCU_ON still use
  different case from Altium's export; all 478 names/2647 memberships agree
  case-insensitively. No case-only PCB pin update is generated. Exact native
  capitalization selection is not claimed.

- Final verification: fmt check and strict workspace/all-target Clippy PASS.
  Full workspace: 554 unit tests PASS; one previously existing GPU routing
  Rgba8Unorm frame-13/dither failure remains (pixel difference 4, tolerance 3).
  No tolerance in that test was changed. Original ReferenceG: 4 review-only unavailable
  single-pin rows, 0 applicable destructive operations, compare+validate ~1.8 s.
  Final reference audit: 2647/2647 memberships, 478/478 names ignoring case,
  no missing reference pins. All 11 original source SHA256 hashes unchanged.
  Native QA roundtrip PASS in target/hierarchy-eco-qa-20261027-c.
  Latest dev executable: target/hierarchy-qa/pcbcraft.exe (built successfully
  with an alternate rustc output path because the old debug executable is running).
  Logs: target/hierarchy-{fmt-final,clippy-final,workspace-final,build-final,
  net-audit-final,reference-audit,roundtrip}.txt.


### 2026-10-28 — resumed live hierarchy ECO verification

- Windows session became accessible. The old debug PCBCraft had unsaved Main
  changes; saved them through File/Save As to
  target/session-recovery-20261028/Main-before-restart.pcbcraft.json. The source
  archive and all 410 Sheet Entries are retained. Original 11 source files still
  match their prior SHA256 hashes. Closed the old app cleanly after that save.
- sky.launch_app twice failed before starting a process (`accessibility
  window-opened handler did not become ready`). A normal development-process
  launch started target/hierarchy-qa/pcbcraft.exe with isolated QA project
  target/hierarchy-eco-qa-20261027-c/ReferenceG.PrjPcb, after checking zero live instances.
  Exactly one new PCBCraft is running, initially titled ReferenceK.SchDoc.
- After launch, capture/activation failed twice despite refreshing selection:
  `foreground window did not report a process id`. Asked the user to foreground
  PCBCraft manually. No automated inputs followed the failed recovery. Final live
  ECO execution/reopen is still pending; prior core/native QA checks remain valid.


### 2026-10-28 — live hierarchical ECO and native component preservation

- Resetting the Computer Use JS kernel recovered capture after manual
  foregrounding. On isolated QA ReferenceG-c, all nine sheets and PCB loaded through
  Design/Update PCB. Baseline ECO showed four explicitly unavailable single-pin
  removal rows. No baseline destructive operation was executable.
- GUI forward QA: MCU/DD1 Comment -> GUI Forward Hierarchy QA; Validate/Execute
  succeeded. Tools/Cross Probe located PCB DD1 and Properties confirmed the value.
  GUI reverse QA: PCB DD1 Comment -> GUI Reverse Hierarchy QA; Design/Update
  Schematics produced exactly one enabled comment row targeting ReferenceK.SchDoc.
  Validate/Execute, child-sheet Properties, menu Undo and Ctrl+Y all confirmed it.
  Other eight sheets stayed clean. Save All wrote QA native files; fresh comparison
  returned the same four unavailable baseline rows. AD24.4.1 opened both files
  and showed the changed DD1 comment in both editors.
- Native reopening exposed a persistence defect: Components6 rewrote equal
  fields on unrelated edits. Cyrillic legacy fields became numeric HTML entities,
  and numeric spellings changed; AD then showed corrupt library metadata,
  rotation 0 and height 0 on DD1. Fix: delta-patch only semantically changed
  component fields, leaving unchanged native bytes and ID spelling intact.
  Added a binary regression for a comment ECO and unrelated track edit, including
  scientific notation, mil units, legacy Cyrillic, Unicode shadows and duplicates.
- New original-source QA roundtrip in target/hierarchy-eco-qa-20261028-d PASS:
  forward comment/net rename, reverse comment to correct MCU child, native
  save/reimport and exact Undo/Redo. Independent raw CFB audit: 1270/1271 component
  blocks byte-identical; DD1 changes only intentional LOCKED and SOURCECOMMENT.
  The entire remaining DD1 byte sequence is preserved, not merely decoded values.
- AD24.4.1 reopened fixed QA-d: DD1 Comment = PCBCraft reverse hierarchy QA;
  Rotation = 270, Height = 1.6mm, Design Item ID = 5696, Source = DB.DbLib;
  Cyrillic footprint and library path plus description now display correctly.
  This does not verify all newly changed Unicode/numeric native values.
- fmt check and strict workspace/all-target Clippy PASS. Workspace unit tests:
  555 PASS, 1 existing GPU routing dither mismatch (Rgba8Unorm, frame 13,
  difference 4 versus tolerance 3). Tolerance unchanged. Latest build succeeded
  at target/hierarchy-qa/pcbcraft.exe; old app closed cleanly before launching
  exactly one replacement on QA-d. Logs: target/hierarchy-{fmt-gui,clippy-gui,
  workspace-gui,roundtrip-gui,build-gui,native-fields-fixed,gui-save-audit}.txt.
- Remaining responsiveness issue: native Save All blocked GUI for about 20 s
  while PCB serialization/verification ran, then recovered with clean documents.
  Native comparator single-pin/rooms parity, exact capitalization, multichannel,
  buses/harnesses, nondefault scopes and all other listed limits remain open.
- Final fresh-window check: replacement PCBCraft PID 35832 loaded QA-d MCU
  cleanly and responds; exactly one live instance. Altium also shows the reverse
  comment on QA-d MCU/DD1. All 11 original SCH/PCB/PrjPcb SHA256 hashes remain
  unchanged after both native reopen checks. No commit or push performed.

### 2026-10-31 — PCB routing startup and Altium controls

- Inspected the running Altium Designer Enterprise 24.4.1 at 1707 px width:
  Route menu, floating Active Bar, Interactive Routing Properties and Tab pause.
  Used the existing research PCB copy; the user project was not edited/saved.
  Full visual and solver parity is not accepted by this observation.
- Fixed P,T interception by the Cross Probe prefix handler; full-dispatcher
  tests cover P,T, Ctrl+W, T,C and explicit tool changes. Restored the floating
  PCB Active Bar with working routing/placement commands and the observed menu
  order. Unsupported engineering commands remain disabled.
- No-net routing no longer obstructs itself at starting pads/tracks or at its
  own copper after a layer change. Only contacted terminals/session additions
  may merge; unrelated copper, drills and keepouts remain obstacles. Terminal
  contacts roll back with checkpoints; existing plated pads can provide a layer
  connection. Direct hits beat nearby pads; drill-only holes cannot be targets.
- Added visible-layer startup and prevented previous-net inheritance on a new
  free-space route. Reworked Net Information/via/conflict sections, Width focus,
  pause overlay and one-Escape resume. A real-window layout failure was caught
  and fixed; height/focus regressions protect these controls. Route status now
  exposes a clearance blockage instead of an unexplained missing leg.
- Windows GUI on the generated isolated fixture: two No Net pads connected via
  an intermediate corner, automatic finish and clean Undo. That ASCII fixture
  was not recognized by Altium and is not a native compatibility validation.
- Final Windows GUI on target/routing-check/ReferenceBRouting.PcbDoc (15,513 items):
  Ctrl+W, ExampleGround SMD start, approximately 0.996 mm placed copper, Tab/Width,
  Escape resume/finish, Undo/Redo/Undo to a clean title. Source research copy and
  GUI copy SHA256 both remain
  redacted-source-sha256.
  Altium returned to the original ReferenceB document; no PCB source was saved.
- Required gates PASS: fmt --all -- --check; workspace/all-target Clippy with
  -D warnings; all 563 workspace unit tests; Windows dev build. Test output:
  target/routing-check/workspace-tests.txt. Updated docs/features.json and
  docs/pcb-update/INTERACTIVE_ROUTING.md, keeping routing partial.
- Remaining scope: native save of this route not newly verified; independent
  bounded walkaround/stop and existing native I/O guards remain. Push/HugNPush,
  arc corners, gloss, loop removal, auto-completion, favorites, exact icon/DPI
  parity and a new Linux GUI comparison are unfinished. No commit/push.

### 2026-10-31 — follow-up: track absent after terminal selection

- User clarified that routing enters but no track appears after choosing a pad.
  Rechecked the earlier ExampleGround via failure instead of treating its clearance
  status as a valid constraint. Native AD24.4.1 research copy routes from the
  corresponding terminal; the Rust probe initially produced zero segments in
  all three sampled directions.
- Located false blockers at Regions6/ShapeBasedRegions6 record 17: fragment NET
  is absent, polygon index 1 supplies ExampleGround. Connectivity inherited that net,
  but routing compared the raw empty fragment NET and blocked itself at startup.
- Added shared Board::item_net, used by routing, clearance net scope,
  InNet/InNetClass and connectivity. Explicit fragment nets retain priority;
  native primitive data stays unchanged. Shelved pour fragments do not obstruct.
  Three regressions cover own/foreign/shelved pours, explicit overrides,
  keepouts, via/backtrack and net-scoped/class-scoped rule resolution.
- Read-only pcb_route_profile now accepts --at X Y --blockers. At native via
  (166.180275,199.690002) mm it changes from zero legs to reaching (+1,+1) and
  (+3,0) mm, with preview/place/finish/Undo and no source write.
- Final Windows GUI on ReferenceBRouting.PcbDoc: Ctrl+W, that same via, cursor movement
  without a placement click produces a departing preview; click places and
  completes the connection, Undo returns a clean title. Original research copy
  and GUI copy still have the prior 7C337B5A...CB08B40F SHA256. Altium original
  ReferenceB tab restored; research experiments were not saved.
- fmt, strict workspace/all-target Clippy, all 566 workspace tests and Windows
  dev build PASS. Registry/docs updated. Full routing parity/native route save
  remains unverified as above; no commit/push.

### 2026-10-31 — schematic signal harnesses

- Observed the running Altium Enterprise 24.4.1 Place/Harness menu, connector,
  entry, Signal Harness and Predefined Connector Properties/dialogs. Created
  isolated HarnessReference.SchDoc; the user's original project was not edited
  or saved. Native Additional records establish IDs 215-218 and local owner
  indexing, including omitted zero owners and the 100mil entry-distance scale.
- Added core Connector/Entry geometry and ownership, constrained entry drag,
  same-carrier connected drag, transforms/copy/delete and validated history.
  Signal Harness, Bus, Bus Entry, Port and Sheet Entry placement is enabled.
  Harness Properties, entry management/fonts, Predefined reuse with optional
  line/port, Tab pause, 90/45/any-angle paths, backtrack and Esc repeat work.
- Project compilation expands independent leaf channels through physical paths,
  scoped ports and unique-instance hierarchy. Nested harnesses, numeric bus
  bits/entries, harness-label net names and connector-free LOCKED definitions
  are supported; cycles/type/definition/bit conflicts are explicit diagnostics.
  Local cross-probing follows channel joins. No external EDA execution engine.
- Native preserving writer supports harness insert/edit/delete, hidden type and
  font changes, owner remaps across FileHeader/Additional, attachment verification
  and repeated saves. Sibling .Harness generation preserves LOCKED definitions
  and native nested syntax. General authored Port/wire/label writes support the
  reused group; unknown native index dependencies still refuse unsafe Save.
- GUI QA caught issues missed by the first tests: Properties/Add used a distinct
  authored sync default; writing DISABLESYNC fixes native Save. Power Port
  SHOWNETNAME remains separate from parameter SHOWNAME. The Predefined dialog's
  two columns require explicit top-down child layouts; a bounded-size regression
  now catches the overflow. Dirty graphical definitions supersede stale unlocked
  sidecars in the reuse dialog.
- Final Windows GUI on target/routing-check/HarnessGui.SchDoc: Add entry 0,
  Save, Predefined Serial + three entries + 500mil line + HarnessPort, one-step
  Undo/Redo and Save. Altium reopened the saved file and visibly displays the new
  entry, all three connectors and HarnessPort. No save was performed in Altium
  after reopening. Final QA file SHA256:
  redacted-source-sha256
  (the final group version is retained on disk).
- Required gates PASS: cargo fmt --all -- --check; cargo clippy --workspace
  --all-targets -- -D warnings; all 585 workspace unit tests. Linux GNU
  all-target check PASS; runtime not checked. Test log is
  target/routing-check/harness-workspace-tests.txt. Windows binary linked under
  target/debug/deps; root copy fails because the old pcbcraft.exe is running
  with unsaved user PCB edits. Preserved it, launched a separately named final
  binary, and closed intermediate harness QA windows without saving their drafts.
- Full Altium parity is still partial: Auto Wire/look-ahead preferences,
  classes/multichannel sheets, Device Sheet folders, Active Bar hold menus and
  context actions, exact brace/selection/table/font/DPI rendering. Native creation
  without a source archive remains unsupported. See docs/schematic-harnesses.md
  and schematic.signal_harnesses in docs/features.json. No commit or push.

### 2026-10-31 — follow-up: route across polygon pours of other nets

- Previous same-net inheritance fix was incomplete: foreign generated pour
  copper still blocked routing. Managed region/fill/track/arc fragments now
  allow routing, while free copper, orphaned references, keepouts and board
  cutouts retain obstacle handling. Committed routing invalidates derived
  copper and the existing background worker repours around the new geometry.
- Regression verifies Stop/Walkaround, preview/place/via/backtrack, actual
  0.6mm scoped polygon clearance after repour and atomic Undo/Redo. All 567
  workspace tests, fmt check and strict workspace/all-target Clippy pass.
- AD24.4.1 research-copy observation: HugNPush permits a free-space preview
  across the filled area, cancelled without saving. This was N/A net, not a
  pad-start comparison. No original Altium project edit/save.
- New Windows binary linked successfully under target/debug/deps; Cargo's
  final root copy failed because the old executable is running. Copied the
  newly linked artifact to target/routing-check/pcbcraft-polygons.exe and
  launched separately, preserving PID26472 with unsaved user edits.
- Actual new-build ReferenceBRouting GUI: ECALL_SW via -> foreground polygon area,
  preview visible, placement succeeds, Escape triggers automatic clearance
  repour; Undo restores clean title. Full parity/native save remain unverified.

### 2026-11-01 — schematic Active Bar restoration (partial)

- Reference screenshot establishes fourteen groups and the separate Arcs and
  Drawing buttons. Installed AD24.4.1 advsch.rcs TopBarSchematic establishes
  command captions/order, separators and power-port presets. Official vendor
  documentation establishes short click, one-second hold, right-click and
  last-used defaults. The resource's single Drawing group differs from the
  screenshot; the screenshot takes precedence. No Altium engine/code is used.
- Replaced the schematic placeholder strip with its own 476 by 33 1/3 point
  layout, fourteen original vector buttons, real hold/right-click menus, shared
  last-used defaults and Selection Filter. The 150% DPI interpretation remains
  an inference; exact visual equivalence has not been measured.
- Added core region/connection selection, edge/center/grid alignment and
  distribution, complete-owner stack ordering, Sheet Symbol flips and Sheet
  Entry operations. GUI actions use validated atomic history; offset and
  combined alignment dialogs guard document revisions. Existing Move/Drag,
  Components, Nets, Power Ports and Harness workflows are connected to the bar.
- Added graphical placement for rectangle/rounded rectangle, circular and
  elliptical arcs/circles, polygons, chained Bezier, Text Frame/Note, Sheet
  Symbols with owned texts and Generic No ERC. Escape/cancel, backtrack,
  rotation, repeated placement and modal canvas blocking are covered.
- Expanded the preserving native writer for these authored objects and owner
  groups. Stack changes survive serialization/reimport rather than being
  silently normalized away. Unknown native index dependencies and cross-stream
  stack changes still refuse save. Native verification found and resolved
  coordinate quantization and owned text presentation mismatches.
- Device Sheet Symbol, Reuse Block, Off Sheet Connector, Parameter Set,
  Differential Pair, Blanket, Compile Mask and Graphic import stay disabled.
  Text/Bezier bounds, some region selection, arc placement phases and graphic
  editing/preferences still differ or require AD24.4.1 observation. Full scope
  and remaining work: docs/interface/schematic-active-bar.md and features.json.
- Computer Use is blocked: @oai/sky and cua entry points report a missing
  Windows native pipe, os error 2. Retry, reconnect and tool reset did not
  recover it. No live Altium menu inspection or final Windows window check is
  claimed. A complete T3 Code restart was suggested as a diagnostic step, not
  a confirmed recovery procedure. Existing user windows were preserved.
- Required gates PASS: cargo fmt --all -- --check; cargo clippy --workspace
  --all-targets -- -D warnings; all 595 workspace unit tests. Linux GNU
  all-targets cargo check PASS; runtime unverified. Tests cover actual egui
  short click, hold, right-click, remembered defaults, filters, shape keys and
  validated history; native graphic/owner/power/stack regressions also pass.
  Headless egui mesh rendering produces
  target/routing-check/active-bar-panel.png; this is not a live GUI screenshot.
  Logs are in target/routing-check/active-bar-*.txt. Windows dev build PASS,
  target/debug/pcbcraft.exe updated successfully. No commit/push.
- User-requested Computer Use recheck after plugin update to 26.1027.30824:
  cua.getState still reports missing native pipe, os error 2, and no apps or
  browsers. Fresh node_repl reset followed by @oai/sky initialization fails
  before executing JavaScript: failed to write kernel assets, os error 3.
  Repeated initialization has the same result. Both live inspection paths
  remain blocked; no screenshots or app input were obtained.
- Later user-requested reconnect test succeeded: @oai/sky list_windows returned
  the running Altium 24.4.1 window. Window activation, screenshot capture,
  switching to HarnessGui.SchDoc, right-click opening Nets and Escape closing
  the menu all succeeded without editing the schematic. Live toolbar is about
  476 by 33 logical points at the observed 1707 by 1027 window. Nets popup is
  much narrower than the current PCBCraft popup and has per-command icons;
  matching popup metrics/icons remains required. Launched target/debug/
  pcbcraft.exe and captured its real 1000 by 650 restored PCB window; schematic
  interaction QA and the full Altium comparison remain outstanding. The
  Computer Use connection blocker is resolved.

### 2026-11-01 — live schematic Active Bar continuation (partial)

- Computer Use through @oai/sky worked for live AD24.4.1 and PCBCraft research.
  Corrected the initial screenshot interpretation: Drawing Tools includes all
  ten arc/graphic commands; the fourteenth gold hexagon is Simulation Sources.
  Observed all group menus, filter grid, popup dimensions, owner dialogs and
  native records on the isolated ActiveBarReference-20261101.SchDoc copy.
- Matched the 476 x 33 1/3 point strip, 29 1/3 point popup rows, native group
  widths, separators, icon/text offsets, last-used commands and keyboard menu
  navigation. New original vector icons distinguish menu operations. Fixed the
  8-point schematic toolbar vertical offset and default Projects width to 334.
  Exact raster/icon/font/dialog/preference parity remains incomplete.
- Added typed Off Sheet Connector, Parameter Set, Differential Pair, Blanket,
  Compile Mask, sources/probes/Initial Condition, Graphic and Device Sheet
  reference data. Core commands own placement, masks, copies and history.
  Owned differential-pair parameters determine type independently of the label;
  copied differential probes receive a new shared pair ID. Masks propagate
  exclusions to owned children and collapsing restores compilation membership.
- Added parameter/class/rule and source/probe Properties. Supported schematic
  PCB rule editors reuse PCBCraft's independent rules UI. Simulation metadata
  is authored and preserved; no simulator execution or result data is claimed.
  The existing importer can load installed Simulation Generic Components.IntLib;
  a three-symbol independent fallback is available when it is absent.
- Device Sheet selector reads registered folders and derives entries from child
  ports, with Device Sheet source kind and unique U_<stem> designators. Local
  Design Reuse creates selection-only snippets, places with fresh IDs and
  reserved designators, opens documents and edits metadata. Its local manifest
  and JSON format are PCBCraft-specific; native Altium snippet metadata,
  thumbnails, deletion and full hierarchical device semantics remain open.
  Workspace-dependent cloud/Managed commands remain disabled.
- Expanded preserving SchDoc writing for new objects, owned parameters, source
  components and SIM models, Device Sheet source kind, Additional Blankets and
  image Storage. External image caches load from bounded local files and do not
  alter unchanged native bytes. Native source-body fill requires RECORD=8
  Ellipse; writing it as RECORD=12 Arc rendered an unfilled body in Altium and
  was corrected with a regression. Native moves refuse incomplete inactive
  symbol geometry rather than updating only its represented subset.
- Real Windows GUI QA: Nets/Drawing/Simulation menus; source placement and
  pre/post-placement Properties; one-step Undo/Redo and Save; external image
  visible after reopen; Device Sheet folder registration, port-derived symbol
  placement and Save. Created ActiveBar Supply QA from one selected source,
  verified one component/eight primitives/no native archive or records, placed
  it as V2 and saved again. Latest build reopens the final file, Ctrl+A selects
  23 roots, and nested Device Sheet windows open/close correctly. Fixed child
  windows falling behind a disabled parent, native file-dialog ownership and
  Escape while a field has focus; modal layering has an event regression.
- AD24.4.1 opened the intermediate ActiveBarImplementation-20261101.SchDoc and
  recognized VSRC V1 with DC Magnitude=5, AC Magnitude=1 and AC Phase=0. Final
  ActiveBarFinal-20261101.SchDoc reopens in PCBCraft; its SHA256 is
  redacted-source-sha256.
  Final Altium reopen is NOT verified: its former window disappeared from
  inventory. Refresh found no targetable AD window, and the supported launch
  retry reported 'launched app did not expose a targetable window'. No original
  Altium project was saved for research. Removed only the temporary research
  folder registration in Altium; all files and the original folder entry remain.
- Required gates PASS after the final code changes: cargo fmt --all -- --check;
  cargo clippy --workspace --all-targets -- -D warnings; all 604 workspace unit
  tests (app 127, core 250, geom 1, IO 122, render 104). Linux GNU all-targets
  cargo check and Windows dev build PASS; Linux runtime/DPI remains unverified.
  Logs: target/routing-check/active-bar-{fmt,clippy,full-tests,linux-check,build}.txt.
  Latest separately launched binary: target/routing-check/active-bar-verified/
  pcbcraft.exe. The existing user PCB window with unsaved edits was preserved.
- The first full link exhausted disk space in generated test/example artifacts.
  Cargo clean -p eda-io reclaimed 21.5 GiB, followed by successful full rebuilds;
  no project/source/research files were removed. Remaining limitations are in
  docs/interface/schematic-active-bar.md and schematic.active_bar in features.json.
  No commit or push during this continuation.

### 2026-11-01 — schematic cursor contrast and sheet dimensions

- User report: a white cursor disappeared over the schematic sheet, and sheet
  sizes were inconsistent. The schematic requested the themed Windows Crosshair
  cursor; it now paints a 14 logical px black cross with a white halo in
  eda-render. It follows the actual pointer independently of placement snapping,
  stays clipped to the active canvas, and leaves normal cursors to panels,
  modals and unfocused windows.
- New documents previously combined an A4 label with physical ISO paper size
  297x210 mm, while import and Properties used the observed Altium drawing area
  292.1x193.04 mm. New documents, import and format selection now share native
  drawing dimensions. Standard selection preserves portrait/landscape and marks
  the current format. Existing saved dimensions are preserved until the user
  explicitly selects a standard format; custom sizes are not normalized.
- Regressions cover all 18 formats in both orientations, validated history and
  Undo/Redo, unchanged native bytes and custom sheet dimensions. Render events
  cover light/dark backgrounds, 100/150/200 percent DPI, active placement, panel
  hover, inactive windows and modal dialogs. Initial new-test failures were
  corrected: custom fixture coordinates use 10 mil units; the panel separator
  must not be counted as a cursor stroke. Final 607 workspace unit tests PASS
  (app 127, core 251, geom 1, IO 123, render 105).
- Real Windows QA on the final build: new A4 shows 292.1x193.04 mm; Portrait
  shows 193.04x292.1 mm; choosing A3 retains Portrait and shows 281.94x393.7 mm.
  Undo restores A4 Portrait, Redo restores A3 Portrait. Home fits the tall sheet
  with the expected aspect ratio; black crosshair is clearly visible on the
  light canvas and keeps its size across zoom. Separate portable QA executable:
  target/routing-check/sheet-cursor-qa/pcbcraft.exe. Other open documents were
  not modified during this QA.
- Fresh Altium visual comparison remains unavailable: inventory had no AD
  window and the supported launch reported 'launched app did not expose a
  targetable window'. Native dimensions are grounded in prior AD24.4.1
  observations and SchDoc import checks. Native sheet-property export and
  Linux runtime/DPI visual parity remain outside this fix.
- Required fmt check, strict all-target workspace Clippy, full workspace tests,
  Windows dev build and Linux GNU all-target check PASS. Logs:
  target/routing-check/sheet-cursor-{fmt,clippy,tests,build,linux}.txt.
  No commit or push requested for this fix.

### 2026-11-01 ? schematic File menu and file operations (partial)

- Compared live File/New/Script/Mixed-Signal Simulation/Import/Export in Altium
  Designer Enterprise 24.4.1 through Windows Computer Use. Inspected Preview SCH
  and Smart PDF welcome/target/BOM/additional-settings pages. Both applications
  were maximized on the same desktop DPI for menu comparison. The real Altium
  LastWorkspace DsnWrk confirmed `[ProjectGroup]`, not an assumed workspace
  section. Research did not save or edit the user's Altium project.
- Replaced the File placeholders with the observed order and nested menus,
  independent icon gutter, measured row widths, 22 px rows and native-style
  separator groups. Removed unrelated fabrication/assembly entries from this
  schematic menu. Added local PrjPcb and DsnWrk operations, separate recent
  histories/migration, group dirty protection, Save Copy, project/document
  reference relocation, safe native authored-sheet creation and sheet-property
  writing. Undo survives Save As without reverting the new file name.
- Added bounded ASCII DXF interchange and declarative PCBCraft command scripts.
  Imports and scripts validate before a single history transaction; unsupported
  DXF entities/primitive families refuse conversion. Independent command script
  fixture: fixtures/schematic/set-title.pcbscript.json.
- Added read-only Print preview and Smart PDF using the actual schematic
  renderer, including current unsaved scene snapshots. Print has drawing scope,
  color, media/orientation, fit/scale and offsets; Windows invokes system Print
  and GDI, Linux uses CUPS lp. PDF uses lossless raster pages and component/pin/
  label/port bookmarks with invisible destination links and optional local net
  information/parameters. Output is bounded by page and total memory limits.
- Windows QA: File width initially expanded to 600 px; corrected by measuring
  row content. Nested item spacing and dialog button order were corrected after
  visual inspection. Save Copy As created target/file-menu-qa/gui-copy.SchDoc:
  its SHA-256 matches the unchanged fixture, and the original tab/name stayed
  active. Smart PDF wizard produced gui-smart.pdf; pypdf checks and independent
  PDFium page rendering verified its page, four link annotations and bookmarks.
  Visible annotation note icons discovered in PDF QA were replaced with invisible
  links. Preview SCH reached the real Xerox printer dialog and was cancelled
  without submitting a print job. Parent-window ownership/DPI restoration and
  repaint after the printer dialog are explicit in the implementation.
- Required gates PASS: cargo fmt --all -- --check; strict all-target workspace
  Clippy; 617 workspace tests (app 131, core 252, geom 1, IO 127, render 106).
  Windows dev build and Linux GNU all-targets cargo check PASS. Logs/artifacts:
  target/file-menu-qa/{fmt.log,clippy.log,tests-final.log,linux-check.log}.
  One repeat hit disk exhaustion while linking examples; standard cargo clean
  -p eda-io removed 12.2 GiB of generated artifacts, then the full tests passed.
- Full requested parity is NOT achieved. Disabled New authoring systems and
  legacy/proprietary converters, DWG, Altium Delphi APIs, Excel BOM templates,
  variants/full-project PDF semantics and exact dialog/icon/raster parity remain
  incomplete. Physical printer output and Linux runtime/DPI were not verified.
  Complete implemented/partial/disabled matrix: docs/interface/schematic-file-menu.md;
  feature entry schematic.file_menu is explicitly partial.
- Final executable is target/debug/pcbcraft.exe; a separate portable copy in
  target/file-menu-qa/pcbcraft.exe is launched for review. The original running
  user window was preserved by renaming its old executable to
  target/debug/pcbcraft-before-file-menu-20261101.exe before rebuilding.
  No commit, push or external publication was requested or performed.

### 2026-11-01 — SCH File continuation: NET, graphical DXF, project PDF links

- Computer Use in AD Enterprise 24.4.1 confirmed Export NetList: Export File,
  Project/Document Scope, 20 ordered format choices, OK/Cancel. A separate
  Protel NET under target/file-menu-qa confirmed bracket component records,
  three reserved fields, parenthesis net records, designator-pin endpoints,
  CRLF and CP1251. The original Altium schematic was not edited or saved.
  Export to AutoCAD was inspected and canceled: version, format, units and
  Include Template options are recorded as observed, not implemented parity.
- Enabled Netlist Schematic with a working Protel exporter. eda-core supplies
  validated physical components and compiled connectivity using the existing
  multipart, annotation, mask and project hierarchy rules. eda-io handles
  format/encoding and atomic writes. Current unsaved project models take
  precedence over disk without opening additional tabs; other formats stay
  disabled. Live PCBCraft exported the active TCI sheet to gui-export.NET:
  14 component records, 14 nets, 991 bytes, source tab still clean.
  The observed dialog group positions and outer size were corrected after live
  comparison; repeat GUI NET export produced identical bytes. A newly created,
  not-yet-saved schematic also retains its project scope for export/output and
  Save Project As; the snapshot regression covers this association.
- Extended DXF from the old primitive subset to ordered graphical SOLID faces
  from the shared read-only renderer. Includes component/pin/power-port/port
  graphics, fills, frame and outlined fonts with holes; curves are flattened.
  eda-geom owns document-space faces, eda-render converts screen geometry and
  font contours, eda-io writes R2004/mm true-color DXF. Added planar SOLID
  reimport with fill-only triangles. Images/unknown objects/non-outline fonts
  and drawings above 250,000 faces refuse output before destination replacement.
- Smart PDF now compiles net information once in project context, including
  unchecked sheets, using native project options. Exported pin links cycle
  through visible pins of a net and prefer another selected page. Forward page
  references are valid; unchecked sheets cannot become dangling destinations.
  Invalid link destinations preserve an existing output file.
- Independent QA: ezdxf 1.4.5 read 23,922 faces with zero audit errors/fixes;
  independently rendered geometry/font details were compared with the canvas.
  pypdf verified both directions of a two-page VCC link; PDFium rendered both
  pages. Tests cover current unsaved project NET, native NET syntax/encoding and
  destination preservation, graphic DXF roundtrip, glyph counters, project PDF
  links and selected-page boundaries. Artifacts/logs: target/file-menu-qa.
- Required gates PASS: fmt check, strict workspace/all-target Clippy, 624 tests
  (app 134, core 252, geom 1, IO 130, render 107). Native build and Linux GNU
  all-target check PASS. A full example relink initially exhausted the disk;
  standard cargo clean for eda-app/eda-render/eda-io removed 59.1 GiB of generated
  artifacts, after which the complete workspace test run passed.
- Remaining scope is explicit in docs/features.json and
  docs/interface/schematic-file-menu.md: only Protel among the 20 NET formats;
  DXF graphic faces rather than electrical CAD transfer; native AutoCAD options,
  DWG and other CAD converters, separate authoring systems, Delphi API, BOM
  templates/variants and full dialog/icon/typeface parity remain incomplete.
  PDF drawing text remains rasterized. No claim of full File parity is made.

### 2026-11-01 — SCH Project / Validate

- Observed Project / Validate in Altium Designer Enterprise 24.4.1 through
  Windows Computer Use: project/free-document labels, ordered Project menu,
  Shift+Ctrl+F3/F4, warning-only compile preserving Properties, Messages columns,
  severity squares and bottom Details. ReferenceB produced two missing-driver
  warnings (NetC6_1, NetC13_2), one net-alias warning and Compile successful.
  A later Altium capture failed with `window crop is outside captured monitor`;
  reselecting its returned window did not recover capture. Observation limits
  and assumptions are explicit in docs/interface/schematic-project-validate.md.
- Added read-only eda-core project validation over shared compiled electrical
  islands. Checks annotation/duplicate designators, multipart physical pins,
  unconnected/single-pin nets, missing input drivers, pin electrical conflicts
  and net aliases; honors compile masks and generic No ERC. Native PrjPcb first
  eight ERC matrix rows/columns supply pin-to-pin severity. Validation reports
  aliases and continues; ordinary ECO/netlist compilation still refuses them.
- Current unsaved/new project sheets override disk, closed sheets are read
  without opening tabs, broken/unsupported compilation produces fatal messages.
  Messages has compiler columns, resizable columns/Details, tooltips/Copy and
  object navigation by double-click and wrapping Next/Previous shortcuts.
  Each run replaces its report and explicitly warns about incomplete coverage.
- Live Windows QA used maximized windows at the reference desktop DPI. Fixed
  submenu arrow positions and full-row hit areas, and disabled ScrollArea
  shrinking/minimum-content expansion in Details. Verified Validate, rerun,
  diagnostic double-click, Ctrl+Shift+F3 and warning-only Properties retention.
  On ReferenceB the GUI and read-only schematic_validate harness find the same three
  electrical problems observed in Altium, with natural-order driver pin lists.
  Reference PrjPcb and SchDoc SHA-256 hashes remain unchanged. Existing user
  PCBCraft/Altium windows were preserved; QA used its own portable executable.
- Required final gates PASS: cargo fmt --all -- --check; strict workspace /
  all-target Clippy; cargo test --workspace, 637 tests (app 138, core 260,
  geom 1, IO 131, render 107). Windows dev build and Linux GNU all-target check
  PASS. Artifacts/logs: target/validate-qa/{final-gates.log,ReferenceB-diagnostics.txt,
  reference-hashes-before.json}. Final executable: target/debug/pcbcraft.exe;
  a byte-identical target/validate-qa/pcbcraft.exe is used for the live review.
- Full Altium parity is NOT achieved. Native TypeN reporting levels, extended
  matrix categories, the remaining ERC rules, rule-specific No ERC, hidden-pin /
  repeated-channel compiler semantics, variants, live markers, full Messages
  context menus/filter/sort/export, asynchronous/cancellable compilation and
  exact messages/icons/fonts/pixels remain incomplete. Linux runtime/DPI was
  not visually verified. docs/features.json marks schematic.project_validate
  partial. No commit, push or publication was requested or performed.

### 2026-11-01 — SCH Validate continuation

- Continued native Windows Computer Use observation on a separate project copy.
  Confirmed Compiler source, exact success text, full alias object descriptions
  and repeated power-object counts, short Details pin names, Messages context
  menu order, Error Reporting groups and all 17 Connection Matrix categories.
- Extended core and PrjPcb matrix import to 17x17 (pins, ports, sheet entries,
  Unconnected); passive unconnected pins honor No Report. Added port conflict /
  No ERC / unconnected tests. ReferenceB's three electrical diagnostic strings now
  match the observed native strings, including Power Object GND (23).
- Messages supports multiple selection, column sorting, grouping, Copy,
  Save/Save Selection TSV and Clear All/Selected/Class. Unsupported specific
  No ERC, Export To Report and Print are disabled. Later application messages
  append with independent timestamps/source and preserve the compiler report.
  Details uses short natural-order pin names and the observed red error icon.
  Native grouping/export format and full object-tree parity remain unverified.
- Native TypeN mapping probes were inconclusive: Altium raised an index error
  loading controlled copies, then runtime error 216 and process exit. Original
  PrjPcb/SchDoc SHA-256 hashes were checked unchanged. User unsaved native
  session recovery was not verified. Computer Use restart left X2 running
  without a targetable window; capture recovery also reported monitor-crop and
  foreground-process-id errors. No per-rule TypeN mapping is claimed.
- Final required gates PASS: fmt check, strict all-target workspace Clippy,
  workspace tests (639: app 139, core 261, geom 1, IO 131, render 107), Windows
  dev build and Linux GNU all-target check. Test output is in
  target/validate-qa/continuation-tests.log. The QA executable matches the
  target/debug executable SHA-256. Native complete parity remains unfinished;
  docs/features.json keeps partial status and lists outstanding behavior.
- Live continuation QA verified Validate, short Details pin labels and red
  summary icon, ordered context menu, ascending/descending No. sorting and
  Save producing target/validate-qa/Messages-continuation.txt with five rows
  (three native-matching diagnostics, explicit coverage warning, success).
  Corrected Save to reuse the parented application FileDialog. Error summaries
  never claim no errors. Final gate log: continuation-final-gates.log.

### 2026-11-01 — Validate reporting levels; user stop checkpoint

- User restarted Altium and authorized continuation. Corrected the ignored QA
  generator to modify only Electrical Rules Check; earlier copies also changed
  Modification/Difference TypeN values. Corrected copies load and validate.
- Four native bit-coded probes and two independent inverse checks confirm
  Type10=no driving source and Type14=multiple names, with 0/1/2/3 mapping to
  No Report/Warning/Error/Fatal Error. Native Error/Fatal runs omit a summary.
  Observations: target/validate-qa/native-rule-observations.json.
- Implemented scoped TypeN import, unknown-level retention, invalid-level
  rejection, and the two confirmed rule mappings. Added section isolation and
  severity/suppression regression checks. Updated Error color and omitted
  synthetic error summaries. These newest UI changes have not had another live
  PCBCraft review; previous Windows GUI checks remain documented above.
- User explicitly requested stopping and committing all work. Native UI input
  and further feature work stopped; specific No ERC investigation remains
  unfinished. Full Altium parity remains partial. Commit preparation runs the
  required fmt/Clippy/workspace tests, plus native build/Linux check, recorded in
  target/validate-qa/stop-commit-gates.log.
- Commit checkpoint completed 2026-11-02: fmt check, strict workspace/all-target
  Clippy, all 640 tests (139 app, 261 core, 1 geom, 132 IO, 107 render), Windows
  build and Linux GNU all-target check PASS. Original user PrjPcb/SchDoc hashes
  checked unchanged. Full feature parity remains explicitly unfinished.

### 2026-10-28 — Blanket, Compile Mask and Parameter Set directives

- Observed AD Enterprise 24.4.1 live Properties: Blanket Border Width, Line Style,
  Border/Fill Color and Collapsed and Disabled; Parameter Set Location, Label,
  Large/Small style and All/Parameters/Rules/Classes; native Width rule dialog.
  Official reference: https://www.altium.com/documentation/altium-designer/schematic/directives
- Implemented typed core objects and validated edit/history lifecycle. Blankets
  support rectangular/polygonal outlines, concave containment, nonintersection,
  corner/edge resizing and perimeter-attached directives. Masks exclude complete
  objects and descendant sheets of masked Sheet Symbols. Collapse restores scope.
  The legacy JSON compile_masks field remains readable and follows typed edits.
- Added editor placement, preview, repeat placement, Enter/Backspace, border-only
  picking, collapse toggles, handles and pre-placement appearance/parameter drafts.
  Properties shows area-specific controls and Parameter Set filters/add/edit/remove;
  supported schematic constraints share the PCB constraint widgets. Placed masks
  fade excluded objects; an unfilled sheet border no longer intercepts area picks.
- Blanket net/component classes, component parameters, differential pairs and
  supported PCB rule families enter ECO. Multipart sections map to one physical
  component class member. Width rules affect route checks. Class membership is
  cleared when its Blanket is disabled; scope is determined from the same Blanket.
- Native SCH: 225 Additional, 211 FileHeader, 43 Parameter Set and hidden 41 rule
  children using the native broken-bar separator. ASCII/CFB create/edit/delete,
  coordinate quantization and owner remaps are verified. Additional WEIGHT must
  include newly authored Blankets; creating the stream requires its own header.
  AD reopening caught the missing-count issue that self-roundtrip did not detect.
- Live native research found legacy Blanket border width stored as LineStyle + 1,
  modern width as LineWidth (0..3), and LineStyleExt directly 0=Solid/1=Dashed/2=Dotted.
  Added regression coverage for all width/style choices; missing style stays Solid.
  Native PCB Classes6/DifferentialPairs6 preserve unchanged/opaque fields. Rules6
  creation uses only observed AD24 family tags; unknown families require a template.
- Real ReferenceG read-only comparison after multipart fix: 4 existing unavailable baseline
  rows, 0 enabled changes, about 2.4 s to compare. Logs:
  target/directives-ReferenceG-profile-final.txt. QA mask excludes C8, R17, R20, R21, R22,
  R24, TP168, VD3, VD4 and corresponding UART_PROG_MCU class members. AD24 live ECO
  showed the same 9 removals, 19 pin removals and class changes. No Execute Changes.
  Research copies live only under target; all 11 original source SHA256 hashes match.
- Native AD reopened target/directives-pcbcraft-qa2.SchDoc, displaying the authored
  polygon after Additional count correction; observed Dashed exposed and fixed an
  off-by-one in LineStyleExt. Final generated copy is directives-pcbcraft-qa3.SchDoc.
- Full parity remains open: X/Y reflection/protected locks, shared multi-selection
  Properties, complete ERC/No ERC filtering, bus/harness/multichannel scope,
  duplicate parameter names/diff-pair classes, class-generation/comparator options,
  removal reconciliation for all generated rules/pairs, exact visuals and Linux QA.
  Native ASCII PCB class/pair modifications and new rule creation remain refused.
  These are recorded as partial, not as completed commands or verified parity.
- Validation logs: target/directives-{workspace-current,core-verified,io-current,
  clippy-current,build-current,fmt-final}.txt. Latest full run before the final
  style regression: 568 pass, one pre-existing GPU routing dither pixel failure
  (frame 13, Rgba8Unorm, difference 4 versus tolerance 3). New IO style suite: 115 pass.
  Strict workspace/all-target Clippy and fmt check pass; final rerun follows below.

- Final style/Additional regression suite: 115 IO PASS. Final workspace run:
  569 PASS and the same pre-existing GPU pixel mismatch only. After the live Width
  dialog exposed clipped OK/Cancel controls, constrained the shared schematic
  constraint region and added an all-supported-families bounds regression.
  Final app suite: 116 PASS, strict all-target Clippy and fmt PASS. Build:
  target/debug/pcbcraft.exe. Logs: directives-{workspace-release,io-current,
  rule-layout,app-ui-verified,clippy-ui-verified,build-ui-verified,fmt-final}.txt.
  Live PCBCraft confirmed area Properties, mask collapse/unmask and exact Undo to
  clean state, Parameter Set values and opening the Width constraint editor.

- Follow-up real-window check: Width Constraint shows both OK and Cancel below
  the table; Cancel returns to the clean document without applying an edit.
- Added document-coordinate X/Y reflection of Blanket/Compile Mask while moving
  or dragging, composing with Space/Shift+Space around the cursor. Covered
  circuitry is never transformed. Preview and commit share a core transformation;
  one History transaction updates mask exclusions and Undo restores the old scope.
  Parameter Set reflection and the other parity gaps listed above remain open.
- Differential Pair ECO now preserves actual positive/negative net spelling and
  updates existing pair names case-insensitively while retaining opaque fields.

- Latest complete validation after reflection/pair-case fixes: 573 PASS, 1 existing
  GPU routing pixel failure (app 117, core 237, geom 1, IO 115, render 103+1).
  fmt and strict workspace/all-target Clippy PASS; debug build PASS.
  Logs: target/directives-{workspace-latest,clippy-latest,fmt-latest,build-latest}.txt.
- Final real Windows QA: Altium opened directives-pcbcraft-qa3.SchDoc and selected
  the authored polygon with Medium/Solid matching PCBCraft. New PCBCraft instance
  demonstrated M,M then X reflected preview, click commit and one Ctrl+Z returning
  the document to its clean state. Source circuit did not move. Exactly one live
  PCBCraft window left running on the QA copy. All 11 original ReferenceG hashes still match.


### 2026-11-02 — Synchronize local directive work with origin/main

- User requested fetching upstream updates, merging as necessary, committing all
  current work and pushing. Preserved the original local work in a temporary Git
  stash and fast-forwarded main from 91a890f to ca2a9f0 (six upstream commits).
- Reconciled area directives with upstream harnesses, Active Bar placement,
  simulation probes, File workflows and Project Validate. Kept both work logs and
  feature entries; unsupported/full-parity limitations remain explicit.
- Unified Parameter Set data with retained native child presentation. Child edits
  update typed parameters/rules atomically; typed edits retain/update children,
  native saves avoid duplicate parameters, and old JSON label fields are accepted.
  Regression coverage checks child editing/Undo and native field/rule roundtrips.
- Verification artifacts are under target/sync-qa. Final gate and real-window
  results are recorded below after the completed checks.
- Windows dev build, final fmt check and strict workspace/all-target Clippy PASS.
  Real-window QA was attempted through the Computer Use skill: launch_app twice
  returned `accessibility window-opened handler did not become ready`, and no
  PCBCraft window was returned afterward. No GUI parity verification is claimed
  for this merge. The existing Altium window was not manipulated.
- Final cargo test --workspace: 659 PASS, one previously documented GPU routing
  pixel failure: Rgba8Unorm, frame 13, dither=true, difference 4 versus tolerance
  3 (eda-render/src/tests/pcb_route_gpu_tests.rs:235). App 142, core 271, geom 1
  and IO 139 all PASS; render 106 PASS / 1 FAIL. All merge regression checks pass.
  No PCB GPU implementation/test files were changed by this merge. Full log:
  target/sync-qa/workspace-tests.log. Final strict checks: fmt-final.log and
  clippy-final.log; successful Windows build: build.log.


### 2026-11-02 ? SCH/PCB selection and navigation correspondence

- User requested native Altium controls, especially default left-button rectangle,
  right-button pan, component/wire/net selection and Alt/Shift variants.
- Observed AD Enterprise 24.4.1 using an authored fixture copy: component toggle,
  wire then segment refinement, directional frame/partial child text selection;
  inspected PCB Select menu without editing/saving user designs. Official rules
  and unverified native modifier combinations are distinguished in
  docs/interface/editor-controls.md.
- Shared SCH/PCB camera navigation, robust schematic press/release cancellation,
  default/additive frame, child text selection, electrical-island net highlights,
  memory and nudge/Undo implemented. PCB adds Ctrl-pad frame, Alt airwire choice,
  exact individual-copper crossing, overlap click cycle and physical/Tab expansion.
- Source/GUI validation is in progress; final results follow below. Full native
  preference/menu/marker/edge-pan/shove parity is not claimed; features.json keeps
  both controls features partial and enumerates remaining differences.

- Read native System/General and Mouse Wheel Configuration without changing
  settings. Left/Right selection is enabled; Ctrl-wheel zoom, plain/Shift scroll
  and Ctrl+Shift-wheel layer/channel mappings are confirmed in the installed UI.
- Restored native Ctrl-click PCB highlight and Ctrl+Shift-click cumulative nets;
  Ctrl-click no longer implicitly starts Cross Probe. T,C retains explicit
  continuous/jump modes. Ctrl+Shift-wheel now requests a layer change and does not
  leak an egui smoothed camera zoom. Schematic compiled channels remain absent.

- Real portable GUI: SCH default left rectangle selected four authored objects;
  S popup opened; Escape preserved the selection; source stayed clean. Maximized
  PCBCraft client 1920x1032 and AD client 1918x1030 use the same desktop DPI,
  but dock widths and exact marker/Fit metrics still differ.
- PCB GUI exposed an existing invisible-polygon hit: a drag through empty,
  unpoured polygon interior moved its definition instead of making a rectangle.
  Undid the authored-copy movement and closed without saving. Picking now uses
  polygon boundaries or actually drawn copper; crossing frames test boundaries.
  Added renderer/controller regressions, including unpoured/non-stale state and
  both directions. Final fresh GUI check and verification results follow below.

- Fresh extracted portable PCB check: left-to-right containing frame and
  right-to-left crossing frame select R1 without editing the model; empty
  unpoured polygon interiors no longer capture the gesture. Track click followed
  by Tab selects the two physically connected copper objects (track and pad).
  S menu layout is readable in the real window. No authored fixtures were saved.
- GUI also exposed egui's pending Tab focus traversal stealing subsequent editor
  shortcuts. Selection Tab/Shift+Tab now resolve that traversal before property
  fields register; a multi-frame real-egui regression checks the following S key
  and confirms that property text receives no input. Routing Tab retains its
  deliberate property editing mode.
- Final source checks: cargo fmt --all -- --check PASS; cargo clippy --workspace
  --all-targets -- -D warnings PASS. cargo test --workspace --no-fail-fast:
  677 PASS, one previously documented GPU pixel comparison FAIL. App 153, core
  273, geom 1, IO 139 PASS; render 111 PASS / 1 FAIL. The unchanged failure is
  Rgba8Unorm frame 13 dither=true, difference 4 versus tolerance 3, at
  crates/eda-render/src/tests/pcb_route_gpu_tests.rs:235. No test was disabled or
  weakened. Full log: .tools/interaction-tests-final.log.
- Final portable (release, locked MSVC x64) re-extracted and launched from TEMP
  with only Windows on PATH: Track -> Tab -> S opens Select immediately, Escape
  preserves two selected objects, Shift+S changes layer display without opening
  Select. Document title stays clean; both authored reference files remain byte
  identical to the fixtures. Data/app.ron is written next to the EXE. ZIP CRC,
  executable manifest hash and absence of smoke-test state in the archive PASS.
- Delivery: dist/PCBCraft-0.1.0-windows-x64-portable-2026-11-02-controls.zip,
  13,806,587 bytes, SHA256
  redacted-source-sha256.
  Sibling controls-verification.json and windows-portable-2026-11-02-controls-tests.log
  record the checks and known failure. Original dated portable archive preserved.

### 2026-11-02 — imported PCB query failures blocking routing

- Audited enabled scope expressions in 21 native PCB files available on this
  machine, read-only. Reproduced RouteSession startup failures on ReferenceD
  (InDifferentialPair), ReferenceE (OnTopLayer), and ReferenceC (HasFootprintPad). Native
  original file hashes are retained in .tools/query-routes-before.json.
- Added exact owned-pad footprint/name matching, differential pair membership
  from POSITIVENETNAME/NEGATIVENETNAME and native class KIND=6, explicit copper
  layer predicates, side/silkscreen/mid-layer predicates, hole shape/plating and
  DrillType scopes, Name Like/inequality, boolean-string comparisons, function
  whitespace, symbolic logical operators, XOR and flat long boolean chains.
  Existing OnTop/OnBottom now include their associated overlay/paste/solder
  layers, as documented by Altium. Unknown predicates remain errors.
- Rule resolution errors now identify rule name, kind and SCOPE1/2EXPRESSION.
  No rules are disabled, no generic fallback replaces an unknown scope, and
  imported native records are not rewritten by evaluation. Builder exposes
  the newly supported differential-pair/class choices.
- Six regressions cover scopes, native legacy literal pad identifiers, strict
  unknown rejection, layer/side distinctions and route preview/place/finish plus
  exact History Undo. The read-only profile tools gain --audit-queries, --pairs
  and --net. Source byte equality replaces a reimport equality check in the
  routing probe; exact loaded-model Undo remains independently asserted.
- Initial repeated audits: routing-related query errors removed from ReferenceD, ReferenceE,
  ReferenceC and ReferenceF. ReferenceG retains the ambiguous numeric-OR annular scope
  ((PadXSize_AllLayers or PadYSize_AllLayers)> HoleDiameter); no guessed numeric
  semantics are installed. Height/CompPinCount/component package predicates and
  unsupported constraint families remain outside the implemented evaluator.
- Final workspace, native routing and Windows portable GUI results follow.
- User clarified that the reported project is on another PC and unavailable
  here. Local reproduction is evidence for the exercised scopes, not a claim
  that the absent board has been tested.
- Final native probes PASS on ReferenceD (29,362 items, D_P), ReferenceE (41,409 items, GND)
  and ReferenceC (27,202 items, ExampleOutput): startup, nonzero preview, place, finish,
  exact Undo and unchanged source bytes/hashes. Existing STEP-import warnings
  remain on ReferenceD/ReferenceE; they do not originate in query evaluation. Results:
  .tools/query-routes-final.json. Audit before/after:
  .tools/query-audit-before.json and .tools/query-audit-after.json.
- Windows strict Clippy/all-targets PASS. Final workspace tests: 683 PASS,
  one unchanged pre-existing GPU dithering pixel comparison FAIL (Rgba8Unorm,
  frame 13, dither=true, difference 4 vs tolerance 3). App 153, core 279, geom 1,
  IO 139 all PASS; renderer 111 PASS / 1 FAIL. No tolerance/test was weakened.
  Full log: .tools/query-compat-tests-final.log. Portable GUI results follow.
- Final release portable GUI PASS on an authored byte-identical ReferenceE copy:
  Fit Board, select/zoom CAN1-H area, Ctrl+W, GND via, nonzero route preview,
  place, Escape finish. GND length changes 435.467 -> 436.452 mm. One Ctrl+Z
  removes the branch and restores the clean document title. Closed without
  saving; original and research-copy SHA256 remain unchanged. An automatic
  repour status appeared briefly after finish; Undo restored the original view.
- Portable launched from TEMP with Windows-only PATH, writes Data/app.ron,
  and passes ZIP CRC/executable-manifest/no-test-state checks. Delivery:
  dist/PCBCraft-0.1.0-windows-x64-portable-2026-11-02-query-fix.zip,
  13,817,186 bytes, SHA256
  redacted-source-sha256.
  Sibling query-fix-verification.json and windows-portable-2026-11-02-query-fix-tests.log
  record the results. The controls portable archive is preserved separately.

### 2026-11-02 - slow batch and online DRC

- Reproduced severe latency on read-only ReferenceE (41,409 items), ReferenceD (29,362),
  ReferenceC (27,202), and ReferenceA (982). Kept a separately built pre-optimization checker
  using the same query evaluator and stable serialized IDs for comparison.
  Initial unbuffered JSON loading was separated from measured DRC time; later
  measurements use direct buffered snapshots and time only checking.
- Prepared priority/scope/matrix caches, scoped per-item broad-phase gaps,
  cached bounds and complex-pour segment indexes, prepared board edges and
  identical per-item/layer annular reuse. Removed per-item full-board cutout scans.
  Spatial mask merging retains original merge/source order; exposed pad clipping
  is cached and far silk pairs do not consume the check budget. Unknown scopes
  keep conservative diagnostic evaluation. Connectivity skips already-joined nodes.
- Honor configured finding limits with ordered-prefix results and explicit
  truncation; exactly-N complete checks retain complete status. Later diagnostic
  phases can be unvisited only when the report is explicitly incomplete.
- Online checks now debounce the last edit/preview/settings change, cancel stale
  work/manual preemption/document closure, never overlap two jobs for a document,
  discard cancelled/racing results, avoid double cloning, and stop idle polling.
- Regressions cover priority/scope/error equivalence, duplicate IDs/probes,
  exact all-pairs copper/matrix/footprint behavior and report prefixes, exact
  segment-index distances with holes/islands, exhaustive silk mask/exposed copper,
  distant-pair budget preservation, concave/convex outlines, rare large-gap scopes,
  mid-check cancellation, stable online revisions and manual preemption/closure.
- Final timing/equivalence, workspace and portable GUI results follow.
- Optimization checkpoint before the subsequent Altium correctness comparison.
  These counts/timings and the following portable hash describe that checkpoint.
- Uncontended dev-optimized timings, excluding load, median of three
  at limit 500: ReferenceA batch 0.154 s / online 0.134 s; ReferenceE batch 0.381 s / online 0.387 s; ReferenceD batch 1.280 s / online 1.265 s; ReferenceC batch 0.594 s / online 0.586 s.
- Complete single passes at limit 50,000 all finish without truncation: ReferenceA batch 0.146 s / online 0.126 s; ReferenceE batch 8.802 s / online 6.655 s; ReferenceD batch 12.897 s / online 8.240 s; ReferenceC batch 8.137 s / online 6.707 s.
- Full original ReferenceA report is identical (277 batch / 226 online); original
  1.105/1.060 s vs new median 0.154/0.134 s. Exact prior-engine comparison
  also passes on 1/16 native ReferenceE/ReferenceD/ReferenceC samples in both modes. New full
  ReferenceD (30,847 findings) and ReferenceC (5,055) exactly match the intermediate
  exhaustive-geometry reports; original large-board jobs exceeded 180 s
  except ReferenceC online at 129.086 s (old work continued after report cap).
- Final Windows workspace: app 155, core 288, geom 1, IO 139 and renderer
  111 PASS; one unchanged renderer GPU dithering comparison FAIL (frame 13,
  Rgba8Unorm, 4 vs tolerance 3). Total 694 PASS / 1 FAIL. Fmt/strict Clippy PASS.
- Extracted release portable GUI PASS on the authored unchanged ReferenceE copy:
  Windows-only PATH/TEMP launch, Fit Board, online markers, manual 500 findings
  (43 unchecked, incomplete) and full 10,871 findings (44 unchecked, complete).
  Title stays clean; no native Save. Closed the test instance. ZIP CRC, EXE
  manifest hash and absent archived smoke state PASS; Data/app.ron is created.
- Delivery: dist/PCBCraft-0.1.0-windows-x64-portable-2026-11-02-drc-fix.zip,
  13,859,894 bytes, SHA256 redacted-source-sha256.
  Prior query/controls archives preserved. Dated verification JSON and interactive
  timing HTML are included separately in dist/. The reference checker retained
  the shared connectivity root-skip optimization; measured DRC-loop comparisons
  are conservative for that part. Linux GUI/performance was not exercised.

### 2026-11-02 - Altium comparison and electrical boundary checkpoint

- The optimization checkpoint above predates intentional correctness changes.
  On the byte-identical ReferenceE research copy, Altium 24.4.1 completes with zero
  violations twice (10 s and 8 s, Stop when 50,000). Source SHA256 remains
  redacted-source-sha256.
- Corrected circular annular-ring measurement, effective polygon nets including
  read-only outlines, filled-land electrical connectivity, rounded mask offsets
  and owned Net Tie copper. Poor-contact CHECKBADCONNECTIONS is explicitly
  unchecked rather than treated as satisfied by island connectivity.
- Independent native probes: complete combined report has 1368 findings,
  including unrelated ReferenceA-copy findings. The isolated copper pairs accept
  48.26 nm deficit and report 50.8 nm. Electrical copper-pair DRC now uses a
  measured 50 nm margin. Mask probes report a 2.54 nm deficit; mask, annular,
  outline/hole comparisons and routing rule values were not relaxed.
- Same-board ReferenceE findings fall from 10871 to 76, without truncation. Remaining
  differences: 75 mask slivers at 3-5 nm deficit, one 0.15 mm internal same-net
  via-pad pair against a 0.2 mm rule. Altium has zero; parity remains incomplete.
  A separate drc-reference-e-mask-isolated.PcbDoc contains eight translated ReferenceE mask
  pairs. Its native run is awaiting user assistance: capture became black and
  refreshed activation returned "failed to activate captured window". The
  preceding combined full run was launched successfully by Computer Use.
- Fresh idle dev-optimized full limit-50,000 batch/online DRC times, excluding
  import/deserialization and scheduling delay, single sequential passes:
  ReferenceA 0.136/0.124 s (93/42 findings, 21 unchecked);
  ReferenceE 7.781/6.029 s (76/76, 45/44 unchecked);
  ReferenceD 11.070/7.244 s (15525/15514, 17 unchecked);
  ReferenceC 7.569/6.237 s (61/61, 27/26 unchecked). No report is truncated.
  Other boards have not been compared with native Altium DRC; these counts
  establish timing only, not correctness of every finding. Native source hashes
  and the ReferenceE copy all remain unchanged.
- Fmt and strict workspace/all-target Clippy PASS. Workspace: app 155, core 295,
  geom 1, IO 139, renderer 111 PASS; same existing GPU frame-13 dithering FAIL
  (Rgba8Unorm, 4 differing pixels vs tolerance 3). Total 701 PASS / 1 FAIL.
- Updated release portable ZIP/import/dependency/license/CRC/manifest checks
  PASS. Archive has empty Data and no smoke state. Current binary has not been
  rechecked in a visible GUI while Computer Use activation is unavailable; the
  earlier scheduler/UI smoke observation above is historical, not this binary.
  dist/PCBCraft-0.1.0-windows-x64-portable-2026-11-02-drc-fix.zip,
  13,856,945 bytes, SHA256
  redacted-source-sha256.
  This is a reviewable checkpoint, not completion of the Altium parity stage.

### 2026-11-02 - project assembly variants checkpoint (user stopped)

- User requested full variant operation and schematic display, then explicitly
  requested stopping further implementation and committing/pushing the result.
  This checkpoint does not complete the full variant stage.
- Observed live AD Enterprise 24.4.1 Variant Management on the existing research
  project: component and parameter grids, variant columns, bottom actions,
  Projects selection and Editor/physical sheet tabs. Native stale VD7/VD9 link
  warning observed, not repaired. No native project/schematic Save performed.
- Corrected the actual ProjectVariantN import (legacy VariantN retained), native
  Kind 0/1/2, UniqueId hierarchy, ParamVariation/ParamDesignator and read-only
  .PrjPcbVariants symbol decoding. Native Desktop sources were read only.
- Added GUI-independent variant/variation/alternate models, validation, history
  and read-only assembly projections; not-fitted marks, varied parameter italics,
  special strings, pin-compatible alternate geometry and stable pin identity.
  Added manager CRUD/parameter/reset/history controls, project variant selection,
  physical sheet view/toolbar, independent project dirty/save handling, atomic
  external-change guarded Save and current-definition Save Project As. Print/PDF
  snapshots use the physical view within the documented channel restrictions.
- Core/IO/app regressions cover unchanged base documents/pins/wires/history,
  invalid-history atomicity, scoped UIDs, alternate pin mismatch, parameter
  precedence, native metadata/unknown-field roundtrip, Unicode Save As, external
  modifications, view cache/read-only guard, save/reopen and project Undo.
- Validation: cargo fmt --all -- --check PASS; strict workspace all-target
  Clippy PASS after fixing a new test initializer warning; dev app build PASS.
  cargo test --workspace: app 157, core 299, geom 1, IO 141, renderer 111 PASS:
  total 709 PASS / 1 existing FAIL. The unchanged renderer routing pixel test
  still reports Rgba8Unorm frame 13, dither=true, 4 pixels vs tolerance 3.
- Attempted a real-window smoke launch of an isolated newly built executable
  with its own portable state. Computer Use returned "accessibility window-opened
  handler did not become ready"; refreshed window inventory had no PCBCraft
  window. Final Windows UI behavior/pixel QA remains unverified; no success claim.
- Remaining work is explicit in docs/interface/schematic-variants.md and
  docs/features.json. In particular, newly authored alternate geometry is stored
  as PCBCraftAlternateDataN extension fields; native Altium alternate-library
  writing, complete multipart/Repeat/channel behavior, UID repair, reports,
  other assembly outputs and exact UI parity remain unfinished. No new portable
  release was requested or built for this checkpoint.

### 2026-11-03 - schematic editing latency

- User reported severe lag during schematic editing. Initial read-only CPU
  diagnostic on ReferenceCControl.SchDoc (125 components, 2162 scene
  primitives, 3476 parameter objects) measured Move/Drag/wire previews at
  112.638/171.288/172.268 ms. These are diagnostic means of five operations,
  not native window latency or a release benchmark.
- Removed repeated document-wide identity searches from preview differences,
  patch application, owner validation, parameter expansion and hierarchy/
  directive reconciliation. Added exact-integer R-tree candidate pruning for
  derived junctions and the changed-wire overlap closure. Existing exact
  geometric predicates, stable wire colour/order and unrelated native overlaps
  remain authoritative.
- A wider probe exposed remaining group Move latency (35/56 ms on Con/Pow).
  Added indexed selection roots and grouped extraction, shared with connected
  Drag. Selection/child ordering, hidden parameters, native text roles and image
  filtering are checked against independent single-object extraction results.
- All-component connected Drag then exposed a separate 171/220 ms Con/Pow
  path. Added spatial branch/moving-point/anchor queries, shared attachment
  collection and indexed stem-ID checks. Cleanup now rejects unrelated tips
  before cloning paths/styles. One-vertex imported wire branch semantics have
  a dedicated regression; existing exact overlap/topology tests remain active.
- Added a bounded revision/document/command preview cache. Unchanged previews
  stop scheduling frames; eventless follow-up paints no longer recreate UUIDs
  for placement tools. Core validation, atomic commits and Undo/Redo remain in
  the edit path. No Altium project/source file was changed.
- Added exhaustive junction-reference, transitive overlap/history, cached
  Undo/Redo/document isolation and real egui stationary Move/Place scheduling
  regressions. The read-only schematic_edit_profile example covers common
  editing operations and CPU frame tessellation; methodology and limitations
  are in docs/schematic-performance.md.
- Real-window verification attempted with the bundled Computer Use skill.
  Both list_windows and refreshed list_apps returned "Computer Use native pipe
  is unavailable: failed to connect native pipe ... (os error 2)". No current
  Windows visual/interaction success is claimed; Linux runtime is unverified.
- User interrupted after the parallel rebuild exhausted memory. No compiler or
  linker processes remained on inspection. PowerShell cleanup was rejected by
  automatic policy; standard `cargo clean --profile dev` succeeded and removed
  29.1 GiB of generated artifacts. Source projects and release outputs retained.
  Added `.cargo/config.toml` with one build job to bound compiler/linker
  concurrency on this 16 GB machine; subsequent verification runs serially.
- Serial dev app/probe build PASS after cleanup (cold dependencies: 32m 07s).
  Repeated read-only CPU corpus with no competing compiler: Con Move/Drag/wire
  11.030/12.224/8.633 ms; all-component group Move/Drag 21.683/39.271 ms.
  Pow (211 components, 5643 parameters) Move/Drag/wire 16.127/18.592/15.485 ms;
  all-component group Move/Drag 32.721/57.935 ms. Large connected group edits
  remain more expensive than single edits; no universal 60 fps claim. Updated
  raw report includes all operations; all four input SHA-256 values unchanged.
- Final validation: cargo fmt --all -- --check PASS; strict workspace/all-target
  Clippy PASS on the final Rust sources (before the interrupted parallel test
  build; subsequent changes only configure build jobs/document results).
  Serial cargo test --workspace --jobs 1 -- --test-threads=1: app 160, core 303,
  geom 1, IO 141, renderer 111 PASS; total 716 PASS / 1 existing FAIL. All new
  schematic regressions PASS, including the one-vertex wire branch case.
  Unchanged PCB GPU routing test still reports Rgba8Unorm frame 13, dither=true,
  4 pixels different vs tolerance 3 at pcb_route_gpu_tests.rs:235. Full command
  therefore exits 101; no tolerance relaxed or unrelated renderer code changed.
  JSON/feature-ID validation and git diff --check PASS. New executable is
  target/debug/pcbcraft.exe; 42.72 GiB free disk after the serial rebuild/tests.

### 2026-11-03 - hidden Parameter Set fields and text rotation

- User clarified that previously hidden data is appearing around Parameter Sets,
  and that text/parameter rotation seems reversed. Read the current PCBCraft
  ReferenceJ window without editing its unsaved document. Read-only native source
  audit found Small STYLE=1 Parameter Sets with ISHIDDEN=T ClassName=ExamplePowerClass.
- Follow-up clarification isolated the unwanted output to the literal parent
  label "Parameter Set", not the hidden ClassName values. The Small marker
  label fix directly addresses that reported case; child visibility is retained.
- The importer already retains the hidden child flag. The renderer nevertheless
  emitted the parent's stored "Parameter Set" label for Small markers. Small
  markers now omit that label; Large markers retain it. A primitive-level
  visibility guard prevents hidden fields from generating ink, bounds or hits.
- Read-only local Altium 24 configuration audit found DisplayStringAsRotated=1.
  PCBCraft previously forced orientations 180/270 back to right-reading 0/90
  and inverted justification. Text now uses all four document angles with the
  same local anchor; screen-space angles negate document angles because Y is
  downward. Large directive labels use the full orientation too.
- Removed the corresponding importer workaround which substituted Top Right
  for an omitted JUSTIFICATION at 180/270. Missing native enums now retain the
  zero Bottom Left anchor independently of the angle; explicit anchors survive.
- Added tessellated-glyph rotation checks against independent document-space
  rotation for all four angles and nine anchors; marker/hidden-field paint and
  hit checks; ASCII/CFB hidden-child rotation, copy, save/reopen and History
  coverage. Official references: Altium schematic/directives and
  preferences/schematic (Display Strings As Rotated). Exact new AD24 window
  comparison remains unverified: launch did not expose a targetable window.
  The two background X2 processes created by these failed attempts were stopped
  to release memory. Existing user PCBCraft session was left running.
- Validation: fmt PASS; strict workspace/all-target Clippy PASS; serial workspace
  tests 720 PASS / 1 existing PCB GPU routing FAIL (Rgba8Unorm, frame 13,
  dither=true, 4 pixels vs tolerance 3). All four new regressions PASS.
  Dev app build PASS; updated target/debug/pcbcraft.exe. To build without closing
  the user's dirty ReferenceJ session, its running executable/hardlink were retained
  as pcbcraft-before-parameter-text.exe; the standard executable path now holds
  the new build. No user document was closed or saved.
- Launched a separate portable QA executable under .tools/parameter-text-qa.
  First capture was black, activation reported "failed to activate captured
  window"; fresh selection/capture showed the Windows lock screen. Stopped UI
  input as required by Computer Use and requested desktop unlock. Current new
  window behavior remains unverified until the desktop is available. No current
  AD24 visual parity or Linux GUI success is claimed.


## 2026-11-04 - Run DRC / live AD24 continuation

- User requested continued Run DRC implementation with Computer Use comparison.
  Altium Enterprise 24.4.1.13 eventually exposed its main window after the user
  started it. Original projects and their unsaved documents were left intact;
  native experiments used ignored ReferenceB copies and independent probes.
- Full observations and rejected hypotheses: docs/pcb-update/DRC_COMPARISON.md.
  Native baseline is two violations versus initial 26 PCBCraft findings. Native
  mask-8/mask-17, Standard-component, Any Net, polygon-gap, collinear-midpoint and
  straight-edge ordinary-region controls were actually opened and run. Stop
  limit 10000 was not reached. A global 1 micrometre edge tolerance is disproved.
- Implemented shared exclusion of retained compatibility/body records from DRC,
  object-specific keepout clearance, native Track/Arc/Fill restriction masks and
  the corrected Arc modern-layer offset. Different Nets now excludes two No Net
  copper objects, while physical routing/pour obstacles keep their scoped gaps.
  Mechanical cutouts and keepouts continue to constrain unassigned copper.
- Text glyph/board-edge geometry is explicitly unchecked. Native extended text
  location and rectangle fields were investigated; no guessed font/baseline
  formula or blanket polygon tolerance was added to force equal result counts.
- Checker uses the observed 771 x 700 outer dimensions and a shorter body on
  small viewports; configuration-only rules are excluded, Return Path remains
  disabled, headers sort, Online/Batch have independent Used On / All On / All Off
  actions. Live QA identified label interception of the context menu; the menu
  is now attached to label responses, checkbox alignment and tree gutters fixed.
- Built a separate portable QA executable and opened the same source copy.
  Real window: native-sized checker and successful Run DRC, three violations /
  29 unchecked conditions. Matching keepout gap is 0.172657 mm. The two polygon
  edge findings and missing native text finding remain unresolved; full matching
  errors, native docked report/Message presentation, typography/icons and Linux
  GUI parity are not claimed.
- Existing live PCBCraft session was retained without closing/saving documents;
  its executable was retained under target/debug before rebuilding the standard
  path. QA data and captures remain ignored. No private project enters Git.
- Validation in progress: the first complete workspace attempt found two routing
  regressions from No Net rule selection and an obsolete exhaustive-test unwrap.
  Physical obstacle resolution and the independent reference loop were corrected;
  final checks and latest-menu GUI verification follow below.
- Final portable QA build: confirmed the checker at 771 x 700 in the real window,
  corrected tree gutters, centered checkboxes and the six-entry context menu.
  Online All Off clears Online while leaving Batch intact; Online Used On restores
  the checks with enabled definitions. Running Batch again completes with three
  violations / 29 unchecked conditions. The source document remains clean.
- Re-ran native DRC on the unchanged source copy: two violations, zero warnings.
  Original and local source copy remain byte-identical after all observations.
- Privacy follow-up: two newly generated local orchestration checkpoint commits
  had non-anonymous author metadata. Anonymized those local refs, expired obsolete
  reflogs/objects; reachable-history scan passes again. Main history is unchanged.
- Final validation: cargo fmt --all -- --check PASS; cargo clippy --workspace
  --all-targets -- -D warnings PASS (38.48 s); cargo test --workspace PASS,
  727 tests / zero failures (160 app, 308 core, 1 geometry, 144 IO, 114 render),
  including the previously intermittent GPU routing test. Full test build/run
  completed in approximately six minutes. Dev application build PASS (5m 47s).
  git diff --check and privacy scans, including reachable history and the new
  comparison document, PASS. Native text/polygon parity and unsupported rule
  coverage remain open; this validation does not establish complete DRC parity.
