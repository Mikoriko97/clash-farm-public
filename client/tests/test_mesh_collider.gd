extends SceneTree
## Headless test: prop wrappers get exact trimesh colliders from their piece
## meshes (no more hand-sized boxes), the trimesh footprint matches the mesh,
## and the NavGrid still reads it as an obstacle.
## Run: godot --headless --path client --script res://tests/test_mesh_collider.gd

var _failures := 0
var _done := false


func _check(ok: bool, what: String) -> void:
	if not ok:
		_failures += 1
		push_error("FAIL: " + what)


func _process(_delta: float) -> bool:
	if _done:
		return true
	_done = true
	_run()
	print("RESULT: %s (%d failures)" % ["PASS" if _failures == 0 else "FAIL", _failures])
	quit(1 if _failures > 0 else 0)
	return true


func _run() -> void:
	var world := Node3D.new()
	root.add_child(world)
	var bench: Node3D = (load("res://scenes/props/bench.tscn") as PackedScene).instantiate()
	bench.position = Vector3(3.0, 0.0, -2.0)
	world.add_child(bench)
	var body := bench.get_node("Body") as StaticBody3D
	_check(body is MeshCollider, "bench body is a MeshCollider")
	var shapes: Array[Node] = body.find_children("*", "CollisionShape3D", false, false)
	var trimesh := 0
	var simple := 0
	for s: Node in shapes:
		var cs := s as CollisionShape3D
		if cs.is_queued_for_deletion():
			continue
		if cs.shape is ConcavePolygonShape3D:
			trimesh += 1
		else:
			simple += 1
	_check(trimesh >= 1, "at least one trimesh shape: %d" % trimesh)
	_check(simple == 0, "hand-made box dropped: %d left" % simple)
	var mesh: MeshInstance3D = bench.find_children("*", "MeshInstance3D", true, false)[0] as MeshInstance3D
	var mesh_box := mesh.global_transform * mesh.get_aabb()
	var shape_node: CollisionShape3D = null
	for s: Node in shapes:
		if (s as CollisionShape3D).shape is ConcavePolygonShape3D:
			shape_node = s as CollisionShape3D
			break
	var shape_box := NavGrid.world_aabb(shape_node)
	_check(shape_box.position.distance_to(mesh_box.position) < 0.05 and shape_box.size.distance_to(mesh_box.size) < 0.05,
		"trimesh footprint equals the mesh: %s vs %s" % [shape_box, mesh_box])
	var nav := NavGrid.new()
	nav.name = "NavGrid"
	world.add_child(nav)
	var rects := nav.collect_obstacles(world)
	_check(rects.size() == 1 and rects[0].has_point(Vector2(3.0, -2.0)), "NavGrid sees the bench footprint: %s" % str(rects))
	# Trees keep their trunk cylinder: no trimesh crown.
	var tree: Node3D = (load("res://scenes/props/tree_a.tscn") as PackedScene).instantiate()
	world.add_child(tree)
	var tree_shapes: Array[Node] = tree.find_children("*", "CollisionShape3D", true, false)
	_check(tree_shapes.size() == 1 and (tree_shapes[0] as CollisionShape3D).shape is CylinderShape3D, "tree keeps its trunk cylinder")
