@tool
extends EditorScenePostImport
## Пост-імпорт моделей Simple Trees (`assets/simple_trees/pieces/*.glb`, їх пише
## `tools/split_simple_trees.py`, docs/ASSETS_NATURE.md). У GLB три рівні
## деталізації — `lod0_near`, `lod1_mid`, `lod2_far` — і порожні матеріали
## `bark` / `leaves` / `far`. Тут:
##   - поверхні отримують спільні матеріали `materials/simple_trees/*.tres`
##     (одна текстура листя і одна кори на весь пак);
##   - кожен рівень отримує дальність видимості, тож поодинокий екземпляр (вузол
##     ресурсу, дерево острова) сам перемикає LOD; флора чанків материка бере меш
##     потрібного рівня напряму (`MainlandTerrain.piece_mesh(scene, "mid"/"far")`);
##   - далекий рівень не кидає тінь;
##   - `shadow_crown` (гладка крона з метаболів, `split_simple_trees.py`) лише кидає
##     тінь: листя-картки в прохіді тіні сонця згортаються (`foliage_cards.gdshader`),
##     бо дірки в картках, що гойдаються, мерехтіли на карті тіней.
## Підключається у `<файл>.glb.import` рядком
## `import_script/path="res://tools/post_import_simple_trees.gd"`.

const MATERIALS := {
	"bark": "res://materials/simple_trees/bark.tres",
	"leaves": "res://materials/simple_trees/leaves.tres",
	"far": "res://materials/simple_trees/far.tres",
}
## Рівень → (початок, кінець) видимості в метрах; 0 — без межі.
const RANGES := {
	"lod0_near": Vector2(0.0, 35.0),
	"lod1_mid": Vector2(35.0, 100.0),
	"lod2_far": Vector2(100.0, 0.0),
}
const MARGIN_M := 6.0


func _post_import(scene: Node) -> Object:
	var materials := {}
	for key: String in MATERIALS:
		materials[key] = load(MATERIALS[key])
		if materials[key] == null:
			push_error("post_import_simple_trees: %s missing" % MATERIALS[key])
			return scene
	_fix(scene, materials)
	return scene


func _fix(node: Node, materials: Dictionary) -> void:
	if node is MeshInstance3D:
		var mi := node as MeshInstance3D
		if mi.mesh != null:
			for s: int in mi.mesh.get_surface_count():
				var current := mi.mesh.surface_get_material(s)
				mi.mesh.surface_set_material(s, materials[_role(current.resource_name if current != null else "")])
		for key: String in RANGES:
			if String(mi.name).begins_with(key):
				var r: Vector2 = RANGES[key]
				mi.visibility_range_begin = r.x
				mi.visibility_range_end = r.y
				mi.visibility_range_begin_margin = MARGIN_M if r.x > 0.0 else 0.0
				mi.visibility_range_end_margin = MARGIN_M if r.y > 0.0 else 0.0
		if String(mi.name).begins_with("lod2_far"):
			mi.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
		if String(mi.name).begins_with("shadow_crown"):
			mi.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_SHADOWS_ONLY
	for child: Node in node.get_children():
		_fix(child, materials)


static func _role(material_name: String) -> String:
	var n := material_name.to_lower()
	if n.contains("bark"):
		return "bark"
	if n.contains("far"):
		return "far"
	return "leaves"
