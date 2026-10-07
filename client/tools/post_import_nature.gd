@tool
extends EditorScenePostImport
## Пост-імпорт шматків паку рослинності (`assets/nature/pieces/*.glb`,
## docs/ASSETS_NATURE.md): кожен GLB несе власну копію матеріалу
## `AllColors_Material` з блендом (Godot імпортує як alpha з depth pre-pass —
## два проходи, сортування прозорого, окремий шейдер на кожен файл). Тут усі
## поверхні отримують спільний непрозорий матеріал з тією ж палітрою: один
## шейдер, одна текстура. Живе (дерева, кущі, квіти, трави, папороть, гілки)
## бере `nature_wind_*` з вітром (дерева гойдаються повільно, дрібне — швидко),
## каміння, пні, колоди й гриби — статичний `nature_palette`. Дрібне (нижче
## `LOW_M`) не кидає тінь. Підключається у `<файл>.glb.import` рядком
## `import_script/path="res://tools/post_import_nature.gd"`.

const PALETTE := "res://materials/nature_palette.tres"
const WIND_TREE := "res://materials/nature_wind_tree.tres"
const WIND_PLANT := "res://materials/nature_wind_plant.tres"
const LOW_M := 0.8
const ALIVE := ["tree", "bush", "flower", "grass", "branch"]


func _post_import(scene: Node) -> Object:
	var palette: Material = load(PALETTE)
	var tree: Material = load(WIND_TREE)
	var plant: Material = load(WIND_PLANT)
	if palette == null or tree == null or plant == null:
		push_error("post_import_nature: shared materials missing")
		return scene
	var name := get_source_file().get_file().get_basename()
	var alive := false
	for prefix: String in ALIVE:
		if name.begins_with(prefix):
			alive = true
	_fix(scene, palette, tree if name.begins_with("tree") else plant, alive)
	return scene


func _fix(node: Node, palette: Material, wind: Material, alive: bool) -> void:
	if node is MeshInstance3D:
		var mi := node as MeshInstance3D
		if mi.mesh != null:
			for s: int in mi.mesh.get_surface_count():
				mi.mesh.surface_set_material(s, wind if alive else palette)
			if mi.mesh.get_aabb().size.y < LOW_M:
				mi.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	for child: Node in node.get_children():
		_fix(child, palette, wind, alive)
