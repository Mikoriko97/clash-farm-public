extends SceneTree
## Headless test: a crop with `crop_stages` (wheat, five Blender-made models) shows
## the stage that matches its progress at scale 1 — sprout right after planting,
## a middle stage half way, the ripe model when ready — and swaps the instanced
## piece under every anchor when the stage changes; crops without stages keep
## scaling one piece as before; all 19 crops (with the 0027 tutorial radish) have five stages on shared materials.
## Run: godot --headless --path client --script res://tests/test_crop_stages.gd

var _failures := 0
var _frames := 0
var _plot: PlotNode


func _check(ok: bool, what: String) -> void:
	if not ok:
		_failures += 1
		push_error("FAIL: " + what)


func _init() -> void:
	_plot = (load("res://scenes/island/PlotNode.tscn") as PackedScene).instantiate()
	root.add_child(_plot)


func _process(_delta: float) -> bool:
	_frames += 1
	if _frames < 2:
		return false
	_run()
	print("RESULT: %s (%d failures)" % ["PASS" if _failures == 0 else "FAIL", _failures])
	quit(1 if _failures > 0 else 0)
	return true


func _piece_name() -> String:
	var anchor := _plot.get_node("Crop").get_child(0)
	return String(anchor.get_child(0).name) if anchor.get_child_count() > 0 else ""


func _apply(state: String, planted: float, ready: float) -> void:
	var now := Time.get_unix_time_from_system()
	_plot.apply({"idx": 0, "col": 0, "row": 0, "state": state, "crop_id": "seed_wheat",
		"planted_at": Time.get_datetime_string_from_unix_time(int(now + planted), true) + "Z",
		"ready_at": Time.get_datetime_string_from_unix_time(int(now + ready), true) + "Z"}, now, now)


func _run() -> void:
	_check((_plot.crop_stages.get("seed_wheat", []) as Array).size() == 5, "wheat has five stage scenes")
	_apply("growing", -10.0, 1000.0)
	_check(_piece_name().begins_with("wheat_s0"), "just planted shows the sprout: %s" % _piece_name())
	_check((_plot.get_node("Crop").get_child(0) as Node3D).scale == Vector3.ONE, "staged crops are not scaled")
	_apply("growing", -500.0, 500.0)
	_check(_piece_name().begins_with("wheat_s2"), "half way shows the third stage: %s" % _piece_name())
	_apply("growing", -950.0, 50.0)
	_check(_piece_name().begins_with("wheat_s3"), "almost ready shows heading, not ripe: %s" % _piece_name())
	_apply("ready", -1000.0, -1.0)
	_check(_piece_name().begins_with("wheat_s4"), "ready shows the ripe model: %s" % _piece_name())
	for anchor: Node in _plot.get_node("Crop").get_children():
		_check(anchor.get_child_count() == 1, "one piece per anchor after swaps: %d" % anchor.get_child_count())
	_check(_plot.stage_index(5) == 4 and _plot.stage_index(1) == 0, "stage index bounds")
	# A crop without stages keeps the old scaling path.
	var now := Time.get_unix_time_from_system()
	_plot.apply({"idx": 0, "col": 0, "row": 0, "state": "growing", "crop_id": "seed_unknown",
		"planted_at": Time.get_datetime_string_from_unix_time(int(now - 10), true) + "Z",
		"ready_at": Time.get_datetime_string_from_unix_time(int(now + 1000), true) + "Z"}, now, now)
	_check((_plot.get_node("Crop").get_child(0) as Node3D).scale.x < 0.5, "a crop without stages still grows by scale")
	_check_generated()


## The 17 Blender-made crops (blender/crops/make_crops.py): every catalog crop and every
## planned one has five stages; each loads, and its meshes use the shared crop materials
## (one atlas for all of them, tools/post_import_crops.gd).
func _check_generated() -> void:
	for seed: String in ["seed_wheat", "seed_carrot", "seed_tomato", "seed_pumpkin", "seed_clashberry"]:
		_check((_plot.crop_stages.get(seed, []) as Array).size() == 5, "catalog crop %s has five stages" % seed)
	_check(_plot.crop_stages.size() == 19, "19 crops with stages (radish since 0027): %d" % _plot.crop_stages.size())
	var shared := 0
	var surfaces := 0
	for seed: String in _plot.crop_stages:
		for scene: Variant in _plot.crop_stages[seed]:
			_check(scene is PackedScene, "%s stage is a scene" % seed)
			if seed == "seed_wheat" or not (scene is PackedScene):
				continue
			var node := (scene as PackedScene).instantiate()
			for m: Node in node.find_children("*", "MeshInstance3D", true, false):
				var mesh := (m as MeshInstance3D).mesh
				for i: int in mesh.get_surface_count():
					surfaces += 1
					var mat := mesh.surface_get_material(i)
					if mat != null and mat.resource_path.begins_with("res://materials/crops/"):
						shared += 1
			node.free()
	_check(surfaces > 0 and shared == surfaces, "every generated crop surface uses a shared crop material: %d/%d" % [shared, surfaces])
	var now := Time.get_unix_time_from_system()
	_plot.apply({"idx": 0, "col": 0, "row": 0, "state": "ready", "crop_id": "seed_pumpkin",
		"planted_at": Time.get_datetime_string_from_unix_time(int(now - 1000), true) + "Z",
		"ready_at": Time.get_datetime_string_from_unix_time(int(now - 1), true) + "Z"}, now, now)
	_check(_piece_name().begins_with("pumpkin_s4"), "a ripe pumpkin shows the ripe pumpkin model: %s" % _piece_name())
