extends SceneTree
## Headless test: the mainland streams terrain chunks around the character with
## the right LODs and colliders, heights follow the baked map, resource nodes
## from `mainland_nodes` land on the ground and leave with their chunk.
## Run: godot --headless --path client --script res://tests/test_mainland.gd

var _failures := 0
var _frames := 0
var _stage := 0
var _scene: Node3D
var _terrain: MainlandTerrain
var _nodes: MainlandNodes
var _landmarks: MainlandLandmarks
var _player: Node3D


func _check(ok: bool, what: String) -> void:
	if not ok:
		_failures += 1
		push_error("FAIL: " + what)


func _init() -> void:
	_scene = (load("res://scenes/mainland/Mainland.tscn") as PackedScene).instantiate()
	root.add_child(_scene)
	_terrain = _scene.get_node("Terrain")
	_nodes = _scene.get_node("Nodes")
	_nodes.spawn_budget_ms = 0.0
	_landmarks = _scene.get_node("Landmarks")
	_player = _scene.get_node("Player")


func _process(_delta: float) -> bool:
	_frames += 1
	if _stage == 0 and _frames == 60:
		_check_terrain()
		_check_nodes()
		_check_landmarks()
		_check_bridge()
		_check_city()
		_player.global_position = Vector3(0.0, 8.0, 0.0)
		_stage = 1
	elif _stage == 1 and _frames == 140:
		_check(_terrain.chunk(Vector2i(8, 14)) == null, "chunk under the old spawn unloaded after the move")
		_check(_terrain.chunk(Vector2i(8, 8)) != null and _terrain.chunk(Vector2i(8, 8)).lod == 0, "chunk under the new position is LOD 0")
		_check(_nodes.count() == 0, "nodes left with their chunk: %d" % _nodes.count())
		print("RESULT: %s (%d failures)" % ["PASS" if _failures == 0 else "FAIL", _failures])
		quit(1 if _failures > 0 else 0)
		return true
	return false


func _check_terrain() -> void:
	# Spawn is at z = 400 → chunk (8, 14): 7 columns × 5 rows fit inside the 16 × 16 map.
	_check(_terrain.pending_count() == 0, "queue drained in 60 frames (%d left)" % _terrain.pending_count())
	_check(_terrain.loaded_count() == 35, "7 × 5 chunks loaded: %d" % _terrain.loaded_count())
	var centre := _terrain.chunk(Vector2i(8, 14))
	_check(centre != null and centre.lod == 0 and centre.get_node_or_null("Collision") != null, "centre chunk is LOD 0 with a collider")
	var far := _terrain.chunk(Vector2i(11, 11))
	_check(far != null and far.lod == 1 and far.get_node_or_null("Collision") == null, "far chunk is LOD 1 without a collider")
	var beach := _terrain.height_at(0.0, 400.0)
	_check(beach > 0.3 and beach < 1.6, "beach height near the dock: %.2f" % beach)
	_check(_terrain.height_at(400.0, -160.0) > 12.0, "mountains rise in the east: %.1f" % _terrain.height_at(400.0, -160.0))
	_check(_terrain.height_at(305.0, -65.0) < -1.0, "lake below the pier holds water: %.1f" % _terrain.height_at(305.0, -65.0))
	var deepest := INF
	for bx: int in range(130, 195, 5):
		deepest = minf(deepest, _terrain.height_at(float(bx), 150.0))
	_check(deepest < -1.0 and _terrain.height_at(120.0, 150.0) > 0.8, "river runs east of the old bridge (%.1f), the bridge stands on the bank" % deepest)
	_check(_terrain.height_at(0.0, 500.0) < -2.0, "open sea south of the beach: %.1f" % _terrain.height_at(0.0, 500.0))
	_check(absf(_terrain.height_at(0.0, 60.0) - 4.0) < 0.3, "city is flat at 4 m: %.2f" % _terrain.height_at(0.0, 60.0))
	var weights := _terrain.biome_at(-300.0, -380.0)
	_check(weights.g > 0.45, "north-west is forest: %s" % str(weights))
	_check(_terrain.biome_at(0.0, 400.0).a > 0.6, "beach is sand")
	# The character stands on the collider instead of falling through it.
	var ground := _terrain.height_at(_player.global_position.x, _player.global_position.z)
	_check(absf(_player.global_position.y - ground) < 0.6, "player rests on the terrain: y=%.2f ground=%.2f" % [_player.global_position.y, ground])
	_check(_terrain.flora_bake != null and _terrain.flora_bake.chunks.size() > 100, "flora bake loaded with chunks")
	_check(_terrain.flora_bake != null and _terrain.flora_bake.kinds.size() == _terrain.flora.size(), "flora bake matches the scene's kind list")


