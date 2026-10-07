class_name Interaction
extends Node
## Proximity: every `scan_every`-th physics tick finds the nearest node of group "interactable"
## (PlotNode, DecorSlot) within `radius` of the player, highlights it and tells
## React (`player_near`). React decides which action to offer; Godot never does.
## Also serves `walk_to` (React asks the character to approach a target) and
## `move_input` (joystick vector) from Bridge. Walks go through `nav_grid` when
## set, so the character rounds plots and props instead of pushing into them.

signal near_changed(kind: String, idx: int, distance: float)

const GROUP := "interactable"
const KIND_NONE := "none"
const KIND_BUILDING := "building"

@export var player: Node3D
## Optional route planner; without it the character walks in a straight line.
@export var nav_grid: NavGrid
## How close (to the object's centre) the character must be for it to count as "near".
@export var radius: float = 2.4
## Default distance from a target's centre where `walk_to` stops (targets may override
## with `approach_distance()` so the character halts at their edge, not inside them).
@export var stop_distance: float = 1.4
## Proximity pass every N physics ticks: on the mainland the group holds hundreds of
## nodes (≈0.5 ms per pass in wasm at 500); a highlight at 20 Hz is still instant.
@export var scan_every: int = 3

var current_kind: String = KIND_NONE
var current_idx: int = -1
var _current: Node3D = null
var _tick := 0


func _ready() -> void:
	var bridge: Node = get_node_or_null(^"/root/Bridge")
	if bridge != null:
		bridge.message_received.connect(_on_bridge_message)


func _physics_process(_delta: float) -> void:
	_tick += 1
	if _tick % maxi(scan_every, 1) == 0:
		scan()


## One proximity pass; public so tests can drive it without the physics loop.
func scan() -> void:
	if player == null:
		return
	# The highlighted node may be gone (bush cleared, decor removed, grid refreshed): a freed object equals null,
	# so without this reset the pass would see "still nothing new" and React would never hear `none`.
	if current_kind != KIND_NONE and not is_instance_valid(_current):
		_current = null
		current_kind = ""  # forces the report below even when nothing else is near
	var best := nearest(get_tree().get_nodes_in_group(GROUP), player.global_position, radius)
	var node: Node3D = best.get("node") as Node3D
	if node == _current and current_kind != "":
		return
	_set_highlight(_current, false)
	_set_highlight(node, true)
	_current = node
	current_kind = str(best.get("kind", KIND_NONE))
	current_idx = int(best.get("idx", -1))
	var distance: float = float(best.get("distance", 0.0))
	near_changed.emit(current_kind, current_idx, distance)
	if current_kind == KIND_BUILDING:
		var audio: Node = get_node_or_null(^"/root/Audio")
		if audio != null:
			audio.call("play_fx", "door_bell")  # міська будівля: дзвіночок над дверима, коли підійшов до входу
	var bridge: Node = get_node_or_null(^"/root/Bridge")
	if bridge != null:
		var payload := {"kind": current_kind, "idx": current_idx, "distance": snappedf(distance, 0.01)}
		if best.has("id"):
			payload["id"] = str(best["id"])
		bridge.send("player_near", payload)


## Physical layer the character walks against; pick-only bodies live on other layers.
const WALK_LAYER := 1
## Capsule radius (0.28) plus a hand's width: how far from a silhouette the character stops.
const STOP_MARGIN := 0.35


## Horizontal radius of a node's physical colliders (layer 1) around its origin,
## counting only geometry below `max_height` (waist height: a lantern arm, a hanging
## lamp or a tree crown must not keep the character away from the post or trunk).
static func body_radius(node: Node3D, max_height: float = 1.0) -> float:
	var radius := 0.0
	var origin := Vector2(node.global_position.x, node.global_position.z)
	for found: Node in node.find_children("*", "CollisionShape3D", true, false):
		var shape := found as CollisionShape3D
		var body := shape.get_parent() as CollisionObject3D
		if shape.shape == null or shape.disabled or body == null or (body.collision_layer & WALK_LAYER) == 0:
			continue
		if shape.shape is ConcavePolygonShape3D:
			for v: Vector3 in (shape.shape as ConcavePolygonShape3D).get_faces():
				var w := shape.global_transform * v
				if w.y <= max_height:
					radius = maxf(radius, Vector2(w.x, w.z).distance_to(origin))
			continue
		var box := NavGrid.world_aabb(shape)
		if box.position.y > max_height:
			continue
		for corner: int in 4:
			var x := box.position.x if corner % 2 == 0 else box.end.x
			var z := box.position.z if corner < 2 else box.end.z
			radius = maxf(radius, Vector2(x, z).distance_to(origin))
	return radius


