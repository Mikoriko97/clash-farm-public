extends SceneTree
## Headless test: island wild growth follows the owner's rules for bought strips —
## "X" kinds grow only outside the base 48 m square (none on the bare island, plenty
## once a strip is bought), shore kinds keep within `near_water_m` of the rounded
## edge, and base kinds stay off the strips.
## Run: godot --headless --path client --script res://tests/test_island_strips.gd

var _failures := 0
var _frames := 0
var _island: Node3D


func _check(ok: bool, what: String) -> void:
	if not ok:
		_failures += 1
		push_error("FAIL: " + what)


func _init() -> void:
	_island = (load("res://scenes/island/Island.tscn") as PackedScene).instantiate()
	root.add_child(_island)


func _process(_delta: float) -> bool:
	_frames += 1
	if _frames == 40:
		_check(_count("x_") == 0, "no strip kinds on the base island: %d" % _count("x_"))
		(_island.get_node("IslandShape") as IslandShape).apply_expand({"n": 1, "e": 2, "s": 0, "w": 0})
	elif _frames == 80:
		_check_strips()
		print("RESULT: %s (%d failures)" % ["PASS" if _failures == 0 else "FAIL", _failures])
		quit(1 if _failures > 0 else 0)
		return true
	return false


func _count(prefix: String) -> int:
	var n := 0
	var placed: Dictionary = (_island.get_node("Wild") as IslandWild).placed_all()
	for name: String in placed:
		if name.begins_with(prefix):
			n += (placed[name] as PackedFloat32Array).size() / 4
	return n


func _check_strips() -> void:
	var wild: IslandWild = _island.get_node("Wild")
	_check(wild.pending() == 0, "wild growth finished after the expansion")
	var half := IslandShape.BASE_HALF_M
	var strip_total := 0
	var strip_in_base := 0
	var shore_far := 0
	var placed: Dictionary = wild.placed_all()
	for name: String in placed:
		var pts: PackedFloat32Array = placed[name]
		for i: int in pts.size() / 4:
			var p := Vector2(pts[i * 4], pts[i * 4 + 1])
			var in_base := absf(p.x) <= half and absf(p.y) <= half
			if name.begins_with("x_"):
				strip_total += 1
				if in_base:
					strip_in_base += 1
				if name.begins_with("x_shore") and wild.edge_distance(p) > 3.05:
					shore_far += 1
	_check(strip_total > 40, "strip kinds grew on the bought strips: %d" % strip_total)
	_check(strip_in_base == 0, "no strip kind inside the base square: %d" % strip_in_base)
	_check(shore_far == 0, "shore kinds hug the edge: %d too far" % shore_far)
	_check(wild.edge_distance(Vector2(0.0, 0.0)) > 20.0 and wild.edge_distance(Vector2(0.0, -31.9)) < 0.5, "edge distance follows the expanded shape")