func _check_city() -> void:
	var city: Node3D = _scene.get_node("City")
	var buildings := city.find_children("*", "CityBuilding", false, false)
	_check(buildings.size() == 8, "eight buildings on the square: %d" % buildings.size())
	var market: CityBuilding = city.get_node("Market")
	_check(market.interaction_kind() == "building" and market.interaction_id() == "market", "market is an interactable building")
	var door := market.approach_target()
	_check(door.distance_to(market.global_position) > 3.0 and absf(door.y - 4.0) < 0.2, "door marker sits outside the walls on the square: %s" % door)
	_check(market.get_node("Body").get_child_count() > 0, "building has a trimesh collider")
	_check((market.get_node("%Name") as Label3D).text == "Market", "name plate shows the label")


func _check_landmarks() -> void:
	_landmarks.apply([
		{"id": "dock", "name": "Причал", "x_dm": 0, "z_dm": 4000, "discovered": false},
		{"id": "shipwreck", "name": "Кістяк корабля", "x_dm": -2200, "z_dm": 3900, "discovered": true},
		{"id": "", "name": "?", "x_dm": 0, "z_dm": 0},
		{"id": "bad", "name": 3, "x_dm": 0, "z_dm": 0},
	])
	_check(_landmarks.count() == 2, "two valid landmarks, malformed ignored: %d" % _landmarks.count())
	var dock := _landmarks.marker("dock")
	_check(dock != null and absf(dock.position.y - _terrain.ground_min(0.0, 400.0, 0.4)) < 0.01, "marker sits on the terrain")
	_check(dock != null and dock.interaction_kind() == "landmark" and dock.interaction_id() == "dock", "marker is an interactable landmark")
	_check(dock != null and (dock.get_node("%Name") as Label3D).text == "Причал", "marker shows the name")
	var wreck := _landmarks.marker("shipwreck")
	_check(wreck != null and (wreck.get_node("%Name") as Label3D).text.begins_with("★"), "discovered marker is starred")
	_landmarks.apply([{"id": "dock", "name": "Причал", "x_dm": 0, "z_dm": 4000, "discovered": true}])
	_check(_landmarks.count() == 2 and dock.discovered, "update keeps others and flips discovered")


## Old bridge (Pack_build_2, two spans over the river east of the landmark): the deck is a
## walkable body whose ends meet the banks and whose middle spans the water.
func _check_bridge() -> void:
	var props: Node = _scene.get_node_or_null("Props")
	_check(props != null and props.get_node_or_null("BridgeWest") != null and props.get_node_or_null("BridgeEast") != null, "two bridge spans placed")
	var space := _scene.get_world_3d().direct_space_state
	for probe: Array in [[141.5, true], [157.5, false], [166.0, false], [174.0, true]]:
		var x: float = probe[0]
		var q := PhysicsRayQueryParameters3D.create(Vector3(x, 30.0, 140.0), Vector3(x, -10.0, 140.0), Interaction.WALK_LAYER)
		var hit := space.intersect_ray(q)
		var on_deck: bool = hit.has("collider") and String((hit["collider"] as Node).name) == "Body"
		_check(on_deck, "walk ray at x=%.1f lands on the bridge deck" % x)
		if on_deck and bool(probe[1]):
			var bank: float = _terrain.height_at(x + (-4.0 if x < 150.0 else 3.0), 140.0)
			_check(absf((hit["position"] as Vector3).y - bank) < 0.9, "deck end at x=%.1f meets the bank: %.2f vs %.2f" % [x, (hit["position"] as Vector3).y, bank])


