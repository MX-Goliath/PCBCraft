# PCBCraft JSON v1

Текущий пример: `fixtures/controller.pcbcraft.json`.

Документ содержит schema_version=1, UUID id, name, kind (Board/Schematic),
size {x,y}, pads и tracks. Координаты и размеры — i64 нанометры.
Допустимый диапазон каждой координаты +/- 1 000 000 000 нм.
Размеры документа и площадок положительны. Ширина tracks положительна.
ID объектов уникальны в документе. Ссылки на nets/footprints пока отсутствуют.
Неизвестные поля и версии отклоняются; данные не отбрасываются при сохранении.

Pad: id, name, position, size, layer.
Track: id, start, end, width, layer.
Layer: Top, Bottom, Overlay, Outline.

Загрузка ограничена 16 MiB. Перед записью модель валидируется;
временный файл создаётся в каталоге назначения, синхронизируется и
заменяет файл через tempfile::persist. Гарантия восстановления после
потери питания и directory fsync ещё не проверена.

Формат не совместим с PcbDoc/SchDoc; их импорт требует отдельного reader.
