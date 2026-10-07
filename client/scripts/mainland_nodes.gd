class_name MainlandNodes
extends Node3D
## Places the server's resource nodes on the streamed terrain
## (`mainland_nodes { chunks: [{ cx, cz, nodes: [...] }] }`, docs/BRIDGE_PROTOCOL.md).
## A chunk's list replaces what was there; nodes vanish with their chunk when the
## terrain unloads it. Looks come from `kinds` (kind → piece scene) set in the
## scene; unknown kinds fall back to `fallback_scene`. Malformed entries are ignored.
##
## Spawning is spread over frames (`spawn_budget_ms` per frame) and limited to
## chunks within `spawn_radius` of the character's chunk: a dense forest window
## holds thousands of pieces, and instancing them all at once froze the game
## (owner feedback 2026-09-09). Chunks farther away keep their data and spawn
## when the character approaches. Nodes sit on the lowest ground under their
## footprint, rocks and ore tilt with the slope.

## Kinds that stand upright whatever the slope; everything else tilts with the ground.
const UPRIGHT_KINDS: Array[String] = ["tree_pine", "tree_oak", "tree_fallen", "herb", "mushroom", "mushroom_rare", "reed"]

@export var terrain: MainlandTerrain
@export var node_scene: PackedScene
@export var kinds: Dictionary = {}
@export var fallback_scene: PackedScene
## Crystals glow: applied to every mesh of a `crystal` node's piece.
@export var crystal_material: Material
## Chunks (Chebyshev distance from the character's chunk) whose nodes are instanced.
@export_range(0, 4) var spawn_radius: int = 2
## Milliseconds of instancing per frame; 0 spawns everything at once (tests).
@export var spawn_budget_ms: float = 4.0
## Footprint radius used to seat a node on the lowest ground under it.
@export var seat_radius: float = 0.7

var _nodes: Dictionary = {}
## chunk → { id: entry }: the latest server data, spawned or not.
var _entries: Dictionary = {}
var _entry_by_id: Dictionary = {}
var _pending: Array[Vector2i] = []
var _centre := Vector2i(0, 0)


func _ready() -> void:
	var bridge: Node = get_node_or_null(^"/root/Bridge")
	if bridge != null:
		bridge.message_received.connect(_on_bridge_message)
	if terrain != null:
		terrain.chunk_unloaded.connect(_on_chunk_unloaded)
		terrain.centre_changed.connect(_on_centre_changed)


func _process(_delta: float) -> void:
	if _pending.is_empty():
		return
	var deadline := Time.get_ticks_usec() + int(spawn_budget_ms * 1000.0)
	while not _pending.is_empty():
		var key: Vector2i = _pending[0]
		if _spawn_some(key, deadline):
			_pending.pop_front()
		else:
			return


## Spawns unspawned nodes of `key` until the deadline (0 = no limit); true when the chunk is complete.
func _spawn_some(key: Vector2i, deadline: int) -> bool:
	var chunk_entries: Dictionary = _entries.get(key, {})
	for id: String in chunk_entries:
		if _nodes.has(id):
			continue
		var e: Dictionary = chunk_entries[id]
		var node := _spawn(id, str(e["kind"]), key)
		if node != null:
			_place(node, e)
		if deadline > 0 and Time.get_ticks_usec() > deadline:
			return false
	return true


## Instances every pending node now (tests and scene changes).
func flush() -> void:
	while not _pending.is_empty():
		_spawn_some(_pending.pop_front(), 0)


func _on_bridge_message(type: String, payload: Dictionary) -> void:
	if type == "mainland_nodes":
		apply(payload.get("chunks", []))
	elif type == "mainland_node":
		apply_one(payload)


## `mainland_node { id, hits_left }`: one node's state after a hit (position unchanged).
func apply_one(entry: Dictionary) -> void:
	var id := str(entry.get("id", ""))
	var stored: Dictionary = _entry_by_id.get(id, {})
	if not stored.is_empty():
		stored["hits_left"] = entry.get("hits_left", stored.get("hits_left", 0))
	var node: MainlandNode = _nodes.get(id)
	if node != null:
		node.apply(entry)


func apply(raw: Variant) -> void:
	if typeof(raw) != TYPE_ARRAY:
		push_warning("MainlandNodes: chunks is not an array")
		return
	for entry: Variant in raw:
		if typeof(entry) != TYPE_DICTIONARY:
			continue
		var chunk_entry := entry as Dictionary
		if not _is_number(chunk_entry.get("cx")) or not _is_number(chunk_entry.get("cz")):
			continue
		var key := Vector2i(int(chunk_entry["cx"]), int(chunk_entry["cz"]))
		var nodes: Variant = chunk_entry.get("nodes", [])
		if typeof(nodes) != TYPE_ARRAY:
			continue
		_apply_chunk(key, nodes as Array)
	# A flat scene (the cave) has a handful of nodes: no need to spread them.
	if spawn_budget_ms <= 0.0 or terrain == null:
		flush()


