# Schematic Properties — AD Enterprise 24.4.1

Наблюдение 20.10.2026, Windows, окно 1707 × 1027 логических пикселей.
Правый dock: x=1390..1706, содержимое x=1398..1693. Источник — живое окно,
не документация другой версии. ReferenceB уже имел unsaved marker до исследования;
исходник не редактировался и не сохранялся. Дополнительные состояния исследованы
на отдельном собственном ASCII-листе, не в пользовательском проекте.

## Наблюдённая компоновка

Header Properties y=54..78; строка контекста y=86..110, справа
`Components (and 11 more)` и кнопки фильтра. Search y=120..142.
Document Options имеет General/Parameters; Component — General/Pins;
Port — General/Parameters. У Parameter, Designator, Text, Text Frame,
Net Label, Power Port, Wire и Rectangle этих вкладок нет.
Статус выделения внизу над вкладками Messages/Properties.

Секции: заголовок около 20 px, фон группы RGB 65,65,65, поля около 22 px,
шаг обычной строки 26 px. Подписи выровнены вправо; поля начинаются
примерно в 117 px от левого края содержимого. Длинные подписи обрезаются,
не растягивают dock. Обычная ширина dock — 316 px, Projects — 330 px.

| Контекст | Наблюдённые секции/поля |
|---|---|
| Document General | Selection Filter: All - On, 12 отдельных категорий в четырёх рядах. General: Units mm/mils, Visible Grid + глаз, Snap Grid + checkbox, Snap to Electrical Object Hotspots, Snap Distance, Document Font, Sheet Border + цвет, Sheet Color. Page Options: Formatting and Size, Template/Standard/Custom, Sheet Size + фактические размеры, Orientation, Title Block, Margin and Zones, Show Zones, Vertical/Horizontal, Origin. |
| Document Parameters | All/Parameters/Rules; Name/Value; пользовательские, затем динамические параметры; Add…/карандаш/корзина. Таблица с внутренней прокруткой. |
| Component General | General: Designator и Comment + глаз/замок; Part/of Parts; многострочный Description; Type; Design Item ID; Source. Location: X/Y и Rotation. Parameters: All/Footprints/Models/Parameters, затем Links/Rules; модели, параметры, Show More, ссылки, Add…/карандаш/корзина. |
| Component Pins | Таблица Pins/Name/Show Full; имена и номера выводов, значки видимости; нижняя строка замка/видимости/Add/карандаш/корзина. |
| Parameter/Designator | Location: X/Y, Rotation. Properties: Component (ссылка), Name, Value, видимость имени/значения; Autoposition, Lock Parameter, Allow Synchronization with Database; Font family/size/color, B/I/U/strike, матрица Justification 3×3. Для Comment исходное выражение, например `="R_USDD"`, а не его вычисленный текст. |
| Text | Location; Properties: Text, URL, Font, Justification. |
| Net Label | Location; Properties: Net Name, Font, Justification. |
| Power Port | Location; Properties: Name + глаз, Style + цвет, Font. Ниже General (Net), Parameters (Net). |
| Wire | General (Net): Physical Name, Net Name, Power Net, High Speed, Differential Pair. Parameters (Net): All/Parameters/Rules/Classes, таблица. Vertices: Width + цвет, Index/X/Y, Add/корзина. Smallest=101600, Small=254000, Medium=508000, Large=1016000 nm. |
| Text Frame | Location: только X/Y. Properties: Text multiline, Word Wrap, Clip to Area, Font, Alignment, Text Margin, Width/Height, Border checkbox/width/color, Fill Color. |
| Rectangle | Location: X/Y. Properties: Width, Height, Border width/color, Line Style; Fill Color checkbox/color, Transparent. |
| Port | Location: X/Y. Properties: Name, I/O Type, Harness Type, Cross Ref, схема Width/Height/Location, размеры, Font, Alignment, Border, Fill. General (Net), Parameters (Net). |

## Реализация и границы проверки

Это карта наблюдений, **не утверждение полного совпадения**.
Реализация хранит скрытые параметры, исходные выражения, presentation, метаданные
компонентов и моделей отдельно от исторического source archive. Properties
редактирует модель через проверяемые команды и Undo/Redo. Новые поля JSON
имеют defaults для старых документов; архив Altium остаётся исходным.

Список стандартных листов наблюдён: A4/A3/A2/A1/A0, A/B/C/D/E,
Letter/Legal/Tabloid, OrCAD A/B/C/D/E. Импорт, Properties и подпись формата
используют одну таблицу drawing area в nm, а не размеры бумаги ISO.
Show More раскрывает параметры в порядке исходных объектов. Стартовые
избранные Power/Resistance воспроизводят наблюдённый R9; это предположение
о начальной настройке, не доказанная настройка каждого компонента Altium.
Звёздочки позволяют менять этот список в PCBCraft.

Нужно отдельно подтвердить/завершить: несколько выбранных объектов и фильтр
типов в шапке, placement/move, Sheet Symbol/Entry, Pin, No ERC, Image, IEEE,
дуги/эллипсы/Bezier, template/custom/title-block варианты, параметры моделей,
библиотечные диалоги и compiled net metadata. Не реализованные действия
должны оставаться отключёнными. Strike, Clip to Area, Text Margin и некоторые
дополнительные флаги пока не имеют полного контракта модели.
Динамические параметры документа, точная семантика блокировок/автопозиции
и выравнивание некоторых таблиц также требуют отдельной работы.

Linux использует те же egui-виджеты и команды Rust. Совпадение пикселей,
растеризации и метрик на Linux требует отдельного запуска GUI.

Переход к Components/IntLib по условию пользователя — после полного завершения
Properties; этот переход ещё не выполнен.
