class_name CameraRig
extends Node3D
## Third-person follow camera: fixed pitch, yaw set in the editor and orbited by
## drag (`orbit`), smooth follow of `target`, mouse-wheel zoom. Editable in CameraRig.tscn.
## Follows on the physics tick, like the character moves, so physics interpolation
## (project setting) smooths both between ticks on any refresh rate.
## The camera hangs on a SpringArm3D: when ground or a building is between the
## character and the wanted camera spot, the arm shortens so the camera never
## sinks under a hillside (owner feedback 2026-09-09, mountains).

@export var target: Node3D
@export var follow_speed: float = 6.0
@export var zoom_step: float = 1.5
@export var min_distance: float = 6.0
@export var max_distance: float = 24.0
## Degrees of yaw per pixel of horizontal drag.
@export var orbit_speed: float = 0.25
## Pitch limits (degrees below the horizon): never under the ground, never straight down.
@export var min_pitch: float = 15.0
@export var max_pitch: float = 75.0
## Point the arm pivots around, above the character's feet.
@export var pivot_height: float = 1.2
## Створення персонажа (`camera_preview`): близько, майже на рівні очей, персонаж збоку від панелі.
@export var preview_distance: float = 3.4
@export var preview_pitch: float = 6.0
@export var preview_shift: float = 0.75
## У портреті панель знизу: камера трохи далі, а персонажа піднімають (`v_offset`), а не зсувають вбік.
@export var preview_portrait_distance: float = 4.6
@export var preview_lift: float = 1.0
## Фокус «голова» (вкладки волосся, обличчя, головного убору): камера на відстані витягнутої руки від обличчя.
@export var preview_head_distance: float = 1.7
@export var preview_head_portrait_distance: float = 2.5
@export var preview_head_pivot: float = 1.25
@export var preview_head_lift: float = 0.5

@onready var _camera: Camera3D = %Camera3D
@onready var _arm: SpringArm3D = %Arm
@onready var _pitch: Node3D = $Yaw/Pitch

## Стан камери до перегляду персонажа; порожній, коли перегляду нема.
var _saved: Dictionary = {}


func _ready() -> void:
	if _arm != null and target != null and target is CollisionObject3D:
		_arm.add_excluded_object((target as CollisionObject3D).get_rid())
	var bridge: Node = get_node_or_null(^"/root/Bridge")
	if bridge != null:
		bridge.message_received.connect(_on_bridge_message)


func _physics_process(delta: float) -> void:
	if target == null:
		return
	var weight := minf(follow_speed * delta, 1.0)
	global_position = global_position.lerp(target.global_position + Vector3(0.0, pivot_height, 0.0), weight)


func _unhandled_input(event: InputEvent) -> void:
	if event.is_action_pressed(&"zoom_in"):
		_zoom(-zoom_step)
	elif event.is_action_pressed(&"zoom_out"):
		_zoom(zoom_step)


## Rotates the rig around the target: horizontal drag = yaw, vertical drag = pitch.
func orbit(dx_px: float, dy_px: float = 0.0) -> void:
	rotation.y = wrapf(rotation.y - deg_to_rad(dx_px * orbit_speed), -PI, PI)
	if _pitch == null or dy_px == 0.0:
		return
	var pitch := rad_to_deg(-_pitch.rotation.x) + dy_px * orbit_speed
	_pitch.rotation.x = -deg_to_rad(clampf(pitch, min_pitch, max_pitch))


func _on_bridge_message(type: String, payload: Dictionary) -> void:
	if type == "camera_preview":
		preview(bool(payload["on"]), float(payload.get("yaw_deg", 0.0)), String(payload.get("focus", "body")))


## Екран створення персонажа: камера впритул, персонаж дивиться в неї, `yaw_deg` повертає
## самого персонажа (стрілки в панелі — оглянути збоку й ззаду). Вимкнення повертає звичайну
## камеру. Рух у цей час вимкнено режимом `ui` з React.
func preview(on: bool, yaw_deg: float = 0.0, focus: String = "body") -> void:
	if not on:
		if not _saved.is_empty():
			min_distance = _saved["min"]
			_pitch.rotation.x = _saved["pitch"]
			_arm.spring_length = _saved["length"]
			pivot_height = _saved["pivot"]
			_camera.h_offset = 0.0
			_camera.v_offset = 0.0
			_saved = {}
		return
	var entering := _saved.is_empty()
	if entering:
		_saved = {"min": min_distance, "pitch": _pitch.rotation.x, "length": _arm.spring_length, "pivot": pivot_height}
	var head := focus == "head"
	min_distance = minf(min_distance, preview_head_distance if head else preview_distance)
	var portrait := _is_portrait()
	var distance := preview_head_distance if head else preview_distance
	if portrait:
		distance = preview_head_portrait_distance if head else preview_portrait_distance
	if entering:
		_arm.spring_length = distance
	else:
		create_tween().tween_property(_arm, "spring_length", distance, 0.3)
	pivot_height = preview_head_pivot if head else _saved["pivot"]
	_pitch.rotation.x = -deg_to_rad(preview_pitch)
	_camera.h_offset = 0.0 if portrait else preview_shift
	_camera.v_offset = -(preview_head_lift if head else preview_lift) if portrait else 0.0
	var model: Node3D = target.get_node_or_null(^"%Model") as Node3D if target != null else null
	if model != null:
		# The model faces `atan2(dir.x, dir.z)`; the camera hangs on the arm's +Z side (the Yaw node
		# is turned in the scene, so the rig's own axis says nothing about where the camera is).
		var toward := _arm.global_basis.z
		model.rotation.y = atan2(toward.x, toward.z) + deg_to_rad(yaw_deg)


func _is_portrait() -> bool:
	var size := get_viewport().get_visible_rect().size
	return size.y > size.x


## Wanted distance from the pivot; the arm may hold the camera closer when something is in the way.
func distance() -> float:
	return _arm.spring_length if _arm != null else _camera.position.z


func _zoom(delta: float) -> void:
	var d := clampf(distance() + delta, min_distance, max_distance)
	if _arm != null:
		_arm.spring_length = d
	else:
		_camera.position.z = d


## Stick or WASD input mapped onto the camera's horizontal plane (up = away from the
## camera); world axes when there is no camera.
static func relative_move(camera: Camera3D, input: Vector2) -> Vector3:
	var basis := camera.global_basis if camera else Basis.IDENTITY
	var forward := Vector3(-basis.z.x, 0.0, -basis.z.z).normalized()
	var right := Vector3(basis.x.x, 0.0, basis.x.z).normalized()
	return (right * input.x - forward * input.y).normalized()
