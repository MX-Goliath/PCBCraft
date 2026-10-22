# PCB Properties — 21.10.2026

## Наблюдения

Эталон: запущенный Altium Designer Enterprise 24.4.1, ReferenceBBoard.PcbDoc
проекта ReferenceB. Нативное окно 1707×1027; PCBCraft 1707×1019 (разная
высота рамки окна). Правая панель около 316 px. Исходная плата не изменялась.
Для осмотра Text временно включался Selection Filter > Texts, затем возвращён
исходный фильтр. Данные схемы с уже имеющимся `*` не сохранялись.

Наблюдались Board General/Parameters, Component, Track, Pad, Via, Text и Polygon
Pour. Board: строка контекста, Search, General/Parameters/Health Check, Selection
Filter в трёх рядах, Snap Options, Board Information, нижняя строка выбора.
Component: Location, Properties, Footprint, Swapping Options. Track: Net
Information, Location, Properties, Paste/Solder Mask Expansion. Pad/Via: Net
Information, Properties/Definition, Template/Library, координаты с замком,
Pad/Via Stack и Hole Information. Text: Location, String/Frame, Text/Layer/Mirror,
Text Height, Font Type, Justification, Font, Stroke Width, Border Mode. Polygon:
Actions, Net Information, Properties и Outline Vertices. Arc/Fill/Region и mixed
selection не подтверждены отдельной полной нативной сверкой.

## Реализация

- Контекстная панель заменила общий редактор с Apply/Revert и служебными
  секциями Polygons/Nets/Board. Общие размеры полей, раскрываемые группы,
  поиск, вкладки, фиксированный статус выбора и прокрутка; ширина 316 px
  сохраняется при выборе объектов и длинных импортированных строках.
- Board: работающие фильтры поддержанных объектов, Grids, режимы All Layers /
  Current Layer / Off, расстояние и отдельные переключатели реализованных
  привязок. Shift+E переключает режим; привязки вычисляются в eda-core,
  экранные координаты в них не используются. Поддержаны концы tracks/arcs,
  центры arcs/pads/vias, вершины regions/polygons/rotated fills, board outline
  и origins компонентов. Настройки вида отдельны от модели и истории.
- Компоненты: положение, поворот, замок, designator/comment, видимость строк;
  смена Top/Bottom выполняет существующую команду Flip. Импортированные
  описание, source ID, библиотека и footprint отображаются только для чтения.
  Высота читается из OVERALLHEIGHT доступных проекций корпуса.
- Track/Arc/Pad/Via/Text/Fill/Region: доступные размеры, координаты, слой/сеть,
  текст, поворот, замок и геометрия. Polygon: параметры собственного pour engine,
  repour/shelve/restore и прямые вершины. Изменения идут через валидируемые
  команды и History; импортер/рендерер не мутируют документ из панели.
- Координаты полей относительно ORIGINX/ORIGINY исходной платы; модель остаётся
  в абсолютных целочисленных nm. Поддержаны mm и mils. Ошибочный ввод не меняет
  модель; устаревший draft не перезаписывает Undo/Redo. Enter/потеря фокуса
  завершают поле, Escape отменяет числовое/однострочное поле.
- Исправлен импорт Comment: буквальный Texts6 comment больше не заменяется
  пустым SOURCECOMMENT. Компонент и принадлежащая ему строка синхронизируются;
  исходные метаданные, скрытое состояние и source archive сохраняются.
- Незавершённое изменённое поле учитывается dirty state и предупреждением
  закрытия. Save завершает Properties перед записью. При смене выбора draft
  фиксируется один раз, включая переход к смешанному выбору.
- Board Parameters показывает вычисляемые сведения импортированной модели;
  параметры с явным PARAMETER.* отображаются для чтения и фильтруются поиском.

## Проверка и границы

Проверки: fmt --check, workspace Clippy all-targets с -D warnings,
169 workspace tests, Windows debug build, Linux GNU all-targets cargo check.
Новые регрессии: все контексты при длинных полях в dock 316 px; Enter/Escape в
mm/mils через настоящие egui input events; atomic/stale drafts; dirty/Close/Save;
смена выбора и Undo/Redo; literal component comments; переключатели привязок,
слои/видимость, концы дуг, повёрнутые fills и board vertices.

В реальном Windows окне просмотрены Board, Component/XP1, Track, Pad и Via; via
Diameter 0.7 → 0.8 mm, потеря фокуса, Undo и Redo. Тестовое изменение отменено,
нативный файл не записан. Исходное пользовательское окно PCBCraft сохранено.
Linux GUI и Linux-артефакты не запускались/не собирались.

