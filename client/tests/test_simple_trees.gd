extends SceneTree
## Headless test: every Simple Trees piece imports with three levels of detail and a
## shadow-only crown, the shared bark/leaves/far materials, distance switching and a
## sane size, and the flora lookup picks the requested level.
## Run: godot --headless --path client --script res://tests/test_simple_trees.gd

const PieceMesh := preload("res://scripts/piece_mesh.gd")
const DIR := "res://assets/simple_trees/pieces"
const LODS := ["lod0_near", "lod1_mid", "lod2_far"]
const SHARED := ["res://materials/simple_trees/bark.tres", "res://materials/simple_trees/leaves.tres", "res://materials/simple_trees/far.tres"]

var _failures := 0


func _check(ok: bool, what: String) -> void:
	if not ok:
		_failures += 1
		push_error("FAIL: " + what)


func _init() -> void:
	var names := _pieces()
	_check(names.size() == 17, "17 pieces (9 trees, 8 bushes): %d" % names.size())
	for file: String in names:
		_check_piece(file)
	_check_lookup()
	print("RESULT: %s (%d failures)" % ["PASS" if _failures == 0 else "FAIL", _failures])
	quit(1 if _failures > 0 else 0)


func _pieces() -> PackedStringArray:
	var out := PackedStringArray()
	for file: String in DirAccess.get_files_at(DIR):
		if file.ends_with(".glb"):
			out.append(file)
	return out


func _check_piece(file: String) -> void:
	var scene: PackedScene = load("%s/%s" % [DIR, file])
	_check(scene != null, "%s loads" % file)
	if scene == null:
		return
	var root := scene.instantiate()
	var meshes: Array[Node] = root.find_children("*", "MeshInstance3D", true, false)
	_check(meshes.size() == 4, "%s has three levels of detail and a shadow crown: %d" % [file, meshes.size()])
	var box := AABB()
	for node: Node in meshes:
		var mi := node as MeshInstance3D
		if String(mi.name).begins_with("shadow_crown"):
			_check(mi.cast_shadow == GeometryInstance3D.SHADOW_CASTING_SETTING_SHADOWS_ONLY, "%s shadow crown only casts shadows" % file)
			_check(mi.visibility_range_end == 0.0, "%s shadow crown has no distance cut" % file)
			_check(mi.mesh.get_faces().size() / 3 <= 700, "%s shadow crown is light: %d" % [file, mi.mesh.get_faces().size() / 3])
			continue
		var level := ""
		for key: String in LODS:
			if String(mi.name).begins_with(key):
				level = key
		_check(not level.is_empty(), "%s: mesh %s is a named level" % [file, mi.name])
		_check(not String(mi.name).begins_with("UCX"), "%s: no collision hull left" % file)
		for s: int in mi.mesh.get_surface_count():
			var m := mi.mesh.surface_get_material(s)
			_check(m != null and m.resource_path in SHARED, "%s/%s surface %d uses a shared material: %s" % [file, mi.name, s, m.resource_path if m != null else "null"])
		match level:
			"lod0_near":
				_check(mi.visibility_range_begin == 0.0 and mi.visibility_range_end > 0.0, "%s near level ends at a distance" % file)
			"lod1_mid":
				_check(mi.visibility_range_begin > 0.0 and mi.visibility_range_end > mi.visibility_range_begin, "%s mid level has a band" % file)
			"lod2_far":
				_check(mi.visibility_range_begin > 0.0 and mi.visibility_range_end == 0.0, "%s far level starts at a distance" % file)
				_check(mi.cast_shadow == GeometryInstance3D.SHADOW_CASTING_SETTING_OFF, "%s far level casts no shadow" % file)
		box = mi.get_aabb() if box.size == Vector3.ZERO else box.merge(mi.get_aabb())
	_check(box.size.y > 1.2 and box.size.y < 11.0, "%s height in metres: %.2f" % [file, box.size.y])
	_check(box.position.y > -1.0 and box.position.y < 0.05, "%s base sits at the origin: %.2f" % [file, box.position.y])
	root.free()


func _check_lookup() -> void:
	var scene: PackedScene = load("%s/tree_simple_03_a.glb" % DIR)
	var mid: Dictionary = PieceMesh.first(scene, "mid")
	var far: Dictionary = PieceMesh.first(scene, "far")
	var plain: Dictionary = PieceMesh.first(scene)
	_check(mid.get("mesh") != null and far.get("mesh") != null and mid["mesh"] != far["mesh"], "lookup gives different meshes for mid and far")
	_check(plain.get("mesh") != null and plain["mesh"] != mid["mesh"], "no hint gives the near level (first mesh)")
	_check(bool(PieceMesh.first(scene, "shadow_crown").get("matched", false)), "shadow crown is found by name")
	var old: PackedScene = load("res://assets/nature/pieces/stone_big_01.glb")
	_check(PieceMesh.first(old, "far").get("mesh") != null, "single-mesh pieces ignore the hint")
	_check(not bool(PieceMesh.first(old, "shadow_crown").get("matched", false)), "old pieces have no shadow crown")
