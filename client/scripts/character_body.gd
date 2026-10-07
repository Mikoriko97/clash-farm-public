class_name CharacterBody
extends RefCounted
## Тіло персонажа (ADR-027 стать, ADR-029 статура; база 4,5 голови — ADR-032): `look.body` (`body_m` | `body_f`) і `look.build`
## (`build_avg` | `build_slim` | `build_sturdy`) разом дають ключ тіла — `body_m`, `body_f`, `body_m_slim`, `body_f_sturdy`, … Чоловіче
## середнє — `Base45_Body` зі сцени (`base45.glb`); решта — GLB `assets/character/base45/<ключ>.glb` (генератор
## `client/blender/characters/build_bodies.py`: та сама топологія, UV, ваги й скелет, лише інша форма тулуба й кінцівок; голова, кисті
## й стопи ті самі). Тож анімації, BoneMap, розкладка палітри
## (`CharacterLook`) і частини голови спільні, а кожне інше тіло — ще один `MeshInstance3D` на тому ж скелеті (створюється при
## першій потребі, GLB вантажиться ліниво): активне видиме, решта сховані. Вбрання кожного тіла — свій набір (`outfit_path_of`).
## Вигляд надходить від React (`body: "body_f"`, `build: "build_slim"`); відсутнє чи невідоме — чоловіче, середня статура.

const MALE := "body_m"
const FEMALE := "body_f"
const MALE_NODE := "Base45_Body"
## Суфікс ключа за статурою (середня — без суфікса).
const BUILDS: Dictionary = {"build_avg": "", "build_slim": "_slim", "build_sturdy": "_sturdy"}
const PARTS_DIR := "res://assets/character/base45/"

static var _templates: Dictionary = {}


## Що беремо з GLB-зразка: меш і skin. Самі вузли GLB звільняються одразу (раніше лишалися сиротами назавжди).
class Template:
	var mesh: Mesh
	var skin: Skin

	func _init(source: MeshInstance3D) -> void:
		mesh = source.mesh
		skin = source.skin


## Ключ тіла за виглядом; тіло без GLB (нема файлу) — найближче існуюче (спершу без статури, потім чоловіче).
static func key_of(look: Variant) -> String:
	var sex := MALE
	var suffix := ""
	if typeof(look) == TYPE_DICTIONARY:
		var dict := look as Dictionary
		if typeof(dict.get("body")) == TYPE_STRING and String(dict["body"]) == FEMALE:
			sex = FEMALE
		if typeof(dict.get("build")) == TYPE_STRING and BUILDS.has(String(dict["build"])):
			suffix = BUILDS[String(dict["build"])]
	for key: String in [sex + suffix, sex, MALE]:
		if key == MALE or ResourceLoader.exists(path_of(key)):
			return key
	return MALE


static func path_of(key: String) -> String:
	return PARTS_DIR + key + ".glb"


## Набір вбрання тіла `key`: `body_m` → `outfits.glb`, `body_f` → `outfits_f.glb`, `body_m_slim` → `outfits_m_slim.glb`.
static func outfit_path_of(key: String) -> String:
	return PARTS_DIR + "outfits" + ("" if key == MALE else key.trim_prefix("body")) + ".glb"


## Зразок тіла з GLB (меш з оригінальними UV); null для чоловічого середнього (воно в сцені) чи якщо GLB нема.
static func template(key: String) -> Template:
	if key == MALE or not ResourceLoader.exists(path_of(key)):
		return null
	if not _templates.has(key):
		var scene := ResourceLoader.load(path_of(key)) as PackedScene
		var found := extract(scene, [key]) if scene != null else {}
		_templates[key] = found.get(key)
	return _templates[key] as Template


## Зразки названих `MeshInstance3D` зі сцени (імʼя → Template; `names` порожній — усі); екземпляр сцени звільняється тут же.
static func extract(scene: PackedScene, names: Array) -> Dictionary:
	var out := {}
	var root := scene.instantiate()
	for node: Node in root.find_children("*", "MeshInstance3D", true, false):
		if (names.is_empty() or names.has(String(node.name))) and not out.has(String(node.name)):  # first match, as find_child
			out[String(node.name)] = Template.new(node as MeshInstance3D)
	root.free()
	return out


## Ставить потрібне тіло під `root` (чоловіче середнє вже у сцені, решта додаються на скелет при першій потребі), ховає інші й
## повертає активний `MeshInstance3D` (null, якщо в сцені нема чоловічого тіла). Безпечно викликати знову.
static func apply(root: Node, look: Variant) -> MeshInstance3D:
	var male := root.find_child(MALE_NODE, true, false) as MeshInstance3D
	var skeleton := root.find_child("Skeleton3D", true, false) as Skeleton3D
	if male == null or skeleton == null:
		return male
	var key := key_of(look)
	var active := male
	if key != MALE:
		active = skeleton.get_node_or_null(NodePath(key)) as MeshInstance3D
		if active == null:
			active = _make(key, skeleton)
	for child in skeleton.get_children():
		if child is MeshInstance3D and (child == male or String(child.name).begins_with("body_")):
			(child as MeshInstance3D).visible = child == active
	return active


static func _make(key: String, skeleton: Skeleton3D) -> MeshInstance3D:
	var source := template(key)
	var node := MeshInstance3D.new()
	node.name = key
	node.mesh = source.mesh
	node.skin = named_skin(source.skin, skeleton)
	skeleton.add_child(node)
	node.skeleton = ^".."
	return node


## Skin з іменами кісток GLB і bind-позами зі спокою скелета гри (bind = обернений глобальний спокій кістки — так само,
## як в імпортного тіла; кістка, якої в грі нема, — одинична). Блендерні орієнтації кісток не впливають.
static func named_skin(source: Skin, skeleton: Skeleton3D) -> Skin:
	var skin := Skin.new()
	for i in source.get_bind_count():
		var bone_name := source.get_bind_name(i)
		var bone := skeleton.find_bone(bone_name)
		var bind := skeleton.get_bone_global_rest(bone).affine_inverse() if bone >= 0 else Transform3D.IDENTITY
		skin.add_named_bind(bone_name, bind)
	return skin
