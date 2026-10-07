extends SceneTree
## Headless test: every decor item of the server catalog (`server/migrations/*.sql`, rows `('decor_…', 'decor'`) has a prop
## scene in `DecorItem.tscn`, each scene loads, stands on the ground (lowest point ≈ 0) and is not bigger than the footprint
## the catalog reserves for it (docs/CONTENT_V2.md: the paid «well» used to be a 15 m wind pump). No engine errors.
## Run: godot --headless --path client --script res://tests/test_decor_catalog.gd

const ErrorLog := preload("res://tests/error_log.gd")

var _failures := 0
var _log := ErrorLog.new()


var _frames := 0


## Межі сіток рахуються лише після першого кадру в дереві (у `_initialize` глобальні перетворення ще не готові).
func _process(_delta: float) -> bool:
	_frames += 1
	if _frames == 2:
		_run()
	return false


func _run() -> void:
	OS.add_logger(_log)
	var catalog := _catalog()
	_check(catalog.size() >= 37, "the migrations list the decor catalog (%d items)" % catalog.size())
	var decor: Node = (load("res://scenes/island/DecorItem.tscn") as PackedScene).instantiate()
	var pieces: Dictionary = decor.get("pieces")
	for id: String in catalog:
		var scene := pieces.get(id) as PackedScene
		_check(scene != null, "%s has a prop scene in DecorItem.tscn" % id)
		if scene == null:
			continue
		var node := scene.instantiate() as Node3D
		root.add_child(node)
		var box := _box(node)
		_check(absf(box.position.y) < 0.12, "%s stands on the ground (lowest point %.2f m)" % [id, box.position.y])
		# Radius the catalog reserves vs the model's half-width (a little overhang is fine, a 15 m pump is not).
		var radius_m := float(catalog[id]) / 10.0
		var half := maxf(box.size.x, box.size.z) * 0.5
		_check(half <= radius_m * 1.6 + 0.3, "%s fits its footprint: half-width %.2f m, reserved %.2f m" % [id, half, radius_m])
		node.free()
	decor.free()
	OS.remove_logger(_log)
	_check(_log.errors.is_empty(), "no engine errors: %s" % "; ".join(_log.errors.slice(0, 3)))
	print("RESULT: %s (%d failures)" % ["PASS" if _failures == 0 else "FAIL", _failures])
	quit(1 if _failures > 0 else 0)


## decor id -> footprint_dm from every migration (later UPDATEs of footprint_dm win).
func _catalog() -> Dictionary:
	var out := {}
	var dir := ProjectSettings.globalize_path("res://").path_join("../server/migrations")
	var files := DirAccess.get_files_at(dir)
	files.sort()
	var row := RegEx.create_from_string(r"\('(decor_[a-z0-9_]+)',\s*'decor'[^\n]*?" + "\"footprint_dm\"" + r":\s*(\d+)")
	var bare := RegEx.create_from_string(r"\('(decor_[a-z0-9_]+)',\s*'decor'")
	var update := RegEx.create_from_string("\"footprint_dm\"" + r":\s*(\d+)\}'::jsonb WHERE id = '(decor_[a-z0-9_]+)'")
	for file: String in files:
		if not file.ends_with(".sql"):
			continue
		var text := FileAccess.get_file_as_string(dir.path_join(file))
		for m: RegExMatch in bare.search_all(text):
			if not out.has(m.get_string(1)):
				out[m.get_string(1)] = 10
		for m: RegExMatch in row.search_all(text):
			out[m.get_string(1)] = int(m.get_string(2))
		for m: RegExMatch in update.search_all(text):
			out[m.get_string(2)] = int(m.get_string(1))
	return out


func _box(node: Node3D) -> AABB:
	var box := AABB()
	var first := true
	for mesh: Node in node.find_children("*", "MeshInstance3D", true, false):
		var m := mesh as MeshInstance3D
		if m.mesh == null or m.visibility_range_begin > 0.0 or not m.is_visible_in_tree():
			continue
		var b := m.global_transform * m.get_aabb()
		box = b if first else box.merge(b)
		first = false
	return box


func _check(ok: bool, what: String) -> void:
	if not ok:
		_failures += 1
		push_error("FAIL: " + what)
