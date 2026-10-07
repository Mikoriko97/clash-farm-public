class_name AnimalWander
extends Node3D
## Ambient animal: wanders around its home spot, pauses, turns to face where it
## goes, bobs while walking. No physics, no gameplay — attach to a prop wrapper
## (scenes/props/hen.tscn, cow.tscn) and tune from the editor.

@export var radius: float = 4.0
@export var speed: float = 0.7
@export var pause_min: float = 1.0
@export var pause_max: float = 4.0
@export var bob_height: float = 0.03
@export var turn_speed: float = 6.0
@export var seed: int = 0

var _home: Vector3
var _target: Vector3
var _pause: float = 0.0
var _rng := RandomNumberGenerator.new()
var _walked: float = 0.0


func _ready() -> void:
	# Moved every render frame, not on the physics tick: interpolation would only add lag.
	physics_interpolation_mode = Node.PHYSICS_INTERPOLATION_MODE_OFF
	_home = global_position
	_rng.seed = seed if seed != 0 else hash(name)
	_pause = _rng.randf_range(pause_min, pause_max)
	_target = _home


func _process(delta: float) -> void:
	if _pause > 0.0:
		_pause -= delta
		if _pause <= 0.0:
			_pick_target()
		return
	var to := _target - global_position
	to.y = 0.0
	if to.length() < 0.1:
		_pause = _rng.randf_range(pause_min, pause_max)
		position.y = _home.y
		return
	var dir := to.normalized()
	global_position += dir * speed * delta
	_walked += speed * delta
	position.y = _home.y + absf(sin(_walked * 9.0)) * bob_height
	var yaw := atan2(dir.x, dir.z)
	rotation.y = lerp_angle(rotation.y, yaw, minf(turn_speed * delta, 1.0))


func _pick_target() -> void:
	var angle := _rng.randf_range(0.0, TAU)
	var dist := _rng.randf_range(radius * 0.3, radius)
	_target = _home + Vector3(cos(angle) * dist, 0.0, sin(angle) * dist)
