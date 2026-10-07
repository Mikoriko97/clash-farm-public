class_name CharacterLook
extends RefCounted
## Вигляд персонажа (ADR-025; база 4,5 голови — ADR-032): перефарбовує тіло бази. Зони тіла лежать на клітинках
## першого ряду палітри (16 × 1): голова й шия, передпліччя й кисті — шкіра, груди з рукавами — сорочка, таз і ноги — штани,
## черевики — взуття. UV кожної грані тіла вже вказує на центр своєї клітинки (генератор `parts_base45.paint_zones`), тож
## тіло лишається як імпортоване (з LOD імпорту), а вигляд — крихітна текстура 16 × 1 (по кольору на клітинку): четвірка
## кольорів шкіра/сорочка/штани/взуття = один матеріал, а його ключ спільний для всіх однакових виглядів.
## Кольори приходять від React (`player_look`, `peers_state[].look`) як «#rrggbb»: таблиці
## кольорів і їх індекси живуть у `web/src/character/palette.ts`, Godot лише малює. Сервер
## зберігає індекси й не знає кольорів. Нічого невалідного не приймаємо: зіпсований колір =
## колір за замовчуванням.

const BODY_NODE := "Base45_Body"
const PALETTE_MATERIAL: Material = preload("res://materials/character_palette.tres")
const CELLS := 16
const ARM_SKIN_CELL := 2
const SKIN_CELLS: Array[int] = [14, 1, 0, 2, 3]
const SHIRT_CELLS: Array[int] = [13, 12]
const PANTS_CELLS: Array[int] = [11, 10, 9, 8]
const SHOE_CELLS: Array[int] = [7, 6, 5, 4]
## Слоти кольорів тіла (з них будується палітра); `hair_color` фарбує частини голови (`CharacterParts`).
const BODY_SLOTS: Array[String] = ["skin", "shirt", "pants", "shoes"]
const DEFAULTS: Dictionary = {
	"skin": Color("#d9a878"),
	"shirt": Color("#5f8f4a"),
	"pants": Color("#6b5a3e"),
	"shoes": Color("#3a2f28"),
	"hair_color": Color("#5a3a22"),
	"outfit_color": Color("#3d5a80"),
}

static var _materials: Dictionary = {}


## `look` → { skin, shirt, pants, shoes, hair_color, outfit_color } кольорами (+ `outfit_trim`, похідний); усе, що не «#rrggbb», — за замовчуванням.
static func colors_of(look: Variant) -> Dictionary:
	var out: Dictionary = DEFAULTS.duplicate()
	if typeof(look) != TYPE_DICTIONARY:
		return _trimmed(out)
	for slot: String in DEFAULTS:
		var raw: Variant = (look as Dictionary).get(slot)
		if typeof(raw) == TYPE_STRING and String(raw).length() == 7 and String(raw).is_valid_html_color():
			out[slot] = Color(String(raw))
	return _trimmed(out)


## Оздоблення вбрання — темніший відтінок його кольору.
static func _trimmed(colors: Dictionary) -> Dictionary:
	colors["outfit_trim"] = (colors["outfit_color"] as Color).darkened(0.3)
	return colors


## Фарбує тіло під `root` (сцена персонажа з `Base45_Body`; інші тіла — `CharacterBody`), ставить вираз обличчя, частини голови та вбрання. Безпечно викликати знову.
static func apply(root: Node, look: Variant) -> void:
	var body := CharacterBody.apply(root, look)
	if body == null:
		return
	var colors := colors_of(look)
	body.set_surface_override_material(0, _material(colors))
	CharacterFace.apply(root, look)
	CharacterParts.apply(root, look, colors)
	CharacterOutfit.apply(root, look, colors)


## З'єднує сцену персонажа з мостом: нове `player_look` перефарбовує її на місці.
static func on_bridge(type: String, payload: Dictionary, root: Node) -> void:
	if type == "player_look" and is_instance_valid(root):
		apply(root, payload.get("look"))


static func key_of(colors: Dictionary) -> String:
	var parts: PackedStringArray = []
	for slot in BODY_SLOTS:
		parts.append((colors[slot] as Color).to_html(false))
	return ",".join(parts)


static func _material(colors: Dictionary) -> Material:
	var key := key_of(colors)
	var cached: Material = _materials.get(key)
	if cached != null:
		return cached
	var image := Image.create(CELLS, 1, false, Image.FORMAT_RGBA8)
	for cell in CELLS:
		image.set_pixel(cell, 0, _cell_color(cell, colors))
	var material := PALETTE_MATERIAL.duplicate() as StandardMaterial3D
	material.albedo_texture = ImageTexture.create_from_image(image)
	_materials[key] = material
	return material


static func _cell_color(cell: int, colors: Dictionary) -> Color:
	if cell in SHIRT_CELLS:
		return colors["shirt"]
	if cell in PANTS_CELLS:
		return colors["pants"]
	if cell in SHOE_CELLS:
		return colors["shoes"]
	return colors["skin"]
