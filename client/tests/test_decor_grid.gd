extends SceneTree
## Headless test: DecorGrid instances DecorItems from world_state.decor at the
## server's point and yaw, drops unlisted ones on a full list, survives garbage,
## and exposes pieces for the Placer ghost. Placer snaps decor to 0.5 m and turns in 15° steps.
## Run: godot --headless --path client --script res://tests/test_decor_grid.gd

var _failures := 0
var _ran := false


func _process(_delta: float) -> bool:
	if _ran:
		quit(1)
		return true
	_ran = true
	var grid: DecorGrid = (load("res://scenes/island/DecorSlots.tscn") as PackedScene).instantiate() as DecorGrid
	root.add_child(grid)
	_check(grid.item_count() == 0, "no decor before world_state")
	grid.apply_world_state({"full": true, "decor": [
		{"id": "a", "item_id": "decor_fence", "x_dm": 80, "z_dm": -40, "yaw_deg": 90},
		{"id": "b", "item_id": "decor_unknown", "x_dm": 0, "z_dm": 120},
		{"id": "", "item_id": "decor_bench", "x_dm": 1, "z_dm": 1},
		{"id": "c", "item_id": "decor_bench", "x_dm": "far", "z_dm": 1},
		"junk",
	]})
	_check(grid.item_count() == 2, "two valid placements created (%d)" % grid.item_count())
	var fence := grid.item("a")
	_check(fence != null and fence.position == Vector3(8.0, 0.0, -4.0), "fence at (80, -40) dm -> (8, -4) m: %s" % (fence.position if fence else Vector3.ZERO))
	_check(fence != null and is_equal_approx(fence.rotation.y, deg_to_rad(90.0)), "yaw applied")
	_check(fence != null and fence.get_node("%Holder").get_child_count() == 1 and not fence.get_node("%Marker").visible, "piece instantiated, marker hidden")
	_check(grid.item("b").get_node("%Marker").visible, "unknown item shows the marker instead of crashing")
	_check(fence.interaction_kind() == "decor" and fence.interaction_id() == "a" and fence.idx == 0, "interactable of kind decor with its placement id")
	grid.apply_world_state({"decor_full": true, "decor": [{"id": "b", "item_id": "decor_barrel", "x_dm": 0, "z_dm": 120, "yaw_deg": 0}]})
	_check(grid.item("a") == null and grid.item_count() == 1, "full decor list drops the fence")
	_check(grid.item("b").item_id == "decor_barrel", "existing placement swaps its piece")
	_check(grid.piece_for("decor_bench") != null and grid.piece_for("nope") == null, "piece lookup for the ghost")
	_check_placer(grid)
	print("RESULT: %s (%d failures)" % ["PASS" if _failures == 0 else "FAIL", _failures])
	quit(1 if _failures > 0 else 0)
	return true


func _check_placer(grid: DecorGrid) -> void:
	var placer: Placer = (load("res://scenes/island/Placer.tscn") as PackedScene).instantiate() as Placer
	placer.decor_grid = grid
	root.add_child(placer)
	_check(Placer.snap(Vector3(3.26, 0.0, -1.74)) == Vector3(3.5, 0.0, -1.5), "decor snaps to half metres")
	placer.configure("decor_bench", 1.2)
	_check(placer.kind == "decor" and placer.get_node("%DecorGhost").get_child_count() == 1, "decor ghost carries the item piece")
	_check(placer.get_node("%DecorGhost").find_children("*", "CollisionObject3D", true, false).is_empty(), "ghost has no colliders of its own")
	placer.rotate_ghost(15)
	placer.rotate_ghost(-45)
	_check(placer.yaw_deg == 330, "yaw wraps in 15° steps: %d" % placer.yaw_deg)
	_check(not placer.is_spot_free(Vector3(23.5, 0.0, 0.0), 1.2), "spot over the edge is not free")
	placer.configure("", 0.0)
	_check(placer.kind == "plot" and placer.yaw_deg == 0, "empty item means a plot ghost")


func _check(ok: bool, what: String) -> void:
	if not ok:
		_failures += 1
	print("%s %s" % ["ok  " if ok else "FAIL", what])
