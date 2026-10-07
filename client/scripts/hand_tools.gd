class_name HandTools
extends Node3D
## Інструменти в правій руці персонажа (ADR-034): сокира, кайло, мотика, коса, лопата, молоток, лійка з
## `assets/character/base45/tools45.glb` (генератор `client/blender/characters/build_tool_anims.py`). Кожен — меш із вагою 1 на кістці
## `RightHand`, хват (держак крізь кулак) задано в Blender у спокійній позі, тож у кліпах дій (рубання, удар кайлом, копання,
## косіння, забивання, полив — `character_anims.tres`) інструмент сидить у кулаці так, як задумано, а друга рука лягає на держак.
## Вузол — дитина `Skeleton3D` у сцені персонажа; меші створює сам (`Skin` зі спокою скелета гри, як у вбрання). Який інструмент
## показати, вирішує той, хто викликає (таблиця FX у island_input чи `tool` сервера на материку); тут лише показ і сховання.

const LIBRARY: PackedScene = preload("res://assets/character/base45/tools45.glb")
const TOOLS: Array[String] = ["axe", "pickaxe", "hoe", "scythe", "spade", "hammer", "can"]

static var _library: Node


## Меш-зразок інструмента з GLB (вузол `tool_<id>`); null, якщо такого нема.
static func template(id: String) -> MeshInstance3D:
	if _library == null:
		_library = LIBRARY.instantiate()
	return _library.find_child("tool_" + id, true, false) as MeshInstance3D


func _ready() -> void:
	var skeleton := get_parent() as Skeleton3D
	if skeleton == null:
		push_warning("HandTools: parent is not a Skeleton3D")
		return
	for id: String in TOOLS:
		var source := template(id)
		if source == null or get_node_or_null(NodePath(id)) != null:
			continue
		var mesh := MeshInstance3D.new()
		mesh.name = id
		mesh.mesh = source.mesh
		mesh.skin = CharacterBody.named_skin(source.skin, skeleton)
		add_child(mesh)
		mesh.skeleton = mesh.get_path_to(skeleton)
	hide_all()


## Shows `tool_id` (a child node name) and hides the rest; unknown ids hide everything.
func show_tool(tool_id: String) -> void:
	for child: Node in get_children():
		if child is Node3D:
			(child as Node3D).visible = child.name == tool_id
	if tool_id != "" and get_node_or_null(NodePath(tool_id)) == null:
		push_warning("HandTools: no tool %s" % tool_id)


func hide_all() -> void:
	for child: Node in get_children():
		if child is Node3D:
			(child as Node3D).visible = false


func shown() -> String:
	for child: Node in get_children():
		if child is Node3D and (child as Node3D).visible:
			return child.name
	return ""
