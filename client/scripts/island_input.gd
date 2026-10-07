extends Node3D
## Pointer input for the island (attached to Island.tscn root): a tap/click on the
## ground walks the character there; on a plot or decor slot it also reports the
## selection and walks next to it; a drag orbits the camera. Fingers go through
## `TouchGestures` (the first finger on the view owns the camera, a joystick thumb never
## reaches here); mouse events that echo a touch are dropped. Ignored entirely while React
## owns the screen (`set_mode ui`).

const TAP_SLOP_PX := 14.0
const RAY_LENGTH := 200.0

@onready var _player: Player = $Player
@onready var _camera_rig: CameraRig = $CameraRig
@onready var _interaction: Interaction = $Interaction
@onready var _placer: Placer = get_node_or_null(^"Placer")
@onready var _day_cycle: DayCycle = get_node_or_null(^"DayCycle")
@onready var _weather: Weather = get_node_or_null(^"Weather")

var _press_pos: Vector2 = Vector2.ZERO
var _pressed: bool = false
var _dragging: bool = false
var _touch := TouchGestures.new()


func _ready() -> void:
	var bridge: Node = get_node_or_null(^"/root/Bridge")
	if bridge != null:
		bridge.message_received.connect(_on_bridge_message)
		bridge.mode_changed.connect(_on_mode_changed)


## A React panel took the screen: a press that began before it must not orbit the camera after it closes
## (the button was released over the panel, so this node never saw the release).
func _on_mode_changed(_mode: String) -> void:
	_touch.reset()
	_pressed = false
	_dragging = false


## The character acts out what the server just confirmed: turn to the target, play the clip.
func _on_bridge_message(type: String, payload: Dictionary) -> void:
	if type == "debug_world":
		_debug_world(payload)
		return
	if type == "teleport":
		_teleport(payload)
		return
	if type != "play_fx":
		return
	var fx := str(payload.get("fx", ""))
	var target: Variant = payload.get("target", {})
	if fx == "sit":
		if typeof(target) == TYPE_DICTIONARY:
			_sit(int((target as Dictionary).get("decor_idx", -1)))
		return
	var entry: Variant = FxClips.TABLE.get(fx)
	if entry == null:
		return
	var clip: StringName = (entry as Dictionary)["clip"]
	var life := _player.get_node_or_null(^"Life") as CharacterLife
	if (entry as Dictionary).get("after", false) and life != null:
		life.react(clip)
		return
	var tool := str(payload.get("tool", (entry as Dictionary)["tool"]))
	if typeof(target) == TYPE_DICTIONARY and (target as Dictionary).has("node_id"):
		# A shared mainland node: the character turns to it and it shakes.
		var nodes: MainlandNodes = get_node_or_null(^"Nodes") as MainlandNodes
		var mnode: MainlandNode = nodes.node(str((target as Dictionary)["node_id"])) if nodes != null else null
		if mnode != null:
			_player.face_point(mnode.global_position)
			mnode.play_fx(fx)
	elif typeof(target) == TYPE_DICTIONARY and (target as Dictionary).has("bush_id"):
		_player.face_point(WildBush.point_of(str((target as Dictionary)["bush_id"]), _player.global_position))
	elif typeof(target) == TYPE_DICTIONARY:
		var t := target as Dictionary
		var kind := "plot" if t.has("plot_idx") else ("resource" if t.has("resource_idx") else "decor")
		var idx := int(t.get("plot_idx", t.get("resource_idx", t.get("decor_idx", t.get("slot", -1)))))
		var node := _interaction.find_target(kind, idx)
		if node != null:
			_player.face_point(node.global_position)
	_player.play_action(clip, tool)


func _unhandled_input(event: InputEvent) -> void:
	var bridge: Node = get_node_or_null(^"/root/Bridge")
	if bridge != null and bridge.mode == "ui":
		return
	if _touch.handle(event, _camera_rig.orbit, func(at: Vector2) -> void: _click(at, true)):
		return
	if event is InputEventMouse and ((event as InputEventMouse).device == InputEvent.DEVICE_ID_EMULATION or _touch.owns_pointer()):
		return
	if event is InputEventMouseButton:
		_on_button(event as InputEventMouseButton)
	elif event is InputEventMouseMotion:
		_on_motion(event as InputEventMouseMotion)
	elif event is InputEventKey and (event as InputEventKey).pressed and not (event as InputEventKey).echo:
		_on_key(event as InputEventKey)


