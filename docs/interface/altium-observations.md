# Наблюдения интерфейса Altium

19 октября 2026, локальный UI через computer-use. Файлы проекта не изменялись.
Скриншоты просмотрены в чате; локальный архив скриншотов не создавался.

Altium Designer Enterprise 24.4.1; заголовок ReferenceB.PrjPcb.
Тёмная тема, интерфейс English. Размер полученного основного изображения
1707 × 1027 логических пикселей. Системный DPI, build сверх 24.4.1,
установленные расширения и полный набор редакторов не проверены.

Home: File, View, Project, Window, Help. Projects слева, Messages справа,
вкладки документов сверху, Panels справа снизу.

PCB: File, Edit, View, Project, Place, Design, Tools, Route, Reports,
Window, Help. Projects слева; вкладки над canvas; Active Bar над рабочей
областью; ряд слоёв внизу canvas; статус координат и привязки внизу.
Справа Messages и Properties разделяют вкладки одной dock-области.
В начальной оболочке Messages открывался снизу; после уточнения визуального
контракта он перенесён в общую правую область с Properties и нижними вкладками.

Properties без выделения: Board, Search, General/Parameters/Health Check,
Selection Filter, Snap Options, Grids/Guides/Axes, snapping all/current/off,
objects for snapping, snap distance, axis snap range, Board Information.
Повторно исследованы свойства выбранной площадки rep1 (без правок документа):
Net Information; Properties с Designator, Layer, Net, Electrical Type,
Propagation Delay, Pin Package Length, Jumper; Template/Library; X/Y/Rotation;
Pad Stack с preview, Simple/Top-Middle-Bottom/Full Stack и таблицами
COPPER/PASTE/SOLDER. Значения электрических полей и stack относятся к
пользовательскому документу, их нельзя переносить в собственную плату PCBCraft.

File в PCB: New, Open (Ctrl+O), Close (Ctrl+F4), Open Project,
Open Project Group, Save (Ctrl+S), Save As, Save Copy As, Save All,
Save Project As, Save Project Group As, Import, Export, Import Wizard,
Run Script, Fabrication Outputs, Assembly Outputs, Print (Ctrl+P),
Smart PDF, Recent Documents/Projects/Project Groups, Exit (Alt+F4).

PCBCraft использует Ctrl+F4 для Close (Ctrl+W — alias только в PCB, в SCH это Wire), собственный формат и только доступное
подмножество команд. Родные PCB/SCH расширения Altium не заявлены как поддержанные.

Повторная сверка: оба окна развёрнуты, полученные изображения 1707 × 1027.
Системный DPI и точные шрифтовые метрики ещё не подтверждены.
Состояние визуального соответствия: `visual-parity.md`.

Повторная сессия: наблюдаемая ширина Projects около 330 points, Properties
около 316. Title strip около 23, меню до y52, document tabs до y78;
Active Bar около x623..1097/y84..114. В Board Properties: Selection Filter
y194, Snap Options y356, Board Information y827. Это ориентиры, считанные
по изображениям 1707 points по ширине, а не результаты пиксельного анализа.
Изображения этой сессии имели высоту 1019..1027 points в зависимости от
состояния рамки/захвата; сравнение полной высоты не считается нормализованным.

Установленная тема DevExpress содержит имя Segoe UI. Windows-версия PCBCraft
читает системный Segoe UI и Semibold; метрики WPF и egui отдельно не измерены.

## Schematic: наблюдение 19 октября 2026

Altium Designer Enterprise 24.4.1, ReferenceB.PrjPcb/ReferenceB.SchDoc.
Меню File/Edit/View/Project/Place/Design/Tools/Simulate/Reports/Window/Help;
Active Bar над листом; снизу Editor и вкладка документа. Properties без
выделения: Document Options, General/Parameters, Selection Filter, General,
Page Options, Margin and Zones. Видимая сетка 1 mm, snap grid 2.5 mm,
документный шрифт Altium GOST Type A 10, Landscape, A4 292.1×193.04 mm.
Числа сопоставлены с записями файла. Они не означают проверки всех вариантов
настроек редактора. Собственный minimal.SchDoc открыт в эталоне и показал
основные объекты; пользовательский исходник не сохранялся поверх оригинала.

## Schematic: меню Place, 20 октября 2026

Наблюдение открытого Altium 24.4.1 без изменения схемы: Bus, Bus Entry,
Part…, Power Port, Wire (Ctrl+W), Net Label, Port, Off Sheet Connector,
Sheet Symbol, Sheet Entry, Device Sheet Symbol, Reuse Block…, Harness,
Directives, Text String, Text Frame, Note, Drawing Tools, Comment….
Drawing Tools: Arc, Full Circle, Elliptical Arc, Ellipse, Line, Rectangle,
Round Rectangle, Polygon, Bezier, Graphic….

В PCBCraft сохранён этот порядок; неподдержанные функции отключены.
Part… пока имеет собственные R/C, отдельный Junction — добавление PCBCraft.
Это наблюдение подписей/порядка, а не проверка всех команд и их shortcut sequences.
