extends SceneTree
## Smoke test of the EXPORTED build, run against the pck the browser will load:
##   godot --headless --main-pack web/public/godot/index.pck --script /abs/path/client/tools/check_export.gd
## The editor and the export are not the same scene data (2026-09-09: overrides on
## instanced children vanished in the export, doors and signs ended up in a field),
## so after every export this checks what the player will actually get: the city
## doors sit outside their buildings and face the square, the terrain under the
## square is flat at 4 m, the character rests on the terrain collider, the ground
## collider matches the height map. Prints RESULT: PASS|FAIL and exits 1 on FAIL.

var _frames := 0
var _failures := 0
var _scene: Node
var _player: CharacterBody3D
var _terrain: Node


func _check(ok: bool, what: String) -> void:
	if not ok:
		_failures += 1
		push_error("FAIL: " + what)


func _init() -> void:
	# Run from the project folder, Godot mounts client/ under the pack and every check
	# passes on project files instead of exported ones: refuse.
	if FileAccess.file_exists("res://project.godot") or FileAccess.file_exists("res://tests/test_flora.gd"):
		push_error("FAIL: check_export sees project files next to the pack; run it from outside client/")
		print("RESULT: FAIL (1 failures)")
		quit(1)
		return
	_check(FileAccess.file_exists("res://build.txt"), "build stamp res://build.txt in the pack (addons/build_stamp)")
	print("build: " + FileAccess.get_file_as_string("res://build.txt").strip_edges())
	var bake: FloraBake = load("res://assets/mainland/flora.res") as FloraBake if ResourceLoader.exists("res://assets/mainland/flora.res") else null
	_check(bake != null and bake.chunks.size() > 100, "flora bake res://assets/mainland/flora.res in the pack with chunks")
	var oak: PackedScene = load("res://assets/simple_trees/pieces/tree_simple_03_b.glb") if ResourceLoader.exists("res://assets/simple_trees/pieces/tree_simple_03_b.glb") else null
	var oak_meshes: Array[Node] = oak.instantiate().find_children("lod*", "MeshInstance3D", true, false) if oak != null else []
	_check(oak_meshes.size() == 3, "Simple Trees oak with three levels of detail in the pack: %d" % oak_meshes.size())
	_check(ResourceLoader.exists("res://assets/simple_trees/textures/leaves_albedo.png") and ResourceLoader.exists("res://shaders/foliage_cards.gdshader"), "leaf atlas and foliage shader in the pack")
	# Character parts (ADR-025; 4.5-head base ADR-032): the GLBs are only preloaded or loaded by scripts, so the export filter must list them.
	var parts: PackedScene = load("res://assets/character/base45/head_parts.glb") if ResourceLoader.exists("res://assets/character/base45/head_parts.glb") else null
	var part_nodes: Array[Node] = parts.instantiate().get_children() if parts != null else []
	_check(part_nodes.size() >= 23, "character parts library head_parts.glb in the pack with all parts: %d" % part_nodes.size())
	_check(CharacterBody.template("body_f_sturdy") != null and CharacterFace.ATLAS != null, "every body GLB and the face atlas are in the pack")
	# Tools in the hand (ADR-034): meshes skinned to the hand bone and their clips.
	var lib := load("res://assets/character/character_anims.tres") as AnimationLibrary
	var axe := HandTools.template("axe")
	var axe_tex: Texture2D = (axe.mesh.surface_get_material(0) as BaseMaterial3D).albedo_texture if axe != null and axe.mesh.surface_get_material(0) is BaseMaterial3D else null
	_check(axe != null and HandTools.template("can") != null and axe_tex != null, "tool meshes tools45.glb (with their texture) are in the pack")
	_check(lib != null and lib.has_animation("chop") and lib.has_animation("reap") and lib.has_animation("water"), "tool clips are in the animation library")
	# Life (ADR-035): every action, reaction and emote of `play_fx` has its clip in the pack.
	var missing: Array[String] = []
	for fx: String in FxClips.TABLE:
		if lib == null or not lib.has_animation((FxClips.TABLE[fx] as Dictionary)["clip"]):
			missing.append(fx)
	_check(missing.is_empty(), "every play_fx clip (life emotes too) is in the library: missing %s" % [missing])
	_check(CharacterOutfit.template("outfit_jacket") != null and CharacterOutfit.template("outfit_dress") != null, "outfit library outfits.glb in the pack with its shells")
	_scene = (load("res://scenes/mainland/Mainland.tscn") as PackedScene).instantiate()
	root.add_child(_scene)
	_player = _scene.get_node("Player")
	_terrain = _scene.find_child("Terrain", true, false)
	_player.global_position = Vector3(-14.0, 6.0, 42.0)