## R turns the decor ghost 15° (Shift+R the other way) while building.
func _on_key(event: InputEventKey) -> void:
	if _placer == null or not _placer.active or event.keycode != KEY_R:
		return
	_placer.rotate_ghost(-15 if event.shift_pressed else 15)


func _on_button(event: InputEventMouseButton) -> void:
	if event.button_index != MOUSE_BUTTON_LEFT:
		return
	if event.pressed:
		_press_pos = event.position
		_pressed = true
		_dragging = false
		return
	if _pressed and not _dragging:
		# Pointer lock (MouseLook) freezes the cursor where it was captured: aim at the view centre, where the camera looks.
		_click(get_viewport().get_visible_rect().get_center() if Input.mouse_mode == Input.MOUSE_MODE_CAPTURED else event.position)
	_pressed = false


func _on_motion(event: InputEventMouseMotion) -> void:
	# Захоплений курсор (MouseLook): сам рух миші обертає камеру.
	if Input.mouse_mode == Input.MOUSE_MODE_CAPTURED:
		_camera_rig.orbit(event.relative.x, event.relative.y)
		return
	if event.button_mask & MOUSE_BUTTON_MASK_RIGHT:
		_camera_rig.orbit(event.relative.x, event.relative.y)
		return
	if not _pressed:
		if _placer != null and _placer.active:
			var hit := _ray_hit(event.position)
			if not hit.is_empty():
				_placer.hover(hit["position"] as Vector3)
		return
	if not _dragging and (event.position - _press_pos).length() > TAP_SLOP_PX:
		_dragging = true
	if _dragging:
		_camera_rig.orbit(event.relative.x, event.relative.y)


## Hip height of the sitting clip above the character origin (feet on the ground); 4.5-head base (ADR-032): hips 0.33 in the clip.
const SIT_HIPS := 0.36


## Sits on bench-like decor: the seat top comes from the trimesh collider, the
## backrest from the tallest vertices, so the character faces away from it with
## its feet on the ground and its hips on the seat. Holds until the player moves.
func _sit(decor_idx: int) -> void:
	var node := _interaction.find_target("decor", decor_idx)
	if node == null or not _player.has_method("glide_to"):
		return
	if not node.has_method("piece_meshes"):
		return
	var meshes: Array[MeshInstance3D] = node.piece_meshes()
	if meshes.is_empty():
		return
	var box := meshes[0].global_transform * meshes[0].get_aabb()
	for mesh: MeshInstance3D in meshes:
		box = box.merge(mesh.global_transform * mesh.get_aabb())
	var centre := box.get_center()
	var forward := -_backrest_direction(meshes, box)
	var seat_y := _surface_height(Vector3(centre.x, box.end.y + 0.5, centre.z), box.position.y)
	var origin := Vector3(centre.x, node.global_position.y, centre.z) + forward * 0.12
	origin.y += clampf(seat_y - node.global_position.y - SIT_HIPS, -0.1, 0.35)
	var exit_point := Vector3(centre.x, node.global_position.y, centre.z) + forward * 0.9
	_player.sit_at(origin, forward, exit_point)


## Where the tall part of the piece is (backrest), as a horizontal world direction.
static func _backrest_direction(meshes: Array[MeshInstance3D], box: AABB) -> Vector3:
	var threshold := box.position.y + box.size.y * 0.7
	var sum := Vector3.ZERO
	var count := 0
	for mesh: MeshInstance3D in meshes:
		if mesh.mesh == null:
			continue
		for v: Vector3 in mesh.mesh.get_faces():
			var w := mesh.global_transform * v
			if w.y >= threshold:
				sum += w - box.get_center()
				count += 1
	if count == 0:
		# No backrest: face across the short side of the piece.
		return Vector3(0.0, 0.0, 1.0) if box.size.x >= box.size.z else Vector3(1.0, 0.0, 0.0)
	var dir := sum / float(count)
	dir.y = 0.0
	return dir.normalized() if dir.length() > 0.01 else Vector3(0.0, 0.0, 1.0)