Полное визуальное или функциональное совпадение пока **не подтверждено**.
Остаются: точные вертикальные интервалы/шрифты/иконки, все состояния и DPI,
templates и библиотеки padstack, text frames,
barcode/justification/borders и точные stroke fonts, component area/opacity и
замки strings/primitives, параметры Workspace/DbLib, contextual filter popup,
полная rule engine/классы/propagation delay/Health Check, редактирование mixed
selection, дуговых polygon vertices и общая DRC/rule engine. Такие функции
отключены или показаны как недоступные значения. В native Snap Options Pad
Edges был включён; поддержка добавлена 22.10.2026 (см. ниже). Grids сейчас
связывает видимость сетки и привязку; native view/filter persistence требует
отдельной сверки. Фильтры Selection Filter по умолчанию включают поддержанные
типы, native SELECTIONFILTER_* пока не декодируется.

Board Information использует реальный outline, а не doc.size с off-board drill
table. Вычисленная площадь 1099.093 sq.mm отличается от наблюдённых 1099.162;
Components Area не подменяется оценкой. Поля Parameters относятся к доступной
импортированной сцене, не к скомпилированному проекту Altium. Pour использует
существующий движок PCBCraft; изменение меди может перепостроить полигоны и
не подтверждает совпадение native pours. Эта ранняя проверка не включала native interoperability; результат последующей
проверки масок и оставшиеся ограничения сохранения описаны ниже.


## Padstack, 3D и маски — продолжение 21.10.2026

### Наблюдение

Исследована отдельная копия `pcb-padstack-reference.PcbDoc` в AD 24.4.1.
Исходная плата пользователя не редактировалась. Наблюдались Simple,
Top-Middle-Bottom и Full Stack, раскрытые Copper/Hole/Top Solder, Round/Square/
Slot, Copper Offset, Manual/Rule Expansion, From Hole Edge, Tented и связь сторон.
У Pad/Via маски находятся в общей таблице стека. В native Full Stack текущей
двухслойной платы показаны две строки меди, в T/M/B — три. Тenting Top и Bottom
независимы; на исследовательской копии сохранён только Top Tented.

### Реализация

- `eda-core`: типизированный padstack с отдельной геометрией слоёв, Simple/T/M/B/
  Full, четырьмя формами площадок и corner radius, локальным смещением меди,
  Round/Square/Slot сверлением и его поворотом. Геометрия общая для picking,
  bounds, 2D, pour и предпросмотра. Flip меняет стороны стека и масок.
- `eda-render`: собственная 3D-геометрия с переносимым software depth buffer и egui texture: поверхности
  меди с настоящими вырезами, стенки отверстия, plated barrel, верхняя/нижняя
  маска и паста. Drag вращает, колесо масштабирует, double-click сбрасывает вид.
  Вращение не меняет модель и историю. Глубина проверяется на каждом пикселе;
  изображение кэшируется до изменения геометрии/вида, границы сглаживаются
  повышенным разрешением. Высота берётся из доступных dielectric
  полей, с fallback 1.6 mm; внутренние слои пока распределены по высоте равномерно.
- Properties: единая таблица Copper/Hole/Paste/Solder с раскрываемыми строками,
  переключателями режимов, послойными размерами/формами, slot/offset/plating,
  отдельными сторонами масок, связью expansions и независимым tenting.
  Раскрытые строки сохраняют dock 316 px; числовое представление округлено
  до трёх знаков, внутренние nm сохраняются до редактирования.
- Маски: Manual overrides и Rule Expansion; приоритет enabled-правил,
  All/IsPad/IsVia/IsTrack/IsArc/IsSMDPad, InNet/InComponent/HasFootprint,
  NOT/AND/OR и скобки. Неизвестное условие приоритетного правила явно возвращает
  ошибку, а не выбирает более низкое All. Поддержаны отрицательная expansion,
  percentage, enabled, tenting, From Hole Edge с формой Slot. Геометрия масок
  используется также на видимых слоях Paste/Solder основного PCB canvas.
- Через кнопку у разрешённого правила доступен редактор существующего
  mask-expansion rule: scope, enabled, priority, expansion, tenting. Это ограниченный
  редактор, его окно пока не воспроизводит весь native PCB Rules and Constraints
  Editor. Изменения проходят валидацию и общую History.
- PCB6: чтение/запись legacy Top/Mid/Bottom, 32 via diameters, pad extra block,
  slot/offset/индивидуальных corner percentages и modern Full Stack entries.
  Board6 layer chain читается до сборки итоговых слоёв. Поля и неизвестные хвосты,
  wildcard rows и rule prefixes сохраняются. Tented flags сверены native Save.
  JSON сохраняет весь новый типизированный stack и независимые маски.
- Маски и переключение режима без изменения фактической меди не запускают
  repour. Изменение геометрии меди/сверления по-прежнему инвалидирует pour.

### Проверка и оставшиеся границы