func _process(_delta: float) -> bool:
	_frames += 1
	if _frames < 150:
		return false
	_check_city()
	_check_ground()
	print("RESULT: %s (%d failures)" % ["PASS" if _failures == 0 else "FAIL", _failures])
	quit(1 if _failures > 0 else 0)
	return true


func _check_city() -> void:
	var city: Node3D = _scene.get_node("City")
	var buildings := city.find_children("*", "CityBuilding", false, false)
	_check(buildings.size() == 8, "eight buildings exported: %d" % buildings.size())
	for b: Node in buildings:
		var node := b as Node3D
		var door: Vector3 = b.call("approach_target")
		# Walls are axis-aligned in the building's own frame; the world AABB of a turned piece is fat.
		var box := _bounds(node.get_node("Piece"), node.global_transform.affine_inverse())
		var local_door := node.to_local(door)
		var flat_door := Vector3(local_door.x, box.position.y + 0.5, local_door.z)
		_check(not box.has_point(flat_door) or b.get("kind") == "market" or b.get("kind") == "hall", "%s: door outside the walls (%s vs %s..%s)" % [b.name, local_door, box.position, box.end])
		var to_square := (city.global_position - node.global_position)
		var to_door := door - node.global_position
		to_square.y = 0.0
		to_door.y = 0.0
		_check(to_door.length() > 2.0 and to_door.normalized().dot(to_square.normalized()) > 0.5, "%s: door on the square side" % b.name)
		var sign: Node3D = node.get_node_or_null("%Sign")
		_check(sign != null and (not b.get("sign_auto") or sign.global_position.distance_to(door) < 8.0 and sign.global_position.distance_to(node.global_position) < door.distance_to(node.global_position) + 1.0), "%s: sign hangs on the facade behind the door" % b.name)
		_check(box.position.y > -0.6 and box.position.y < 0.3, "%s: piece stands on the square level, sunk or lifted a little (%.2f)" % [b.name, box.position.y])


func _check_ground() -> void:
	var p := _player.global_position
	var ground: float = _terrain.call("height_at", p.x, p.z)
	_check(absf(ground - 4.0) < 0.01, "height map flat under the square: %.3f" % ground)
	_check(_player.is_on_floor() and absf(p.y - ground) < 0.05, "character rests on the terrain: y=%.3f ground=%.3f on_floor=%s" % [p.y, ground, _player.is_on_floor()])
	var query := PhysicsRayQueryParameters3D.create(Vector3(-10.0, 50.0, 44.0), Vector3(-10.0, -10.0, 44.0), 1)
	query.exclude = [_player.get_rid()]
	var hit := root.get_world_3d().direct_space_state.intersect_ray(query)
	var hit_y: float = (hit["position"] as Vector3).y if hit.has("position") else NAN
	_check(hit.has("position") and absf(hit_y - float(_terrain.call("height_at", -10.0, 44.0))) < 0.01, "terrain collider matches the height map: %.3f" % hit_y)


## Merged mesh bounds of `piece`, expressed in the frame given by `into` (world → frame).
static func _bounds(piece: Node3D, into: Transform3D = Transform3D.IDENTITY) -> AABB:
	var box := AABB()
	var first := true
	for mi: Node in piece.find_children("*", "MeshInstance3D", true, false):
		var m := mi as MeshInstance3D
		# Invisible collision helpers (buried ramps) are not part of the visible footprint.
		if m.name.begins_with("collision_"):
			continue
		var gb := (into * m.global_transform) * m.get_aabb()
		box = gb if first else box.merge(gb)
		first = false
	return box
