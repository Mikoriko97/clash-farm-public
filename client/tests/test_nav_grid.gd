extends SceneTree
## Headless test: NavGrid routes around a plot-sized obstacle, keeps straight
## lines straight, snaps blocked goals to the nearest free cell and respects the
## island edge. Player follows the waypoints in order.
## Run: godot --headless --path client --script res://tests/test_nav_grid.gd

var _failures := 0
var _root: Node3D
var _ran := false


func _init() -> void:
	_root = Node3D.new()
	root.add_child(_root)
	_add_box(Vector3(0.0, 0.15, 0.0), Vector3(2.4, 0.3, 2.4))
	_add_box(Vector3(0.0, -1.0, 0.0), Vector3(48.0, 2.0, 48.0))  # ground: ignored
	var nav := NavGrid.new()
	nav.name = "NavGrid"
	nav.obstacle_root = _root
	_root.add_child(nav)


## Nodes only resolve their global transforms once the loop runs: test on the first frame.
func _process(_delta: float) -> bool:
	if _ran:
		quit(1)  # a script error above skipped quit(): never loop forever
		return true
	_ran = true
	var nav: NavGrid = _root.get_node("NavGrid") as NavGrid
	var rects := nav.collect_obstacles(_root)
	_check(rects.size() == 1, "ground box is ignored, plot box kept: %d obstacles" % rects.size())
	_check(rects.size() == 1 and rects[0].has_point(Vector2(1.1, -1.1)), "plot footprint covers its corners")

	var straight := nav.find_path(Vector3(-6.0, 0.0, 6.0), Vector3(6.0, 0.0, 6.0))
	_check(straight.size() == 2 and straight[0].is_equal_approx(Vector3(-6.0, 0.0, 6.0)) and straight[1].is_equal_approx(Vector3(6.0, 0.0, 6.0)), "clear line is one straight leg from the character to the click: %s" % straight)

	var around := nav.find_path(Vector3(-4.0, 0.0, 0.0), Vector3(4.0, 0.0, 0.0))
	_check(around.size() >= 3, "path through the plot bends around it: %d points" % around.size())
	_check(_avoids(nav, around), "no leg crosses the inflated plot")
	_check(around.size() > 0 and around[around.size() - 1].is_equal_approx(Vector3(4.0, 0.0, 0.0)), "path ends at the goal")

	var blocked := nav.find_path(Vector3(-4.0, 0.0, 0.0), Vector3(0.0, 0.0, 0.0))
	_check(blocked.size() >= 2 and nav.is_free(Vector2(blocked[blocked.size() - 1].x, blocked[blocked.size() - 1].z)), "goal inside the plot snaps to a free cell next to it")

	var edge := nav.find_path(Vector3(0.0, 0.0, 10.0), Vector3(40.0, 0.0, 40.0))
	var last := edge[edge.size() - 1] if edge.size() > 0 else Vector3.ZERO
	_check(edge.size() >= 2 and WorldBounds.clamp_point(Vector2(last.x, last.z), 23.2, 6.2) == Vector2(last.x, last.z), "goal off the island stops inside the rounded edge: %s" % last)

	_check_player_follow()
	print("RESULT: %s (%d failures)" % ["PASS" if _failures == 0 else "FAIL", _failures])
	quit(1 if _failures > 0 else 0)
	return true


func _check_player_follow() -> void:
	var player: Player = (load("res://scenes/player/Player.tscn") as PackedScene).instantiate() as Player
	root.add_child(player)
	player.walk_path(PackedVector3Array([Vector3(1.0, 0.0, 0.0), Vector3(1.0, 0.0, 3.0)]), 0.5)
	var d1: Vector3 = player._target_direction()
	_check(d1.is_equal_approx(Vector3(1.0, 0.0, 0.0)), "heads for the first waypoint")
	player.global_position = Vector3(1.0, 0.0, 0.1)
	var d2: Vector3 = player._target_direction()
	_check(d2.is_equal_approx(Vector3(0.0, 0.0, 1.0)), "switches to the next waypoint on arrival")
	player.global_position = Vector3(1.0, 0.0, 2.6)
	_check(player._target_direction() == Vector3.ZERO and not player.is_walking_to_target(), "stops stop_distance short of the last waypoint")
	player.queue_free()


func _avoids(nav: NavGrid, path: PackedVector3Array) -> bool:
	for i in range(path.size() - 1):
		var a := Vector2(path[i].x, path[i].z)
		var b := Vector2(path[i + 1].x, path[i + 1].z)
		for s in range(21):
			if not nav.is_free(a.lerp(b, s / 20.0)):
				return false
	return true


func _add_box(centre: Vector3, size: Vector3) -> void:
	var body := StaticBody3D.new()
	body.position = centre
	var shape := CollisionShape3D.new()
	var box := BoxShape3D.new()
	box.size = size
	shape.shape = box
	body.add_child(shape)
	_root.add_child(body)


func _check(ok: bool, label: String) -> void:
	if not ok:
		_failures += 1
	print("%s %s" % ["ok  " if ok else "FAIL", label])
