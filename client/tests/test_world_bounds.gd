extends SceneTree
## Headless test: WorldBounds.clamp_point keeps points inside the rounded square
## that the ground shader draws (edges, corners, interior untouched).
## Run: godot --headless --path client --script res://tests/test_world_bounds.gd

var _failures := 0


func _init() -> void:
	var h := 23.2
	var r := 6.2
	_check(WorldBounds.clamp_point(Vector2(3.0, -5.0), h, r) == Vector2(3.0, -5.0), "interior point untouched")
	_check(WorldBounds.clamp_point(Vector2(30.0, 0.0), h, r) == Vector2(h, 0.0), "east edge clamps to the half size")
	_check(WorldBounds.clamp_point(Vector2(0.0, -40.0), h, r) == Vector2(0.0, -h), "north edge clamps")
	var corner := WorldBounds.clamp_point(Vector2(30.0, 30.0), h, r)
	var centre := Vector2(h - r, h - r)
	_check(absf((corner - centre).length() - r) < 0.001 and corner.x < h, "corner point lands on the rounded arc: %s" % corner)
	var on_arc := WorldBounds.clamp_point(Vector2(h, h), h, r)
	_check((on_arc - centre).length() <= r + 0.001, "square corner itself is pulled onto the arc")
	print("RESULT: %s (%d failures)" % ["PASS" if _failures == 0 else "FAIL", _failures])
	quit(1 if _failures > 0 else 0)


func _check(ok: bool, label: String) -> void:
	if not ok:
		_failures += 1
	print("%s %s" % ["ok  " if ok else "FAIL", label])