func _check_nodes() -> void:
	_nodes.apply([
		{"cx": 8, "cz": 14, "nodes": [
			{"id": "a1", "kind": "rock", "x_dm": 60, "z_dm": 4050, "hits_left": 6},
			{"id": "a2", "kind": "tree_pine", "x_dm": -40, "z_dm": 4080, "hits_left": 0},
			{"id": "a3", "kind": "herb", "x_dm": 20, "z_dm": 4020, "hits_left": 1},
			{"id": "", "kind": "rock", "x_dm": 0, "z_dm": 0},
			{"id": "bad", "kind": "rock", "x_dm": "far", "z_dm": 0},
			"junk",
		]},
		"junk",
		{"cx": "x"},
	])
	_nodes.flush()
	_check(_nodes.count() == 3, "three valid nodes spawned, malformed ignored: %d" % _nodes.count())
	var rock := _nodes.node("a1")
	_check(rock != null and absf(rock.position.y - _terrain.ground_min(6.0, 405.0, 0.7)) < 0.01, "node sits on the lowest ground under it")
	_check(_terrain.ground_min(300.0, -200.0, 0.7) <= _terrain.height_at(300.0, -200.0), "ground_min never exceeds the centre height")
	_check(_terrain.normal_at(0.0, 60.0).is_equal_approx(Vector3.UP), "flat square has an upright normal")
	_check(rock != null and rock.get_node("Body/Collision").disabled == false, "rock has a collider")
	var herb := _nodes.node("a3")
	_check(herb != null and herb.get_node("Body/Collision").disabled, "herb is walk-through")
	var tree := _nodes.node("a2")
	_check(tree != null and tree.is_depleted() and not tree.visible and not tree.is_in_group(Interaction.GROUP), "used-up tree vanishes and cannot be targeted")
	_check(rock != null and rock.interaction_kind() == "mnode" and rock.interaction_id() == "a1", "interaction reports kind and id")
	# A fresh list for the chunk drops what is missing.
	_nodes.apply([{"cx": 8, "cz": 14, "nodes": [{"id": "a1", "kind": "rock", "x_dm": 60, "z_dm": 4050, "hits_left": 5}]}])
	_check(_nodes.count() == 1 and _nodes.node("a1").hits_left == 5, "chunk list replaces the old one")
	_nodes.apply("nope")
	_check(_nodes.count() == 1, "non-array payload ignored")
	# Something far away also spawns and must go when its chunk unloads later.
	_nodes.apply([{"cx": 8, "cz": 14, "nodes": [{"id": "a1", "kind": "rock", "x_dm": 60, "z_dm": 4050, "hits_left": 5}, {"id": "a9", "kind": "ore_iron", "x_dm": 100, "z_dm": 4100, "hits_left": 4}]}])
	_check(_nodes.count() == 2, "second node added")
	# One hit reported by the server: only that node's stock changes.
	_nodes.apply_one({"id": "a9", "hits_left": 0.0})
	_check(_nodes.node("a9").is_depleted() and _nodes.node("a1").hits_left == 5, "apply_one updates one node")
	_nodes.apply_one({"id": "ghost", "hits_left": 3})
	_check(_nodes.count() == 2, "unknown id ignored")
	# A node used up in one window and listed on its next reserved spot (0024): hidden while it
	# respawns, and back — full and visible — on the new spot once the server lists it so.
	_nodes.apply([{"cx": 8, "cz": 14, "nodes": [{"id": "a9", "kind": "ore_iron", "x_dm": 90, "z_dm": 4060, "hits_left": 0}]}])
	var moved := _nodes.node("a9")
	_check(moved != null and is_equal_approx(moved.position.x, 9.0) and is_equal_approx(moved.position.z, 406.0), "a listed node moves to the new spot: %s" % moved.position)
	_check(moved.is_depleted() and not moved.visible and not moved.is_in_group(Interaction.GROUP), "still hidden while it respawns")
	_nodes.apply([{"cx": 8, "cz": 14, "nodes": [{"id": "a9", "kind": "ore_iron", "x_dm": 90, "z_dm": 4060, "hits_left": 4}]}])
	_check(moved.visible and moved.is_in_group(Interaction.GROUP) and moved.hits_left == 4, "back and targetable on the new spot")
