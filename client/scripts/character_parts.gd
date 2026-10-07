class_name CharacterParts
extends RefCounted
## Частини голови персонажа (ADR-025, етап 3; база 4,5 голови — ADR-032): зачіска, борода, головний убор, окуляри з
## `assets/character/base45/head_parts.glb` (генератор — `client/blender/characters/build_head_parts.py`; обличчя — атлас,
## `CharacterFace`). Кожна частина — вузол GLB з іменем на кшталт `hair_short`; вона будувалась у координатах спокійної пози бази,
## тож стає на кістку голови через `BoneAttachment3D` із «монтажем» = обернений спокійний трансформ
## кістки. Матеріали `role_hair` і `role_skin` перефарбовуються з вигляду гравця, решта мають запечений
## колір. Кожен слот тримає рівно одну частину; невідомий id (чи порожній) = слот порожній.
## Вигляд надходить від React разом із кольорами (`CharacterLook`); Godot нічого не вирішує.

const LIBRARY: PackedScene = preload("res://assets/character/base45/head_parts.glb")
const HEAD_BONE := "Head"
const ATTACHMENT := "HeadParts"
const SLOTS: Array[String] = ["hair", "beard", "hat", "glasses"]
## Матеріал → слот кольору у виглядові.
const ROLES: Dictionary = {"role_hair": "hair_color", "role_skin": "skin", "role_cloth": "outfit_color", "role_trim": "outfit_trim"}
## Усі слоти за замовчуванням порожні (обличчя — атлас бази, `CharacterFace`).
const DEFAULT_IDS: Dictionary = {}
## Під шапкою з полями чи тулією об'ємна зачіска не вміщається й пробиває її: її замінює коротка
## (шапка «приплющує» волосся). Вінок і пов'язка лежать низько й пускають будь-яку зачіску.
const FLATTENING_HATS: Array[String] = ["hat_straw", "hat_cap", "hat_beanie", "hat_cowboy", "hat_bucket"]
const BULKY_HAIR: Array[String] = ["hair_bun", "hair_mohawk", "hair_spiky", "hair_afro"]
const FLAT_HAIR := "hair_short"

## Дрібні деталі здалеку не видно, а їхні draw call'и й тіні (по каскадах) коштують: `range` — метрів від
## камери, далі не малюються; `shadow` — чи відкидають тінь. Волосся й капелюхи лишаються (силует, тінь від крисів).
const DETAIL: Dictionary = {
	"beard": {"range": 40.0, "shadow": false},
	"glasses": {"range": 32.0, "shadow": false},
}

static var _library: Node
static var _materials: Dictionary = {}


## Вузол-зразок частини з GLB (для тестів і перевірки id); null, якщо такої нема.
static func template(id: String) -> Node3D:
	if _library == null:
		_library = LIBRARY.instantiate()
	return _library.get_node_or_null(NodePath(id)) as Node3D


## Слот → id частини за виглядом: невалідне значення відкидається, обличчя за замовчуванням.
static func ids_of(look: Variant) -> Dictionary:
	var out: Dictionary = {}
	var source: Dictionary = look as Dictionary if typeof(look) == TYPE_DICTIONARY else {}
	for slot in SLOTS:
		var raw: Variant = source.get(slot)
		var id: String = DEFAULT_IDS.get(slot, "")
		if typeof(raw) == TYPE_STRING:
			var text := String(raw)
			if text.is_empty():
				id = ""
			elif text.begins_with(slot + "_") and template(text) != null:
				id = text
		out[slot] = id
	if String(out["hat"]) in FLATTENING_HATS and String(out["hair"]) in BULKY_HAIR:
		out["hair"] = FLAT_HAIR
	return out


## Ставить частини на голову під `root` (сцена персонажа зі `Skeleton3D`) і фарбує їх. Безпечно викликати знову.
static func apply(root: Node, look: Variant, colors: Dictionary) -> void:
	var skeleton := root.find_child("Skeleton3D", true, false) as Skeleton3D
	if skeleton == null:
		return
	var mount := _mount(skeleton)
	if mount == null:
		return
	var ids := ids_of(look)
	for slot in SLOTS:
		var id: String = ids[slot]
		var current := mount.get_node_or_null(NodePath(slot)) as Node3D
		if current != null and String(current.get_meta("part_id", "")) != id:
			mount.remove_child(current)
			current.queue_free()
			current = null
		if current == null and not id.is_empty():
			current = template(id).duplicate() as Node3D
			current.name = slot
			current.set_meta("part_id", id)
			_detail(current, slot)
			mount.add_child(current)
		if current != null:
			paint(current, colors)


static func _detail(part: Node3D, slot: String) -> void:
	var spec: Dictionary = DETAIL.get(slot, {})
	if spec.is_empty():
		return
	for mesh_node in meshes_of(part):
		mesh_node.visibility_range_end = float(spec["range"])
		if not bool(spec["shadow"]):
			mesh_node.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF


static func _mount(skeleton: Skeleton3D) -> Node3D:
	var attachment := skeleton.get_node_or_null(NodePath(ATTACHMENT)) as BoneAttachment3D
	if attachment == null:
		var bone := skeleton.find_bone(HEAD_BONE)
		if bone < 0:
			return null
		attachment = BoneAttachment3D.new()
		attachment.name = ATTACHMENT
		attachment.bone_name = HEAD_BONE
		skeleton.add_child(attachment)
		var mount := Node3D.new()
		mount.name = "Mount"
		# Частини лежать у координатах спокійної пози: кістка вже рухає їх, тож віднімаємо її спокій.
		mount.transform = skeleton.get_bone_global_rest(bone).affine_inverse()
		attachment.add_child(mount)
	return attachment.get_node_or_null(^"Mount") as Node3D


## Частина з GLB — сам `MeshInstance3D` без нащадків (`find_children` шукає лише серед нащадків).
static func meshes_of(part: Node) -> Array[MeshInstance3D]:
	var out: Array[MeshInstance3D] = []
	if part is MeshInstance3D:
		out.append(part as MeshInstance3D)
	for node in part.find_children("*", "MeshInstance3D", true, false):
		out.append(node as MeshInstance3D)
	return out


## Перефарбовує матеріали-ролі частини (`role_hair`, `role_skin`, `role_cloth`, `role_trim`) кольорами вигляду.
static func paint(part: Node3D, colors: Dictionary) -> void:
	for mesh_node in meshes_of(part):
		for surface in mesh_node.mesh.get_surface_count():
			var source := mesh_node.mesh.surface_get_material(surface)
			var role: String = ROLES.get(source.resource_name, "") if source != null else ""
			if role.is_empty():
				mesh_node.set_surface_override_material(surface, null)
			elif colors.has(role):
				mesh_node.set_surface_override_material(surface, _material(source, colors[role] as Color))


static func _material(source: Material, color: Color) -> Material:
	var key := "%s|%s" % [source.resource_name, color.to_html(false)]
	var cached: Material = _materials.get(key)
	if cached != null:
		return cached
	var material := (source as StandardMaterial3D).duplicate() as StandardMaterial3D
	material.albedo_color = color
	material.resource_name = key
	_materials[key] = material
	return material
