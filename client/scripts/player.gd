class_name Player
extends CharacterBody3D
## Third-person farm character. Moves from keyboard (WASD / arrows, Shift = run),
## from the React joystick (`external_input`, via Bridge `move_input`) or towards a
## point set by a click on the ground (`walk_to_point`). Animation is picked from
## speed; clips come from assets/character/character_anims.tres.
## Pure view logic: no economy, no network (rules/godot.md).

signal arrived

const ANIM_IDLE := &"idle"
## Standing clip; a scene may swap it (the cave holds the lantern up: `idle_torch`).
@export var idle_clip: StringName = ANIM_IDLE
const ANIM_WALK := &"walk_forward"
const ANIM_RUN := &"run_forward"
const BLEND_TIME := 0.15

@export var walk_speed: float = 2.2
@export var run_speed: float = 4.8
@export var turn_speed: float = 12.0
@export var gravity: float = 9.8
## Стрибок (пробіл або кнопка на телефоні, міст `jump`): початкова вертикальна швидкість.
@export var jump_speed: float = 5.0
## Distance at which a walk target counts as reached.
@export var arrive_distance: float = 0.2
## Set to false by Bridge when React opens a modal (set_mode ui).
@export var input_enabled: bool = true

## Joystick vector from the shell: x right, y forward is negative (Input.get_vector layout).
var external_input: Vector2 = Vector2.ZERO
var _stick_running := false
var _jump_queued := false
## Click / `walk_to` route (NavGrid waypoints and the stall counter).
var _route := WalkRoute.new()
## Seconds left of the one-shot action clip; movement decisions must not cut it.
var _action_left: float = 0.0
## Scripted glide (house door): input and physics off, walk clip plays, a tween carries the body through walls.
var _scripted: bool = false
## Clip forced while scripted (sleep sequence); empty = the walk clip of a glide.
var _scripted_clip: StringName = &""
## Seated on decor: physics off (the seat collider would push the capsule out);
## any movement input stands the character up at `_sit_exit`.
var _sitting: bool = false
var _sit_exit: Vector3 = Vector3.ZERO

@onready var _anim: AnimationPlayer = %AnimationPlayer
@onready var _hand: HandTools = get_node_or_null(^"%RightHand") as HandTools
@onready var _model: Node3D = %Model


func _ready() -> void:
	# Autoload may be absent in headless tests; React modal (set_mode ui) blocks input.
	var bridge: Node = get_node_or_null(^"/root/Bridge")
	CharacterLook.apply(_model, bridge.look if bridge != null else null)
	if bridge != null:
		bridge.mode_changed.connect(_on_mode_changed)
		# A modal opened during a zone change told only the old scene: start from the current mode.
		input_enabled = bridge.mode != "ui"
		# Метод екземпляра, не `.bind(_model)` на статичну функцію: зʼєднання з автолоадом зникає разом із гравцем (інакше після зміни зони мертвий вузол сипав помилки).
		if not bridge.message_received.is_connected(_on_bridge_message):
			bridge.message_received.connect(_on_bridge_message)


func _on_bridge_message(type: String, payload: Dictionary) -> void:
	CharacterLook.on_bridge(type, payload, _model)


func _on_mode_changed(mode: String) -> void:
	input_enabled = mode != "ui"
	if not input_enabled:
		cancel_walk()
		external_input = Vector2.ZERO


## Walk straight towards `point` and stop `stop_distance` short of it (0 = arrive_distance).
func walk_to_point(point: Vector3, stop_distance: float = 0.0) -> void:
	walk_path(PackedVector3Array([point]), stop_distance)


## Follow `points` in order; `stop_distance` applies to the last one only.
func walk_path(points: PackedVector3Array, stop_distance: float = 0.0) -> void:
	if points.is_empty():
		return
	if _sitting:
		stand_up()
	_route.start(points, maxf(stop_distance, arrive_distance))


## Turns the model towards `point` right away (used before an action clip); seated it keeps its back to the backrest.
func face_point(point: Vector3) -> void:
	var to := point - global_position
	if to.length() > 0.05 and not _sitting:
		_model.rotation.y = atan2(to.x, to.z)


func cancel_walk() -> void:
	_route.cancel()


## Carries the character to `point` in `duration` s while the walk clip plays (doorways); resolves at the end, then input resumes.
func glide_to(point: Vector3, duration: float, keep_input: bool = false) -> void:
	cancel_walk()
	if not keep_input:
		external_input = Vector2.ZERO
	_sitting = false
	_scripted_clip = &""
	_scripted = true
	velocity = Vector3.ZERO
	face_point(point)
	var tween := create_tween()
	tween.tween_property(self, "global_position", point, maxf(duration, 0.05))
	await tween.finished
	_scripted = false


func is_scripted() -> bool:
	return _scripted


## Plays `clip` with input and physics off until it ends; a looping clip (or `hold` > 0) plays `hold` s. `backwards` reverses it.
func perform(clip: StringName, speed: float = 1.0, backwards: bool = false, hold: float = 0.0) -> void:
	if not _anim.has_animation(clip):
		push_warning("Player: unknown clip %s" % clip)
		return
	cancel_walk()
	external_input = Vector2.ZERO
	velocity = Vector3.ZERO
	_scripted = true
	_scripted_clip = clip
	_action_left = 0.0
	var animation := _anim.get_animation(clip)
	if backwards:
		_anim.play_backwards(clip, BLEND_TIME)
		_anim.speed_scale = speed
	else:
		_anim.play(clip, BLEND_TIME, speed)
	var seconds := hold if hold > 0.0 else animation.length / maxf(speed, 0.01)
	if hold <= 0.0 and animation.loop_mode == Animation.LOOP_NONE:
		await _anim.animation_finished
	else:
		await get_tree().create_timer(seconds).timeout
	_anim.speed_scale = 1.0
	_scripted_clip = &""
	_scripted = false


