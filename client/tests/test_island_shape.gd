extends SceneTree
## Headless test: IslandShape turns world_state.expand into a rectangle that the
## ground mesh, its collision, WorldBounds, NavGrid and Placer follow; the
## rounded-rectangle clamp keeps points inside asymmetric extents.
## Run: godot --headless --path client --script res://tests/test_island_shape.gd

var _failures := 0
var _ran := false


func _process(_delta: float) -> bool:
	if _ran:
		quit(1)
		return true
	_ran = true
	var lo := Vector2(-24.0, -24.0)
	var hi := Vector2(40.0, 24.0)
	_check(WorldBounds.clamp_rect(Vector2(35.0, 0.0), lo, hi, 6.0) == Vector2(35.0, 0.0), "point on the eastern strip stays")
	_check(WorldBounds.clamp_rect(Vector2(50.0, 0.0), lo, hi, 6.0) == Vector2(40.0, 0.0), "east edge clamps at 40")
	_check(WorldBounds.clamp_rect(Vector2(-30.0, 0.0), lo, hi, 6.0) == Vector2(-24.0, 0.0), "west edge still at -24")
	_check(WorldBounds.clamp_point(Vector2(30.0, 0.0), 23.2, 6.2) == Vector2(23.2, 0.0), "square clamp unchanged")

	var island: Node = (load("res://scenes/island/Island.tscn") as PackedScene).instantiate()
	root.add_child(island)
	var shape: IslandShape = island.get_node("IslandShape")
	var bounds: WorldBounds = island.get_node("WorldBounds")
	var nav: NavGrid = island.get_node("NavGrid")
	var placer: Placer = island.get_node("Placer")
	var collision: CollisionShape3D = island.get_node("Ground/Collision")
	_check(not nav.is_free(Vector2(30.0, 0.0)), "x = 30 is sea before any expansion")
	shape.apply_expand({"n": 0, "e": 2, "s": 1, "w": 0})
	_check(shape.box_max == Vector2(40.0, 32.0) and shape.box_min == Vector2(-24.0, -24.0), "two strips east, one south: %s %s" % [shape.box_min, shape.box_max])
	_check(bounds.box_max == Vector2(40.0, 32.0), "WorldBounds follows")
	_check(placer.box_max == Vector2(40.0, 32.0), "Placer follows")
	_check(shape.water_material != null and shape.water_material.get_shader_parameter("island_centre") == Vector2(8.0, 4.0) and shape.water_material.get_shader_parameter("half_extent") == Vector2(32.0, 28.0), "water shore follows the island centre and extents")
	_check(nav.is_free(Vector2(30.0, 0.0)) and not nav.is_free(Vector2(45.0, 0.0)), "NavGrid walks the new strip and stops at its edge")
	var box := collision.shape as BoxShape3D
	_check(is_equal_approx(box.size.x, 64.0) and is_equal_approx(box.size.z, 56.0) and is_equal_approx(collision.position.x, 8.0), "collision box grew and moved: %s at %s" % [box.size, collision.position])
	shape.apply_expand({"n": "lots", "e": 1, "s": 0, "w": 0})
	_check(shape.box_max == Vector2(40.0, 32.0), "malformed expand is ignored")
	shape.apply_expand({"e": 0})
	_check(shape.box_max == Vector2(40.0, 32.0), "partial expand is ignored, island does not shrink")
	shape.apply_expand({"n": 9, "e": 0, "s": 0, "w": 0})
	_check(shape.box_min == Vector2(-24.0, -56.0), "counts clamp to four strips")
	print("RESULT: %s (%d failures)" % ["PASS" if _failures == 0 else "FAIL", _failures])
	quit(1 if _failures > 0 else 0)
	return true


func _check(ok: bool, what: String) -> void:
	if not ok:
		_failures += 1
	print("%s %s" % ["ok  " if ok else "FAIL", what])
