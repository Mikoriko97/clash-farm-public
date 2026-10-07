class_name MainlandLandmarks
extends Node3D
## Places landmark markers from `mainland_landmarks { landmarks: [...] }`
## (docs/BRIDGE_PROTOCOL.md): the server only lists landmarks the player has
## discovered or whose chunk is explored, so markers appear as the fog lifts.
## A dozen at most: they stay loaded for the whole visit. Malformed entries are ignored.

@export var terrain: MainlandTerrain
@export var marker_scene: PackedScene

var _markers: Dictionary = {}


func _ready() -> void:
	var bridge: Node = get_node_or_null(^"/root/Bridge")
	if bridge != null:
		bridge.message_received.connect(_on_bridge_message)


func _on_bridge_message(type: String, payload: Dictionary) -> void:
	if type == "mainland_landmarks":
		apply(payload.get("landmarks", []))


func apply(raw: Variant) -> void:
	if typeof(raw) != TYPE_ARRAY:
		push_warning("MainlandLandmarks: landmarks is not an array")
		return
	for entry: Variant in raw:
		if not _valid(entry):
			continue
		var e := entry as Dictionary
		var id := str(e["id"])
		var marker: LandmarkNode = _markers.get(id)
		if marker == null:
			marker = _spawn(id)
			if marker == null:
				continue
		marker.apply(e)
		var x := float(e["x_dm"]) / 10.0
		var z := float(e["z_dm"]) / 10.0
		var y := terrain.ground_min(x, z, 0.4) if terrain != null else 0.0
		marker.position = Vector3(x, maxf(y, 0.0), z)


func count() -> int:
	return _markers.size()


func marker(id: String) -> LandmarkNode:
	return _markers.get(id)


static func _valid(entry: Variant) -> bool:
	if typeof(entry) != TYPE_DICTIONARY:
		return false
	var e := entry as Dictionary
	if typeof(e.get("id")) != TYPE_STRING or str(e["id"]).is_empty():
		return false
	if typeof(e.get("name")) != TYPE_STRING:
		return false
	for key: String in ["x_dm", "z_dm"]:
		var v: Variant = e.get(key)
		if typeof(v) != TYPE_FLOAT and typeof(v) != TYPE_INT:
			return false
	return true


func _spawn(id: String) -> LandmarkNode:
	if marker_scene == null:
		push_warning("MainlandLandmarks: no marker scene")
		return null
	var marker := marker_scene.instantiate() as LandmarkNode
	if marker == null:
		return null
	marker.name = "Landmark_" + id
	add_child(marker)
	_markers[id] = marker
	return marker