## Glides onto a seat, faces `forward`, holds the sitting pose (physics off); movement input or a click stands up at `exit_point`.
func sit_at(seat_origin: Vector3, forward: Vector3, exit_point: Vector3) -> void:
	await glide_to(seat_origin, 0.35)
	face_point(seat_origin + forward)
	_sit_exit = exit_point
	_sitting = true
	_scripted = true
	_scripted_clip = &"sitting_idle"
	_action_left = 0.0
	_anim.play(&"sitting_idle", BLEND_TIME)


func is_sitting() -> bool:
	return _sitting


func stand_up() -> void:
	if not _sitting:
		return
	_sitting = false
	_scripted_clip = &""
	_scripted = false
	# Джойстик шле лише зміни: не обнуляти його, інакше з утримуваним стіком гравець встане й стоятиме (нового значення не прийде).
	await glide_to(_sit_exit, 0.3, true)


func _raw_input() -> Vector2:
	var input := Input.get_vector(&"move_left", &"move_right", &"move_forward", &"move_back")
	return input if input != Vector2.ZERO else external_input


func is_walking_to_target() -> bool:
	return _route.active


func _physics_process(delta: float) -> void:
	if _sitting and (_raw_input() != Vector2.ZERO):
		stand_up()
	if _scripted:
		if _scripted_clip == &"":
			_update_animation(true, false)
		return
	var dir := _move_direction() if input_enabled else Vector3.ZERO
	# Біг від джойстика (`StickRun`); клавіатура має пріоритет, там біг — Shift.
	_stick_running = Input.get_vector(&"move_left", &"move_right", &"move_forward", &"move_back") == Vector2.ZERO and StickRun.runs(external_input, _stick_running)
	var running := input_enabled and (Input.is_action_pressed(&"sprint") or _stick_running)
	var speed := run_speed if running else walk_speed
	velocity.x = dir.x * speed
	velocity.z = dir.z * speed
	var jump := input_enabled and not _sitting and (_jump_queued or Input.is_action_just_pressed(&"jump"))
	_jump_queued = false
	if is_on_floor() and jump:
		velocity.y = jump_speed
		if _anim.has_animation(&"jump"):
			_anim.play(&"jump", BLEND_TIME)
	else:
		velocity.y = 0.0 if is_on_floor() else velocity.y - gravity * delta
	if dir != Vector3.ZERO:
		_face(dir, delta)
	move_and_slide()
	_check_stall(dir)
	_action_left = maxf(_action_left - delta, 0.0)
	_update_animation(dir != Vector3.ZERO, running)


## Blocked by a collider while walking to a target: count it as arrived instead of pushing forever.
func _check_stall(dir: Vector3) -> void:
	if _route.stalled(dir, get_real_velocity().length()):
		arrived.emit()


## Direct input wins and cancels any walk target; otherwise head for the target.
func _move_direction() -> Vector3:
	var input := _raw_input()
	if input != Vector2.ZERO:
		_route.active = false
		return CameraRig.relative_move(get_viewport().get_camera_3d(), input)
	if _route.active:
		return _target_direction()
	return Vector3.ZERO


func _target_direction() -> Vector3:
	var dir := _route.direction(global_position, arrive_distance)
	if not _route.active:
		arrived.emit()
	return dir


func _face(dir: Vector3, delta: float) -> void:
	var target := atan2(dir.x, dir.z)
	_model.rotation.y = lerp_angle(_model.rotation.y, target, minf(turn_speed * delta, 1.0))


## Стрибок з мосту чи кнопки на телефоні: виконується на наступному фізичному такті, якщо на землі.
func request_jump() -> void:
	_jump_queued = true


## Locomotion clip from the movement state. An action clip keeps playing while the
## character stands still; walking away interrupts it.
func _update_animation(moving: bool, running: bool) -> void:
	if not is_on_floor() and _anim.current_animation == &"jump":
		return
	if moving:
		_action_left = 0.0
	elif _action_left > 0.0:
		return
	if _hand != null and _hand.shown() != "":
		_hand.hide_all()
	var next := idle_clip if _anim.has_animation(idle_clip) else ANIM_IDLE
	if moving:
		next = ANIM_RUN if running else ANIM_WALK
	if _anim.current_animation != next or not _anim.is_playing():
		_anim.play(next, BLEND_TIME)


## Action clip (harvest, plant, water, till, mine) played once in full, then back
## to the locomotion clip. Looping clips (till) play a single cycle.
func play_action(clip: StringName, tool: String = "") -> void:
	if not _anim.has_animation(clip):
		push_warning("Player: unknown clip %s" % clip)
		return
	_anim.play(clip, BLEND_TIME)
	_action_left = _anim.get_animation(clip).length
	if _hand != null:
		_hand.show_tool(tool)
	if _sitting:
		# Seated, physics is off and nothing picks the next clip: back to the seat pose once this one is over
		# (it used to freeze on the clip's last frame). A timer, not `animation_finished`: looping clips play one cycle.
		get_tree().create_timer(_action_left).timeout.connect(_back_to_seat.bind(clip))


func _back_to_seat(clip: StringName) -> void:
	if _sitting and _anim.assigned_animation == clip:
		_action_left = 0.0
		_anim.play(&"sitting_idle", BLEND_TIME)
		if _hand != null:
			_hand.hide_all()


func is_acting() -> bool:
	return _action_left > 0.0