188 workspace tests (38 app / 70 core / 1 geom / 56 IO / 23 render), fmt --check,
strict Clippy all-targets, Windows debug build и Linux GNU all-targets check.
Регрессии покрывают slot/offset/cutout, послойный picking/bounds, atomic edits,
Flip, mask rule priority/unknown scopes, native round-trip с неизвестным хвостом,
Rules6 prefix/opaque fields, отказ от потери данных, конечность 3D vertices,
blind via span, быстрый drag и ширину раскрытой панели в течение 60 кадров.

Windows: просмотрена выбранная Pad 2 в реальном окне PCBCraft, таблицы Simple/
T/M/B/Full, вращение 3D и Top Tented с обновлением предпросмотра,
Undo/Redo/Undo до чистого состояния.
Изменения в QA-окне не записывались в пользовательский
файл. Проверочная копия `pcb-padstack-roundtrip.PcbDoc`, записанная PCBCraft
после изменения Top Tented и solder-mask rule expansion, открылась в AD 24.4.1;
у Pad 2 визуально подтверждены Top Tented=true и Bottom=false. Полная проверка
всех правил/других объектов и DRC этим не подтверждается.

Полного визуального совпадения пока нет: native материалы/освещение/камеру,
разделители/иконки/интервалы и все DPI ещё нужно сверять. Templates/PadViaLibrary,
thermal relief и Pad Features не реализованы. Современные custom mask shapes,
независимые native side expansions/percentages и Pad From Hole Edge native Save
пока отклоняет; JSON их сохраняет. Обычные симметричные expansions/modes,
независимый solder tenting и Via From Hole Edge записываются. Advanced stack/
mask edits в ASCII отклоняются. Полный drill tolerance/IPC-4761 и точное физическое
расположение внутренних/plane-слоёв не завершены.

На реальной плате изменение геометрии с последующим repour может изменить число
native primitives. Save в этом случае отказал из-за непереназначенных ссылок
UniqueIDPrimitiveInformation; исходный и выходной файл не были изменены. Этот
предохранитель сохранён. Успешное открытие копии после изменения только масок
не подтверждает native-сохранение произвольной изменённой меди. Linux GUI в этой
итерации не запускался; Linux-артефакт не собирался.


## Привязка Pad Edges — 22.10.2026

По сохранённому наблюдению AD 24.4.1 включён пункт `Pad Edges` в существующей
таблице Snap Options, без изменения её порядка и компоновки. Переключатель
управляет только видом; документ и Undo/Redo не меняются.

В `eda-core` курсор проецируется на ближайшую точку внешней границы меди Pad.
Учитываются Rectangle/Octagonal/Round/RoundedRectangle, радиусы углов,
поворот, Copper Offset и размеры конкретного слоя Simple/T/M/B/Full Stack.
Round с разными сторонами имеет форму капсулы. Окружности вычисляются
аналитически, без зависимости от тесселяции или масштаба экрана.

Current Layer использует видимую медь текущего слоя; All Layers рассматривает
видимые медные слои. Нулевая медь и drill-only pads не создают фиктивных краёв.
Радиус захвата измеряется от исходного курсора; вне радиуса остаётся обычная
сеточная привязка. Pad Centers работает независимо, Ctrl+Shift временно
отключает привязку. Среди поддержанных кандидатов выбирается ближайший.

Проверки: конечные стороны/углы, дуги изнутри и снаружи в 120 направлениях,
капсулы, скруглённые и восьмиугольные площадки, послойные размеры/смещения,
поворот, скрытые слои, нулевая медь, Snap Distance, Off и неизменность модели.
Отдельная регрессия подаёт реальные pointer/modifier events в PCB canvas и
проверяет координаты, передаваемые контроллеру при включении/отключении и bypass.

Новая живая сверка с Altium в Linux-среде недоступна. Приоритеты совпадающих
привязок, custom pad shapes, внутренние края отверстий и полная native
семантика All Layers остаются неподтверждёнными. Pad Vertices, Track/Arc Lines
и Intersections по-прежнему недоступны; этот этап реализует только Pad Edges.


Итоговая проверка: 193 workspace tests (38 app / 74 core / 1 geom / 56 IO /
24 render), fmt --check, Clippy all-targets -D warnings и Linux debug build.
В Linux GUI на editor-v2.PcbDoc при одном положении мыши Pad Edges включён:
X=9.750/Y=9.669 mm; выключен: X=10.000/Y=10.000 mm (grid 1 mm); повторное
включение возвращает edge snap. Документ остаётся чистым.
TI PMP23595 импортирован: 8452 объекта, 236 компонентов, 76 цепей, 33 полигона.
PMP22650 остаётся заблокирован ошибкой `Invalid padstack land size, shape or offset`;
исправление этого случая не входит в реализованную привязку. Оригиналы TI и
скриншоты проверок находятся вне Git.
