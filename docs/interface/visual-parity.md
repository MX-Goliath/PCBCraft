# Визуальное соответствие Altium Designer

Обязательная цель: точная копия доступного Altium Designer Enterprise 24.4.1,
Windows и Linux. Источник требований — пользователь и исходный план, §7 и §25.
Точное совпадение ещё не достигнуто. Структура и плотность повторно сверены
в состояниях Board и Pad; оставшиеся различия ниже являются открытой работой.

19 октября 2026, повторно просмотрены реальные развёрнутые окна обоих приложений.
Полученные изображения: ширина 1707; высота 1019..1027 в разных состояниях
рамки окна/захвата. Скриншоты отображены в чате.
DPI не измерен, пиксельное сравнение не выполнялось.

| Поверхность | Выполнено | Осталось |
|---|---|---|
| Оболочка | Общая строка title bar 23 points; Quick Access, drag, double-click maximize, min/max/close, края resize | Точные метрики и иконки, поведение resize/drag на Linux, системное меню |
| Меню | Наблюдаемый порядок, Home/PCB контекст, File, Ctrl+F4 | Полное содержимое и геометрия каждого меню |
| Projects | Левая область 330 points, Search с векторной иконкой, папки и PCB-иконки в дереве | Точный набор toolbar, header dropdown/pin/autohide, проектная модель |
| Messages/Properties | Одна правая область 316 points, нижние вкладки, активный синий заголовок | Полный docking, точные header/tab states, pin/float, остальные нижние вкладки |
| Board Properties | General/Parameters, отключённый Health Check; Selection Filter; Snap Options с таблицей 21 point на строку; Board Information | Точные табы/поля/отступы, selection context dropdown, остальные типы selection/snapping, guides/axis range и health engine |
| Pad Properties | Net Information, Properties, Pad Stack; Designator/Layer/X/Y/X-Size/Y-Size меняют собственную модель с undo | Electrical/net/propagation/template/rotation/lock и multilayer pad stack; вместо физического 3D показана собственная прямоугольная площадка |
| Active Bar | Полоса около 472×31 points, 13 собственных векторных иконок, Fit/Grid/Select | Точные пиктограммы, цвета, состояния, инструменты размещения/трассировки |
| Цвета и плотность | Нейтральные серые группы/поля, рамки; Segoe UI из Windows, встроенные Selawik Regular/Semibold | Цвета подобраны визуально; соответствие WPF/egui rasterization и Linux метрик ещё не измерено |
| Linux | Тот же UI/layout, встроенные шрифты, успешный cargo check всего workspace с all-targets для Linux GNU | Нативная линковка, тесты и запуск X11/Wayland, сопоставление DPI/ввода |

Проверены реальные переключения Messages → Properties, состояния Board/Pad,
ввод X=12mm из 10mm с применением по Tab и возврат через Ctrl+Z.
fmt, clippy и шесть тестов проходят; новая Windows-сборка запущена и просмотрена.
Ориентиры в итоговом Board Properties: Selection Filter около y194,
Snap Options y358, Board Information y829; эталон около y194/356/827.
Это визуальные оценки, не допуск точного соответствия.

Привязки используют реальные центры/вершины/края прямоугольных pads и
концы tracks с ограничением расстояния/слоя/видимости. Неподдерживаемые
строки таблицы отключены, поэтому их состояние отличается от Altium.

Неподдерживаемые инженерные команды отключены. Их присутствие в меню не
означает реализации. Различия геометрии двух текущих плат не являются
результатом сравнения одинакового документа: native Altium import ещё отсутствует.
