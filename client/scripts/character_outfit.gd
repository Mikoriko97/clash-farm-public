class_name CharacterOutfit
extends RefCounted
## Вбрання персонажа (ADR-026; база 4,5 голови — ADR-032): восьмеро «скінних» оболонок з `assets/character/base45/outfits.glb` (інші тіла —
## `outfits_f.glb` тощо) (генератор — `client/blender/characters/build_outfits.py`). Оболонка — копія граней тіла бази,
## зсунута по нормалях, з тими самими вагами кісток, тож рухається з анімаціями тіла. Її меш беремо з GLB,
## а `Skin` будуємо самі з СПОКІЙНОГО положення кісток скелета гри (так само, як імпортне тіло: bind =
## обернений глобальний спокій кістки) — тож блендерні орієнтації кісток не впливають. Імена в GLB —
## імена Godot (BoneMap). Один слот `Outfit` на скелет; `role_cloth` фарбується з `outfit_color`.
## Вигляд надходить від React (`outfit: "outfit_jacket"`, порожній рядок = без вбрання).

## Вбрання кожного тіла (`CharacterBody.outfit_path_of`) — свій GLB, що вантажиться ліниво: оболонка копіює грані саме цього тіла.
const NODE_NAME := "Outfit"

static var _libraries: Dictionary = {}
static var _meshes: Dictionary = {}
static var _skins: Dictionary = {}


## Зразок вбрання (меш і skin) з GLB тіла з ключем `body` (для тестів і перевірки id); null, якщо такого нема.
## Бібліотека — словник id → `CharacterBody.Template`: екземпляр GLB звільняється одразу після розбору.
static func template(id: String, body: String = CharacterBody.MALE) -> CharacterBody.Template:
	if not id.begins_with("outfit_"):
		return null
	if not _libraries.has(body):
		var path := CharacterBody.outfit_path_of(body)
		var scene := ResourceLoader.load(path) as PackedScene if ResourceLoader.exists(path) else null
		_libraries[body] = CharacterBody.extract(scene, []) if scene != null else {}
	return (_libraries[body] as Dictionary).get(id) as CharacterBody.Template


## Id вбрання з вигляду: лише наявний у GLB тіла цього вигляду, інакше порожньо.
static func id_of(look: Variant) -> String:
	if typeof(look) != TYPE_DICTIONARY:
		return ""
	var raw: Variant = (look as Dictionary).get("outfit")
	if typeof(raw) == TYPE_STRING and template(String(raw), CharacterBody.key_of(look)) != null:
		return String(raw)
	return ""


## Одягає (чи роздягає) персонажа під `root` і фарбує. Безпечно викликати знову.
static func apply(root: Node, look: Variant, colors: Dictionary) -> void:
	var skeleton := root.find_child("Skeleton3D", true, false) as Skeleton3D
	if skeleton == null:
		return
	var id := id_of(look)
	var body := CharacterBody.key_of(look)
	var current := skeleton.get_node_or_null(NodePath(NODE_NAME)) as MeshInstance3D
	if current != null and (String(current.get_meta("part_id", "")) != id or String(current.get_meta("body", "")) != body):
		skeleton.remove_child(current)
		current.queue_free()
		current = null
	if current == null and not id.is_empty():
		var source := template(id, body)
		current = MeshInstance3D.new()
		current.name = NODE_NAME
		current.mesh = _mesh(body + "/" + id, source.mesh)
		current.set_meta("part_id", id)
		current.set_meta("body", body)
		current.skin = _skin(body + "/" + id, source.skin, skeleton)
		skeleton.add_child(current)
		current.skeleton = ^".."
	if current != null:
		CharacterParts.paint(current, colors)


## Меш вбрання з LOD імпорту (`MeshLod`; арматура бази в метрах, поправка не потрібна), один на ключ (тіло/id).
static func _mesh(key: String, source: Mesh) -> ArrayMesh:
	var cached: ArrayMesh = _meshes.get(key)
	if cached == null:
		cached = MeshLod.rebuilt(source, 1.0)
		_meshes[key] = cached
	return cached


## Skin з іменами кісток GLB і bind-позами зі спокою скелета гри (`CharacterBody.named_skin`), один на ключ.
static func _skin(key: String, source: Skin, skeleton: Skeleton3D) -> Skin:
	var cached: Skin = _skins.get(key)
	if cached == null:
		cached = CharacterBody.named_skin(source, skeleton)
		_skins[key] = cached
	return cached