## Nearest interactable by horizontal distance; empty dictionary when none within
## `max_distance` (a node may widen its own reach with `interaction_radius()`).
static func nearest(candidates: Array, origin: Vector3, max_distance: float) -> Dictionary:
	var best := {}
	var best_distance := INF
	var best_priority := 1 << 20
	for candidate: Variant in candidates:
		var node := candidate as Node3D
		if node == null or not node.has_method("interaction_kind"):
			continue
		var delta := node.global_position - origin
		delta.y = 0.0
		var distance := delta.length()
		var reach: float = node.interaction_radius() if node.has_method("interaction_radius") else max_distance
		# Lower `interaction_priority()` wins whatever the distance (a wild bush never hides a plot).
		var priority: int = node.interaction_priority() if node.has_method("interaction_priority") else 0
		if distance <= reach and (priority < best_priority or (priority == best_priority and distance < best_distance)):
			best_priority = priority
			best_distance = distance
			best = {"node": node, "kind": node.interaction_kind(), "idx": node.interaction_index(), "distance": distance}
			if node.has_method("interaction_id"):
				best["id"] = node.interaction_id()
	return best


## Interactable node by kind and index, or null.
func find_target(kind: String, idx: int) -> Node3D:
	for candidate: Node in get_tree().get_nodes_in_group(GROUP):
		var node := candidate as Node3D
		if node != null and node.has_method("interaction_kind") \
				and node.interaction_kind() == kind and node.interaction_index() == idx:
			return node
	return null


## Finds the target React named and walks the character next to it.
func walk_to(kind: String, idx: int) -> bool:
	if player == null or not player.has_method("walk_to_point"):
		return false
	var node := find_target(kind, idx)
	if node == null:
		push_warning("Interaction: no %s #%d to walk to" % [kind, idx])
		return false
	return walk_to_node(node)


## Walks up to an interactable that is already known (a click on it).
func walk_to_node(node: Node3D) -> bool:
	if player == null or not player.has_method("walk_to_point") or node == null:
		return false
	if node.has_method("approach_target"):
		# Doors and the like: one fixed spot, whichever side the character comes from.
		walk_to_position(node.approach_target(), 0.3)
		return true
	var distance: float = node.approach_distance() if node.has_method("approach_distance") else stop_distance
	# The approach point already sits at the target's edge: stop right on it, not another
	# `distance` short of it (that doubled the gap and left the character standing far away).
	walk_to_position(approach_point(node.global_position, player.global_position, distance), 0.15)
	return true


## Routes the character to `point` around obstacles (straight line without a NavGrid).
func walk_to_position(point: Vector3, stop_distance_override: float = 0.0) -> void:
	if player == null or not player.has_method("walk_path"):
		return
	var path := PackedVector3Array([point])
	if nav_grid != null:
		var routed := nav_grid.find_path(player.global_position, point)
		if not routed.is_empty():
			path = routed
	player.walk_path(path, stop_distance_override)


## Point `distance` away from `target` on the side the character is coming from.
static func approach_point(target: Vector3, from: Vector3, distance: float) -> Vector3:
	var away := from - target
	away.y = 0.0
	if away.length() < 0.001:
		away = Vector3(0.0, 0.0, 1.0)
	return target + away.normalized() * distance


func _on_bridge_message(type: String, payload: Dictionary) -> void:
	match type:
		"walk_to":
			walk_to(str(payload["kind"]), int(payload["idx"]))
		"move_input":
			if player != null and "external_input" in player:
				var x := clampf(float(payload["x"]), -1.0, 1.0)
				var y := clampf(float(payload["y"]), -1.0, 1.0)
				player.external_input = Vector2(x, y)
		"jump":
			if player != null and player.has_method("request_jump"):
				player.request_jump()


static func _set_highlight(node: Object, on: bool) -> void:
	if is_instance_valid(node) and node.has_method("set_highlighted"):
		node.call("set_highlighted", on)
