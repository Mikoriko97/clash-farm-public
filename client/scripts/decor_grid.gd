class_name DecorGrid
extends Node3D
## Instances one DecorItem per server placement from `world_state.decor`
## (`{ id, item_id, x_dm, z_dm, yaw_deg }`: decimetres from the island centre,
## yaw in degrees). `full` or `decor_full` payloads drop unlisted placements.
## Nothing here is placed by hand; positions always come from the server.

const GROUP := "decor_grid"

@export var item_scene: PackedScene

var _items: Dictionary = {}  # placement id -> DecorItem
var _next_idx: int = 0
var _probe: DecorItem


func _ready() -> void:
	add_to_group(GROUP)
	var bridge: Node = get_node_or_null(^"/root/Bridge")
	if bridge != null:
		bridge.message_received.connect(_on_bridge_message)


func item(placement_id: String) -> DecorItem:
	return _items.get(placement_id) as DecorItem


func item_count() -> int:
	return _items.size()


## Piece scene for a catalog item (the Placer's ghost uses it); null when unknown.
func piece_for(item_id: String) -> PackedScene:
	if _probe == null and item_scene != null:
		_probe = item_scene.instantiate() as DecorItem
	return _probe.piece_for(item_id) if _probe != null else null


func _on_bridge_message(type: String, payload: Dictionary) -> void:
	if type == "world_state":
		apply_world_state(payload)


func apply_world_state(payload: Dictionary) -> void:
	var entries: Variant = payload.get("decor", [])
	if typeof(entries) != TYPE_ARRAY:
		return
	var replace_all: bool = bool(payload.get("full", false)) or bool(payload.get("decor_full", false))
	var listed: Dictionary = {}
	for entry: Variant in entries:
		if typeof(entry) != TYPE_DICTIONARY:
			continue
		var node := _ensure(entry as Dictionary)
		if node != null:
			listed[node.placement_id] = true
	if replace_all:
		for id: String in _items.keys():
			if not listed.has(id):
				(_items[id] as Node).queue_free()
				_items.erase(id)
	if is_inside_tree():
		get_tree().call_group(NavGrid.GROUP, "mark_dirty")


## Existing placement by id, or a new one at the entry's point; null for a malformed entry.
func _ensure(entry: Dictionary) -> DecorItem:
	var id := str(entry.get("id", ""))
	var x: Variant = entry.get("x_dm")
	var z: Variant = entry.get("z_dm")
	if id.is_empty() or not _is_number(x) or not _is_number(z):
		push_warning("DecorGrid: malformed placement %s" % id)
		return null
	var node := item(id)
	if node == null:
		if item_scene == null:
			return null
		node = item_scene.instantiate() as DecorItem
		node.placement_id = id
		node.idx = _next_idx
		node.name = "Decor%d" % _next_idx
		_next_idx += 1
		add_child(node)
		_items[id] = node
	node.position = Vector3(float(x) / 10.0, 0.0, float(z) / 10.0)
	var yaw: Variant = entry.get("yaw_deg", 0)
	node.rotation.y = deg_to_rad(float(yaw)) if _is_number(yaw) else 0.0
	node.apply(str(entry.get("item_id", "")))
	return node


static func _is_number(v: Variant) -> bool:
	return typeof(v) == TYPE_FLOAT or typeof(v) == TYPE_INT
