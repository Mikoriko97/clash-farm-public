extends SceneTree
## Headless test: ResourceGrid feeds world_state.resources to its ResourceNode
## children, a depleted node vanishes, garbage is ignored, and the
## node reports itself as an interactable of kind "resource"; since 0017 the server's
## point moves a node, its stage scales it, an unknown idx is instanced from the
## kind's scene and a full snapshot drops dynamic nodes it no longer lists.
## Run: godot --headless --path client --script res://tests/test_resource_grid.gd

var _failures := 0
var _ran := false


func _process(_delta: float) -> bool:
	if _ran:
		quit(1)
		return true
	_ran = true
	var grid := ResourceGrid.new()
	root.add_child(grid)
	grid.add_child(_node(0, "tree"))
	grid.add_child(_node(3, "rock"))
	grid.apply_world_state({"resources": [
		{"idx": 0, "kind": "tree", "hits_left": 5},
		{"idx": 3, "kind": "rock", "hits_left": 0, "respawn_at": "2026-09-04T18:00:00Z"},
		{"idx": 9, "kind": "tree", "hits_left": 2},
		{"idx": 0, "hits_left": "lots"},
		"garbage",
	]})
	var tree := grid.node(0)
	var rock := grid.node(3)
	_check(tree != null and tree.hits_left == 5 and not tree.is_depleted(), "tree keeps 5 hits")
	_check(rock != null and rock.is_depleted() and rock.respawn_at_unix > 0.0, "rock is depleted with a respawn time")
	_check(not rock.visible and not rock.is_in_group(Interaction.GROUP) and tree.visible and tree.get_node("Piece").scale == Vector3.ONE, "depleted node vanishes (hidden, not targetable), live one stays")
	_check(tree.interaction_kind() == "resource" and tree.interaction_index() == 0, "interactable of kind resource")
	_check(tree.is_in_group(Interaction.GROUP), "live node is in the interactable group")
	_check(grid.node(9) == null, "unknown idx ignored")
	grid.apply_world_state({"resources": [{"idx": 3, "kind": "rock", "hits_left": 8}]})
	_check(not rock.is_depleted() and rock.visible and rock.is_in_group(Interaction.GROUP) and rock.get_node("Piece").scale == Vector3.ONE, "regrown rock is back: visible, targetable, full size")
	_check_living()
	print("RESULT: %s (%d failures)" % ["PASS" if _failures == 0 else "FAIL", _failures])
	quit(1 if _failures > 0 else 0)
	return true


## 0017: server-owned points, stages and dynamic nodes.
func _check_living() -> void:
	var grid := ResourceGrid.new()
	# Prop scenes ship without the ResourceNode script (Island.tscn attaches it): the grid must add it.
	var packed := PackedScene.new()
	var proto := Node3D.new()
	var piece := Node3D.new()
	piece.name = "Piece"
	proto.add_child(piece)
	piece.owner = proto
	packed.pack(proto)
	proto.free()
	grid.tree_scene = packed
	grid.rock_scene = packed
	root.add_child(grid)
	grid.add_child(_node(0, "tree"))
	grid.apply_world_state({"full": true, "resources": [
		{"idx": 0, "kind": "tree", "hits_left": 5, "x_dm": -160, "z_dm": -140, "stage": 2},
		{"idx": 9, "kind": "tree", "hits_left": 5, "x_dm": 120, "z_dm": -300, "stage": 0},
		{"idx": 10, "kind": "rock", "hits_left": 8, "x_dm": -50, "z_dm": 200, "stage": 2},
	]})
	var base := grid.node(0)
	_check(base != null and base.position.is_equal_approx(Vector3(-16.0, 0.0, -14.0)), "starter node moved to the server's point")
	var sapling := grid.node(9)
	_check(sapling != null and sapling.position.is_equal_approx(Vector3(12.0, 0.0, -30.0)), "unknown idx instanced at the server's point")
	_check(sapling != null and not sapling.is_grown() and sapling.get_node("Piece").scale.x < 0.5, "sapling is small and not gatherable")
	_check(grid.node(10) != null and grid.node(10).kind == "rock", "rock instanced by kind")
	grid.apply_world_state({"full": false, "resources": [{"idx": 9, "kind": "tree", "hits_left": 5, "stage": 2}]})
	_check(sapling.is_grown() and sapling.get_node("Piece").scale == Vector3.ONE, "patch grows the sapling to full size")
	grid.apply_world_state({"full": true, "resources": [{"idx": 0, "kind": "tree", "hits_left": 5}]})
	_check(grid.node(9) == null and grid.node(10) == null and grid.node(0) != null, "full snapshot drops dynamic nodes, keeps the scene one")


func _node(idx: int, kind: String) -> ResourceNode:
	var n := ResourceNode.new()
	n.idx = idx
	n.kind = kind
	var piece := Node3D.new()
	piece.name = "Piece"
	n.add_child(piece)
	return n


func _check(ok: bool, what: String) -> void:
	if not ok:
		_failures += 1
	print("%s %s" % ["ok  " if ok else "FAIL", what])
