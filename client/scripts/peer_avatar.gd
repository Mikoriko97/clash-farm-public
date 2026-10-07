class_name PeerAvatar
extends Node3D
## Another player as the server describes them (`peers_state`): the same body
## and clips as the local character, no collider, a nick over the head. Poses
## arrive ~5 times a second; the avatar glides towards the latest one so it
## never pops. Pure view.

## Seconds to reach a new pose (a little above the server tick).
@export var smoothing: float = 0.25

var peer_id: String = ""
## Мапа висот зони (`height_at(x, z)`, MainlandTerrain); на острові її нема, земля на y = 0.
var ground: Node = null
var _target_pos: Vector3 = Vector3.ZERO
var _target_yaw: float = 0.0
var _clip: StringName = &"idle"
var _last_seen_ms: int = 0
## Останній застосований вигляд (рядком), щоб не перефарбовувати на кожен кадр пози.
var _look_seen: String = ""

@onready var _model: Node3D = %Model
@onready var _anim: AnimationPlayer = %AnimationPlayer
@onready var _label: Label3D = %Nick


## `{ id, nick, look?, x_dm, z_dm, yaw_deg, anim }` (validated by PeerAvatars); `look` — кольори як у `player_look`.
func apply(entry: Dictionary, snap: bool = false) -> void:
	var x := float(entry["x_dm"]) / 10.0
	var z := float(entry["z_dm"]) / 10.0
	var y := float(ground.call("height_at", x, z)) if ground != null and ground.has_method("height_at") else 0.0
	_target_pos = Vector3(x, y, z)
	_target_yaw = deg_to_rad(float(entry["yaw_deg"]))
	_last_seen_ms = Time.get_ticks_msec()
	if _label != null:
		_label.text = str(entry.get("nick", ""))
	var look: Variant = entry.get("look")
	if _model != null and str(look) != _look_seen:
		_look_seen = str(look)
		CharacterLook.apply(_model, look)
	var clip := StringName(str(entry.get("anim", "idle")))
	if _anim != null and _anim.has_animation(clip) and clip != _clip:
		_clip = clip
		_anim.play(clip, 0.15)
	if snap:
		global_position = _target_pos
		if _model != null:
			_model.rotation.y = _target_yaw


func _process(delta: float) -> void:
	var t := clampf(delta / maxf(smoothing, 0.01), 0.0, 1.0)
	global_position = global_position.lerp(_target_pos, t)
	if _model != null:
		_model.rotation.y = lerp_angle(_model.rotation.y, _target_yaw, t)


func seen_ms_ago() -> int:
	return Time.get_ticks_msec() - _last_seen_ms