func _apply_chunk(key: Vector2i, entries: Array) -> void:
	var listed: Dictionary = {}
	for entry: Variant in entries:
		if not _valid(entry):
			continue
		var e := entry as Dictionary
		var id := str(e["id"])
		listed[id] = e
		_entry_by_id[id] = e
		var node: MainlandNode = _nodes.get(id)
		if node != null:
			node.apply(e)
			_place(node, e)
	for id: String in _entries.get(key, {}):
		if not listed.has(id):
			_remove(id)
	_entries[key] = listed
	_queue_if_near(key)


func _queue_if_near(key: Vector2i) -> void:
	var d := key - _centre
	var near := terrain == null or maxi(absi(d.x), absi(d.y)) <= spawn_radius
	if near and not _pending.has(key):
		_pending.append(key)
		_pending.sort_custom(func(a: Vector2i, b: Vector2i) -> bool:
			return _centre.distance_squared_to(a) < _centre.distance_squared_to(b))


func _on_centre_changed(centre: Vector2i) -> void:
	_centre = centre
	for key: Vector2i in _entries.keys():
		var d := key - centre
		if maxi(absi(d.x), absi(d.y)) > spawn_radius + 1:
			_despawn_chunk(key)
		else:
			_queue_if_near(key)


## Lowest ground under the footprint, so a slope buries the uphill side instead of
## leaving the downhill side in the air; rocks and ore lean with the ground.
func _place(node: MainlandNode, e: Dictionary) -> void:
	var x := float(e["x_dm"]) / 10.0
	var z := float(e["z_dm"]) / 10.0
	if terrain == null:
		node.position = Vector3(x, 0.0, z)
		node.set_placed(true)
		return
	var kind := str(e["kind"])
	var r := seat_radius * (0.4 if kind in ["herb", "mushroom", "mushroom_rare", "reed"] else 1.0)
	var y := terrain.ground_min(x, z, r)
	node.position = Vector3(x, y, z)
	if kind in UPRIGHT_KINDS:
		node.basis = Basis.IDENTITY
	else:
		var n := terrain.normal_at(x, z)
		node.basis = Basis(Quaternion(Vector3.UP, n)) if n.y < 0.999 else Basis.IDENTITY
	# A node under water hides.
	node.set_placed(y >= 0.3)


func count() -> int:
	return _nodes.size()


func node(id: String) -> MainlandNode:
	return _nodes.get(id)


func pending_count() -> int:
	return _pending.size()


static func _is_number(v: Variant) -> bool:
	return typeof(v) == TYPE_FLOAT or typeof(v) == TYPE_INT


static func _valid(entry: Variant) -> bool:
	if typeof(entry) != TYPE_DICTIONARY:
		return false
	var e := entry as Dictionary
	if typeof(e.get("id")) != TYPE_STRING or str(e["id"]).is_empty():
		return false
	if typeof(e.get("kind")) != TYPE_STRING:
		return false
	return _is_number(e.get("x_dm")) and _is_number(e.get("z_dm"))


func _spawn(id: String, kind: String, key: Vector2i) -> MainlandNode:
	if node_scene == null:
		push_warning("MainlandNodes: no node scene")
		return null
	var node := node_scene.instantiate() as MainlandNode
	if node == null:
		return null
	node.chunk = key
	add_child(node)
	var look: PackedScene = kinds.get(kind, fallback_scene)
	node.setup(id, kind, look)
	if kind == "crystal" and crystal_material != null:
		for mesh: Node in node.find_children("*", "MeshInstance3D", true, false):
			if mesh.name != "Highlight":
				(mesh as MeshInstance3D).material_override = crystal_material
	_nodes[id] = node
	node.apply(_entry_by_id.get(id, {}))
	return node


func _remove(id: String) -> void:
	_entry_by_id.erase(id)
	_despawn(id)


func _despawn(id: String) -> void:
	var node: MainlandNode = _nodes.get(id)
	if node == null:
		return
	_nodes.erase(id)
	node.queue_free()


func _despawn_chunk(key: Vector2i) -> void:
	for id: String in _entries.get(key, {}):
		_despawn(id)
	_pending.erase(key)


func _on_chunk_unloaded(cx: int, cz: int) -> void:
	var key := Vector2i(cx, cz)
	for id: String in _entries.get(key, {}):
		_remove(id)
	_entries.erase(key)
	_pending.erase(key)
