class_name FloraBake
extends Resource
## Запечена розстановка флори материка (docs/MAINLAND.md §9): для кожного чанка
## і кожного виду (індекс у `MainlandTerrain.flora`) — [x, z, yaw, scale, …]
## у світових метрах. Пише `tools/bake_flora.gd`, читає `MainlandTerrain`
## (висоту й нахил чанк добирає сам при побудові). Файл —
## `assets/mainland/flora.res`, лежить в експорті.

## Шляхи видів у порядку запікання — для звірки з масивом `flora` сцени.
@export var kinds := PackedStringArray()
## Vector2i(cx, cz) → { індекс виду: PackedFloat32Array }.
@export var chunks: Dictionary = {}
## Індекс виду → скільки екземплярів на всій карті.
@export var counts: Dictionary = {}
## Сід, з яким запікали (для повторюваності).
@export var seed: int = 0


func chunk_data(c: Vector2i, kind: int) -> PackedFloat32Array:
	var per: Variant = chunks.get(c)
	if typeof(per) != TYPE_DICTIONARY:
		return PackedFloat32Array()
	var arr: Variant = (per as Dictionary).get(kind)
	return arr if arr is PackedFloat32Array else PackedFloat32Array()
