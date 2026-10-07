extends SceneTree
## Headless test: baked mainland flora is solid where it should be and the
## island's wild growth keeps out of the house, plots, decor and resources.
## Run: godot --headless --path client --script res://tests/test_flora.gd

var _failures := 0
var _frames := 0
var _mainland: Node3D
var _terrain: MainlandTerrain
var _island: Node3D


func _check(ok: bool, what: String) -> void:
	if not ok:
		_failures += 1
		push_error("FAIL: " + what)


func _init() -> void:
	_mainland = (load("res://scenes/mainland/Mainland.tscn") as PackedScene).instantiate()
	root.add_child(_mainland)
	_terrain = _mainland.get_node("Terrain")
	# North-west forest: chunk (3, 2) is LOD 0 around the character.
	(_mainland.get_node("Player") as Node3D).position = Vector3(-300.0, 8.0, -380.0)


func _process(_delta: float) -> bool:
	_frames += 1
	if _frames == 240:
		_check_mainland()
		_mainland.queue_free()
		_island = (load("res://scenes/island/Island.tscn") as PackedScene).instantiate()
		root.add_child(_island)
	elif _frames == 300:
		_check_island()
		print("RESULT: %s (%d failures)" % ["PASS" if _failures == 0 else "FAIL", _failures])
		quit(1 if _failures > 0 else 0)
		return true
	return false


func _check_mainland() -> void:
	_check(_terrain.flora_pending() == 0, "flora queue drained: %d left" % _terrain.flora_pending())
	var chunk := _terrain.chunk(Vector2i(3, 2))
	_check(chunk != null and chunk.lod == 0, "forest chunk (3, 2) is loaded at LOD 0")
	if chunk == null:
		return
	var meshes := chunk.find_children("Flora_*", "MultiMeshInstance3D", false, false)
	var bodies := chunk.find_children("FloraBody_*", "StaticBody3D", false, false)
	_check(meshes.size() >= 5, "forest chunk carries several flora kinds: %d" % meshes.size())
	_check(bodies.size() >= 3, "trees, bushes and stones got collider bodies: %d" % bodies.size())
	var shapes := 0
	for body: Node in bodies:
		shapes += body.get_child_count()
	_check(shapes >= 20, "forest chunk holds many trunk colliders: %d" % shapes)
	var crowns := chunk.find_children("FloraShadow_*", "MultiMeshInstance3D", false, false)
	_check(not crowns.is_empty() and (crowns[0] as MultiMeshInstance3D).cast_shadow == GeometryInstance3D.SHADOW_CASTING_SETTING_SHADOWS_ONLY, "trees cast shadows through shadow-only crowns: %d" % crowns.size())
	var tinted := 0
	for node: Node in meshes:
		if (node as MultiMeshInstance3D).multimesh.use_colors:
			tinted += 1
	_check(tinted >= 2, "tree kinds carry per-instance leaf tints: %d" % tinted)
	# A baked trunk is solid: a small sphere at its base overlaps a StaticBody3D.
	var space := chunk.get_world_3d().direct_space_state
	var hit := false
	for body: Node in bodies:
		if body.get_child_count() == 0:
			continue
		var col := body.get_child(0) as CollisionShape3D
		var query := PhysicsShapeQueryParameters3D.new()
		var probe := SphereShape3D.new()
		probe.radius = 0.2
		query.shape = probe
		query.transform = Transform3D(Basis.IDENTITY, col.global_position)
		for r: Dictionary in space.intersect_shape(query, 8):
			if r.get("collider") == body:
				hit = true
		break
	_check(hit, "physics sees the trunk collider")
	_check_groups(chunk)
	var far := _terrain.chunk(Vector2i(6, 2))
	_check(far != null and far.lod == 1 and far.find_children("FloraBody_*", "StaticBody3D", false, false).is_empty(), "far chunks carry no colliders")
	_check(far != null and far.find_children("FloraShadow_*", "MultiMeshInstance3D", false, false).is_empty(), "far chunks carry no shadow crowns")


## Hero groups stand on the ground and the bake keeps generated flora out of their circles.
func _check_groups(chunk: Node3D) -> void:
	var holder: FloraGroups = _mainland.get_node("Groups")
	var groups := holder.groups()
	_check(groups.size() >= 6, "mainland carries hero groups: %d" % groups.size())
	var log_fern: FloraGroup = holder.get_node("LogFern")
	_check(absf(log_fern.position.y - _terrain.ground_min(log_fern.position.x, log_fern.position.z, log_fern.radius_m * 0.5)) < 0.05, "hero group snapped to the ground")
	var inside := 0
	for node: Node in chunk.find_children("Flora_*", "MultiMeshInstance3D", false, false):
		var mm := (node as MultiMeshInstance3D).multimesh
		for i: int in mm.instance_count:
			var p := chunk.global_transform * mm.get_instance_transform(i).origin
			if Vector2(p.x, p.z).distance_to(Vector2(log_fern.position.x, log_fern.position.z)) < log_fern.radius_m - 0.05:
				inside += 1
	_check(inside == 0, "no generated flora inside the log_fern group circle: %d" % inside)


func _check_island() -> void:
	var wild: IslandWild = _island.get_node("Wild")
	_check(wild.instance_count() > 100, "island grew wild plants: %d" % wild.instance_count())
	var rects := wild.obstacle_rects(Scatter.new())
	_check(rects.size() >= 8, "house, resources and bushes count as obstacles: %d" % rects.size())
	var inside := 0
	var total := 0
	var placed: Dictionary = wild.placed_all()
	for name: String in placed:
		var pts: PackedFloat32Array = placed[name]
		for i: int in pts.size() / 4:
			var p := Vector2(pts[i * 4], pts[i * 4 + 1])
			total += 1
			for r: Rect2 in rects:
				if r.grow(-wild.clear_margin_m).has_point(p):
					inside += 1
					break
	_check(inside == 0, "no wild plant inside an obstacle footprint: %d of %d" % [inside, total])
	_check(not wild.find_children("WildBody_*", "StaticBody3D", false, false).is_empty(), "island bushes are solid")
