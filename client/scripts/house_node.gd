class_name HouseNode
extends Node3D
## The island house (owner backlog 2026-09-05): an interactable like plots and
## resources (`player_near { kind: "house", idx: 0 }`), plus "being inside".
## React sends `house { inside: true }`: the character walks to the threshold,
## glides through the door and vanishes; Godot answers `house_entered` and React
## opens its panel. `house { inside: false }`: the character reappears in the
## doorway and walks back out. Pure view: sleeping and cooking are server actions.

signal entered
## The sleep sequence (lie down → sleep → get up) ended.
signal slept

const KIND := "house"

## The door in the house's local space (littlehouse_a: on the +z gable, left of centre;
## measured on renders, see tools/snapshot.gd --player=door).
@export var door_local: Vector3 = Vector3(-0.86, 0.0, 1.35)
## Outward normal of the door wall, local space.
@export var door_normal: Vector3 = Vector3(0.0, 0.0, 1.0)
## Where the character stands before the door (m outward from it).
@export var threshold_gap: float = 1.0
## How far past the door the character walks before it is hidden (m).
@export var inside_depth: float = 1.4
## Seconds for the glide through the door, each way.
@export var glide_secs: float = 0.9
## Nearest-target radius: the threshold is farther from the centre than a plot's edge.
@export var near_radius: float = 4.6
@export var player: Node3D
## Routes the walk to the threshold around obstacles; unset = glide straight there.
@export var interaction: Interaction
## Seconds the screen stays dark while sleeping.
@export var sleep_secs: float = 2.0

var _sleep_pending: bool = false

var inside: bool = false
## True once the character is actually hidden indoors (the walk in may still be under way).
var _entered: bool = false
## A walk in is under way and React still waits for its `house_entered`.
var _entering: bool = false
## Bumped by every `set_inside`: a walk in that sees another number was cancelled (or replaced) meanwhile.
var _attempt: int = 0

@onready var _highlight: MeshInstance3D = get_node_or_null(^"%Highlight")


func _ready() -> void:
	add_to_group(Interaction.GROUP)
	var bridge: Node = get_node_or_null(^"/root/Bridge")
	if bridge != null:
		bridge.message_received.connect(_on_bridge_message)


func interaction_kind() -> String:
	return KIND


func interaction_index() -> int:
	return 0


func interaction_radius() -> float:
	return near_radius


## Interaction.walk_to brings the character here: in front of the door, not "near the centre".
func approach_target() -> Vector3:
	return threshold_position()


func set_highlighted(on: bool) -> void:
	if _highlight != null:
		_highlight.visible = on


func door_position() -> Vector3:
	return to_global(door_local)


func threshold_position() -> Vector3:
	return to_global(door_local + _outward() * threshold_gap)


func inside_position() -> Vector3:
	return to_global(door_local - _outward() * inside_depth)


func _outward() -> Vector3:
	var flat := Vector3(door_normal.x, 0.0, door_normal.z)
	return flat.normalized() if flat.length() > 0.001 else Vector3(0.0, 0.0, 1.0)


func _on_bridge_message(type: String, payload: Dictionary) -> void:
	if type == "house":
		set_inside(bool(payload["inside"]))
	elif type == "play_fx" and str(payload.get("fx", "")) == "sleep":
		request_sleep()


## React reports a successful sleep while its panel is still open: the night
## passes as soon as the panel closes, before the character walks back out.
func request_sleep() -> void:
	_sleep_pending = true
	if not inside and not _entered:
		_sleep()


## Night passes inside: the screen goes dark while the character is still indoors,
## then it steps back out. The lie-down clips wait for a bed (no interior yet).
func _sleep() -> void:
	_sleep_pending = false
	var vignette := get_tree().get_first_node_in_group("vignette") as CanvasItem
	_fade(vignette, 0.98, 0.6)
	await get_tree().create_timer(sleep_secs).timeout
	_fade(vignette, 0.2, 1.0)
	slept.emit()


func _fade(vignette: CanvasItem, strength: float, secs: float) -> void:
	if vignette == null or not (vignette.material is ShaderMaterial):
		return
	var tween := create_tween()
	tween.tween_method(
		func(v: float) -> void: (vignette.material as ShaderMaterial).set_shader_parameter("strength", v),
		float((vignette.material as ShaderMaterial).get_shader_parameter("strength")), strength, secs)


func set_inside(value: bool) -> void:
	if value == inside:
		return
	inside = value
	_attempt += 1
	if player == null:
		return
	if inside:
		_entering = true
		_enter(_attempt)
	else:
		_cancel_entry()
		_leave()


## Walk to the threshold (routed), glide through the door, vanish, tell React
## (`house_entered { ok }`). A click elsewhere or a modal cancels the walk:
## then `ok` is false and the character simply stays outside.
func _enter(attempt: int) -> void:
	if player.has_method("glide_to"):
		var threshold := threshold_position()
		if _far_from(threshold) and interaction != null:
			interaction.walk_to_position(threshold, 0.3)
			while attempt == _attempt and player.is_walking_to_target():
				await get_tree().physics_frame
			if attempt != _attempt:
				return  # `house { inside: false }` already answered `ok: false`
			if _far_from(threshold):
				inside = false
				_cancel_entry()
				return
		_door_sound("door_open")
		await player.glide_to(inside_position(), glide_secs)
		if attempt != _attempt:
			# Cancelled mid-door: step back out to the threshold, visible and free to walk again.
			if not inside:
				await player.glide_to(threshold_position(), glide_secs)
			return
	else:
		player.global_position = inside_position()
	player.visible = false
	_door_shut()
	_entered = true
	_entering = false
	entered.emit()
	_send_entered(true)


## The walk in stopped before the character got through the door: React stops waiting (`ok: false`, no panel).
func _cancel_entry() -> void:
	if _entering:
		_entering = false
		_send_entered(false)


## Звук дверей (автолоад `Audio`; у headless-тестах його нема — тоді мовчки нічого).
func _door_sound(fx: String) -> void:
	var audio: Node = get_node_or_null(^"/root/Audio")
	if audio != null:
		audio.call("play_fx", fx)


## Двері зачиняються слідом: коли персонаж зник за ними чи вийшов.
func _door_shut() -> void:
	await get_tree().create_timer(0.25).timeout
	_door_sound("door_close")


func _far_from(point: Vector3) -> bool:
	var flat := player.global_position - point
	flat.y = 0.0
	return flat.length() > 0.6


func _send_entered(ok: bool) -> void:
	var bridge: Node = get_node_or_null(^"/root/Bridge")
	if bridge != null:
		bridge.send("house_entered", {"ok": ok})


## Reappear in the doorway and walk out to the threshold. If the walk in never
## finished (React opened its panel on the fallback timer), the character is
## still outside and nothing needs to move.
func _leave() -> void:
	if not _entered:
		return
	if _sleep_pending:
		await _sleep()
	_entered = false
	player.global_position = inside_position()
	player.visible = true
	if player.has_method("glide_to"):
		_door_sound("door_open")
		await player.glide_to(threshold_position(), glide_secs)
		_door_shut()
	else:
		player.global_position = threshold_position()
