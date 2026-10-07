extends SceneTree
## Headless test: quality tiers change the grass grid, the sun's shadow distance and
## MSAA; auto keeps the scene's own values and measures only real play; an unknown tier is ignored; the setting
## persists in user://settings.cfg; the island grass mask keeps blades off plots and
## outside the island; the mainland grass grid follows the character.
## Run: godot --headless --path client --script res://tests/test_quality.gd

var _failures := 0
var _frames := 0
var _island: Node3D


func _check(ok: bool, what: String) -> void:
	if not ok:
		_failures += 1
		push_error("FAIL: " + what)


func _init() -> void:
	# `--script` runs carry no autoloads: add the Quality singleton by hand, as the game has it.
	var quality: Node = load("res://scripts/quality.gd").new()
	quality.name = "Quality"
	root.add_child(quality)
	_island = (load("res://scenes/island/Island.tscn") as PackedScene).instantiate()
	root.add_child(_island)


func _process(_delta: float) -> bool:
	_frames += 1
	if _frames == 1:
		root.get_node(^"Quality").call("set_tier", "auto")
	if _frames == 30:
		_check_island()
		_check_tiers()
		print("RESULT: %s (%d failures)" % ["PASS" if _failures == 0 else "FAIL", _failures])
		quit(1 if _failures > 0 else 0)
		return true
	return false


func _check_island() -> void:
	var grass: GrassGpu = _island.get_node("GrassField")
	_check(grass.instance_count() > 10000, "island grass grid has instances: %d" % grass.instance_count())
	_check(grass.cast_shadow == GeometryInstance3D.SHADOW_CASTING_SETTING_OFF, "grass casts no shadow")
	_check(grass.custom_aabb.size.x > 40.0, "custom AABB covers the island: %s" % grass.custom_aabb)
	var mat := grass.material_override as ShaderMaterial
	_check(mat != null and int(mat.get_shader_parameter("mask_mode")) == 1, "island grass uses the occupancy mask")
	var mask: Image = (mat.get_shader_parameter("mask_tex") as ImageTexture).get_image()
	var half := GrassGpu.MASK_HALF_M
	_check(mask.get_pixel(half, half).r > 0.5, "island centre is free for grass")
	_check(mask.get_pixel(half + 60, half).r < 0.5, "outside the island no grass")
	var plots: PlotGrid = _island.get_node("PlotGrid")
	var taken := Vector2i(999, 999)
	for row: int in range(-4, 5):
		for col: int in range(-4, 5):
			if plots.cell_taken(col, row):
				taken = Vector2i(col, row)
	if taken.x != 999:
		var px := int(taken.x * PlotGrid.CELL_M) + half
		var pz := int(taken.y * PlotGrid.CELL_M) + half
		_check(mask.get_pixel(px, pz).r < 0.5, "no grass on a plot cell %s" % taken)
	else:
		print("no plots on the bare island: plot mask rule not exercised")


## View distance follows the tier on the mainland (streaming radii, grass radius).
func _check_view(quality: Node) -> void:
	var mainland: Node3D = (load("res://scenes/mainland/Mainland.tscn") as PackedScene).instantiate()
	root.add_child(mainland)
	var terrain: MainlandTerrain = mainland.get_node("Terrain")
	var grass: GrassGpu = mainland.get_node("Grass")
	quality.call("set_tier", "high")
	_check(terrain.load_radius == 5 and terrain.lod0_radius == 2 and terrain.unload_radius == 7, "high tier streams 5 chunks out: %d/%d/%d" % [terrain.load_radius, terrain.lod0_radius, terrain.unload_radius])
	_check(is_equal_approx(grass.radius_m, 45.0), "high tier grass radius 45 m: %.0f" % grass.radius_m)
	_check(grass.instance_count() < 200000, "high tier grass budget under 200k blades: %d" % grass.instance_count())
	quality.call("set_tier", "low")
	_check(terrain.load_radius == 2 and terrain.unload_radius >= 3, "low tier streams 2 chunks out")
	_check(is_equal_approx(grass.radius_m, 28.0), "low tier grass radius 28 m")
	quality.call("set_tier", "auto")
	mainland.queue_free()


func _check_tiers() -> void:
	var quality: Node = root.get_node(^"Quality")
	var grass: GrassGpu = _island.get_node("GrassField")
	var sun: DirectionalLight3D = _island.get_node("Sun")
	quality.call("set_tier", "high")
	var high_count := grass.instance_count()
	var scene_shadow := sun.directional_shadow_max_distance
	quality.call("set_tier", "low")
	_check(str(quality.get("effective")) == "low", "low tier applies")
	_check(grass.instance_count() < high_count * 0.5, "low tier thins the grass: %d < %d" % [grass.instance_count(), high_count])
	_check(sun.directional_shadow_max_distance < scene_shadow, "low tier shortens sun shadows: %.0f < %.0f" % [sun.directional_shadow_max_distance, scene_shadow])
	_check(root.msaa_3d == Viewport.MSAA_DISABLED, "low tier turns MSAA off")
	quality.call("set_tier", "high")
	_check(grass.instance_count() == high_count, "high tier restores the grass grid")
	_check(is_equal_approx(sun.directional_shadow_max_distance, scene_shadow), "high tier restores the scene's shadow distance")
	_check(root.msaa_3d == Viewport.MSAA_2X, "high tier uses 2x MSAA")
	quality.call("set_tier", "bogus")
	_check(str(quality.get("effective")) == "high", "unknown tier ignored")
	var cfg := ConfigFile.new()
	_check(cfg.load("user://settings.cfg") != OK or str(cfg.get_value("graphics", "tier", "")) != "bogus", "headless runs do not persist the tier")
	quality.call("set_tier", "auto")
	_check(str(quality.get("effective")) == "medium", "auto starts medium in headless (desktop picks high, phones medium)")
	_check_auto(quality)
	_check_view(quality)


## Auto measures only after the zone is ready, ignores throttled frames (hidden tab)
## and still steps down on genuinely slow ones. Frames are fed by hand.
func _feed(quality: Node, ms: float, count: int) -> void:
	for i: int in count:
		quality.call("_process", ms / 1000.0)


func _check_auto(quality: Node) -> void:
	quality.call("set_tier", "auto")
	quality.set("_armed", false)
	quality.set_process(false)
	_feed(quality, 40.0, 600)
	_check(str(quality.get("effective")) == "medium", "auto does not judge loading frames before the zone is ready")
	quality.call("arm")
	quality.set_process(false)
	_feed(quality, 1500.0, 50)
	_check(str(quality.get("effective")) == "medium", "resume frames of a background tab do not lower the tier")
	_feed(quality, 12.0, 400)
	_check(str(quality.get("effective")) == "medium", "fast frames keep the tier")
	quality.call("set_tier", "auto")
	quality.call("arm")
	quality.set_process(false)
	_feed(quality, 30.0, 160)
	_check(str(quality.get("effective")) == "low", "slow real frames step auto down within seconds: %s" % quality.get("effective"))
	# The bug of 2026-09-22: frames over 100 ms were thrown away, so a game running at
	# 5-10 FPS never stepped down. Now 8 FPS steps down after ~4.5 s of play.
	quality.call("set_tier", "high")
	quality.call("set_tier", "auto")
	quality.set("effective", "high")
	quality.call("arm")
	quality.set_process(false)
	_feed(quality, 125.0, 40)
	_check(str(quality.get("effective")) == "medium", "8 FPS steps high down to medium in under 5 s: %s" % quality.get("effective"))
	quality.call("set_tier", "auto")