## First surface below `from` (the seat top), or `fallback` when nothing is hit.
func _surface_height(from: Vector3, fallback: float) -> float:
	var query := PhysicsRayQueryParameters3D.create(from, from + Vector3.DOWN * 4.0)
	var hit := get_world_3d().direct_space_state.intersect_ray(query)
	return float((hit["position"] as Vector3).y) if not hit.is_empty() else fallback


## Dev controls from the HUD menu (docs/BRIDGE_PROTOCOL.md `debug_world`). Visual and
## client-local only, so they need no build gate; the HUD shows them in dev builds.
func _debug_world(payload: Dictionary) -> void:
	if payload.has("hour") and _day_cycle != null:
		if payload["hour"] == null:
			_day_cycle.unfreeze()
		else:
			_day_cycle.freeze(float(payload["hour"]))
	if payload.has("day_length_min") and _day_cycle != null:
		_day_cycle.day_length_min = clampf(float(payload["day_length_min"]), 1.0, 1440.0)
	if payload.has("weather") and _weather != null:
		if str(payload["weather"]) == "auto":
			_weather.auto()
		else:
			_weather.force(Weather.state_from_name(str(payload["weather"])))
	if payload.get("lightning", false) and _weather != null:
		_weather.strike()
	if payload.get("probe", false):
		_probe()
	if payload.has("stats"):
		var stats: Node = get_node_or_null(^"/root/DebugStats")
		if stats != null:
			stats.call("set_enabled", bool(payload["stats"]))


## `debug_world { probe: true }` → `debug_probe` (WorldProbe; dev only).
func _probe() -> void:
	# A scene on its way out (zone change) still hears the bridge for a frame: stay quiet.
	if not is_inside_tree():
		return
	var bridge: Node = get_node_or_null(^"/root/Bridge")
	if bridge != null:
		bridge.send("debug_probe", WorldProbe.report(self, _player, _camera_rig))


## The server did not believe where the character stood: put it where the server says
## (same zone only; a zone mismatch resolves itself with the next set_zone).
func _teleport(payload: Dictionary) -> void:
	var bridge: Node = get_node_or_null(^"/root/Bridge")
	if bridge != null and str(payload.get("zone", "")) != str(bridge.get("zone")):
		return
	var x := float(payload["x_dm"]) / 10.0
	var z := float(payload["z_dm"]) / 10.0
	var terrain: Node = get_node_or_null(^"Terrain")
	var y: float = float(terrain.call("height_at", x, z)) + 0.3 if terrain != null else _player.global_position.y
	_player.cancel_walk()
	_player.global_position = Vector3(x, y, z)


func _ray_hit(screen_pos: Vector2) -> Dictionary:
	var camera := get_viewport().get_camera_3d()
	if camera == null:
		return {}
	var origin := camera.project_ray_origin(screen_pos)
	var to := origin + camera.project_ray_normal(screen_pos) * RAY_LENGTH
	return get_world_3d().direct_space_state.intersect_ray(PhysicsRayQueryParameters3D.create(origin, to))


func _click(screen_pos: Vector2, from_touch: bool = false) -> void:
	var hit := _ray_hit(screen_pos)
	if hit.is_empty():
		return
	if _placer != null and _placer.active:
		_placer.click(hit["position"] as Vector3)
		return
	var collider: Object = hit.get("collider", null)
	var node: Node = (collider as Node).get_parent() if collider is Node else null
	if node is PlotNode:
		(node as PlotNode).report_selected()
		_interaction.walk_to("plot", (node as PlotNode).idx)
	elif node is ResourceNode:
		_interaction.walk_to("resource", (node as ResourceNode).idx)
	elif node is DecorItem:
		_interaction.walk_to("decor", (node as DecorItem).idx)
	elif node is HouseNode:
		_interaction.walk_to(HouseNode.KIND, 0)
	elif node is MainlandNode or node is LandmarkNode or node is CityBuilding:
		_interaction.walk_to_node(node)
	elif node is IslandWild and (node as IslandWild).approach_bush(_interaction, collider, int(hit.get("shape", -1))):
		pass  # a click on a wild bush: walk up to it, the HUD offers «Clear bush»
	else:
		# На ПК клік по землі захоплює мишу для огляду (далі WASD); на сенсорних — тап веде туди.
		var look: Node = get_node_or_null(^"/root/MouseLook")
		if from_touch or look == null or not look.call("capture"):
			_interaction.walk_to_position(hit["position"] as Vector3)
