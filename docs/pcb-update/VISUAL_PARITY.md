# PCB: сверка с Altium 24.4.1 — 21.10.2026

Эталон — открытый пользователем `ReferenceBBoard.PcbDoc` из проекта
`ReferenceB.PrjPcb`. Режим 2D, активен Top Layer. Исходный файл и плата
в Altium не редактировались. Полное визуальное совпадение пока **не достигнуто**.

## Наблюдения

- В View Configuration подтверждены Top Layer RGB 153/0/0 (`#990000`)
  и Bottom Layer RGB 11/83/148 (`#0B5394`). Цвета принадлежат профилю Altium,
  а не геометрии PcbDoc; эти наблюдённые значения использованы как палитра.
- В Object Visibility у Pads, Polygons и Regions на сигнальных слоях
  прозрачность 20%; у Tracks, Arcs и Texts — 0%. Эти настройки присутствуют
  в `2DCONFIGURATION` документа. Включены pad numbers/nets и via nets/span.
- В Components6 у всех 69 компонентов NAMEON/COMMENTON=false. Записи строк
  при этом существуют в Texts6. Вручную размещённые надписи не скрываются
  этими флагами.
- `Carbon on Top L` находится на современном Mechanical 18. Однобайтовый
  fallback слоя не является его идентификатором. Таблица сверления красная;
  нижняя и верхняя шелкография одновременно видимы поверх меди.
- Контур платы содержит дуги с начальной вершиной у конечного угла дуги,
  а также швы длиной в несколько нанометров. Обход только против часовой
  стрелки создавал диагонали и разворот пути на скруглениях.

## Реализация

| Область | Изменение | Граница проверки |
| --- | --- | --- |
| Слои | Современные ID Tracks/Arcs/Fills/Texts, V7_LAYER у Regions и корпусов; имена из V9 cache; Mechanical 1–1024 | Реальная двухслойная плата; перенос произвольных modern layers при native Save ограничен |
| Отображение | Видимость, прозрачность по типам, snap grid из native 2D-конфигурации | Исходные настройки файла; не вся система профилей View Configuration |
| Текст | NAMEON/COMMENTON, отдельные font kind/bold/italic/inverted, зеркальность, вращение, размеры и сохранённые метрики; фиксированный масштаб раскладки | Нет точных глифов Altium Default/Sans Serif; embedded fonts/LettersGeometry и специальные стили не реализованы |
| Контуры | Направление обхода дуги по её вершине; безопасное рисование швов без бесконечного miter | Общая геометрия PCB/полигонов и synthetic GPU-mesh regression |
| Pad | Индивидуальный corner radius на каждом сигнальном слое; общий расчёт для renderer/pour/picking | Полные padstack shapes, slots/offsets и отдельные corner masks остаются ограничены |
| Вырезы и корпуса | Штриховка вырезов; сохранённые 2D-проекции обоих семейств ComponentBodies | Это проекция из PcbDoc, не STEP-рендерер и не редактор 3D-моделей |
| Дубли | Legacy и ShapeBased остаются в модели/архиве; alternate representation скрывается только при совпадении record index, свойств, владельца и геометрии с допуском ARCRESOLUTION | Проверены 57 пар Regions и 62 пары Bodies реальной платы |
| Отверстия | Отдельный проход сверления поверх меди; подписи площадок активного слоя и Multi-Layer | Via-span indicators и точный адаптивный выбор размера/расположения всех подписей ещё не совпадают |

После импорта сохраняются **15513** записей (раньше 15389: добавлены 124 записи
проекций, представляющие 62 корпуса в двух форматах), 69 компонентов, 20 цепей,
3 полигона. Парные записи не удваивают прозрачность на экране и не удаляются
из исходного формата. Native writer сохраняет неизменённые body streams;
изменение проекции блокирует native Save, пока не реализованы полные 3D transforms.

Радиус в процентах вычисляется от половины короткой стороны площадки;
100% даёт полное скругление. Это соответствует
[официальной документации Pads & Vias](https://www.altium.com/documentation/altium-designer/pcb/pads-vias).
Смещения полей font kind/bold/italic/barcode сверены с самостоятельным
[AltiumSharp PcbLibReader](https://github.com/issus/AltiumSharp/blob/master/src/OriginalCircuit.Altium/Serialization/Readers/PcbLibReader.cs).
Сторонний parser/renderer, шрифты или ядро Altium не включены в приложение.

## Проверка и оставшаяся работа

- 159 workspace tests; fmt --check; Clippy --all-targets -D warnings.
- Регрессии binary import/native сохранения modern layer fallback, непрозрачных
  хвостов и body streams; отказ от сохранения устаревшей 3D-модели.
- Геометрия glyph mesh сохраняется при zoom/rotation/mirror на DPI 100/150/200%.
- Проверены скрытые строки/fit, corner radius/picking, штриховка с отверстиями,
  направление дуг и отсутствие неограниченных треугольников на native seams.
- Windows debug build и реальное открытие платы; Linux GNU all-targets check.
  Linux GUI и Linux-артефакты в этой итерации не запускались.

Следующий обязательный шаг для полного совпадения: точные PCB stroke fonts,
native text styles/embedded glyph geometry, via-span markers и адаптивные подписи,
сверка толщин и штриховки при одинаковом масштабе, DPI и рабочем поле. Компоновка
PCB Properties, плавающая панель инструментов и layer bar также ещё отличаются.
Процент совпадения и pixel-perfect acceptance не заявляются.
