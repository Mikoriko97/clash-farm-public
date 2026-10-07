@tool
extends EditorScenePostImport
## Пост-імпорт культур грядки (`assets/crops/<культура>_s0..s4.glb`, їх пише
## `blender/crops/make_crops.py`). У GLB порожні матеріали `crop_cards` (листя, квіти,
## колоски — картки з альфою) і `crop_solid` (плоди, стебла); тут обидві ролі отримують
## спільні матеріали `materials/crops/*.tres` з одним атласом `assets/crops/crops_atlas.png`,
## тож на весь набір із 85 моделей у паку одна текстура.
## Підключається у `<файл>.glb.import` рядком
## `import_script/path="res://tools/post_import_crops.gd"`.

const CARDS := "res://materials/crops/crop_cards.tres"
const SOLID := "res://materials/crops/crop_solid.tres"


func _post_import(scene: Node) -> Object:
	var cards: Material = load(CARDS)
	var solid: Material = load(SOLID)
	if cards == null or solid == null:
		push_error("post_import_crops: shared crop materials missing")
		return scene
	_fix(scene, cards, solid)
	return scene


func _fix(node: Node, cards: Material, solid: Material) -> void:
	if node is MeshInstance3D:
		var mesh := (node as MeshInstance3D).mesh
		if mesh != null:
			for s: int in mesh.get_surface_count():
				var current := mesh.surface_get_material(s)
				var role := current.resource_name if current != null else ""
				mesh.surface_set_material(s, solid if role.contains("solid") else cards)
	for child: Node in node.get_children():
		_fix(child, cards, solid)
