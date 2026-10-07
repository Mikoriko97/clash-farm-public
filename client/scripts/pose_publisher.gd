class_name PosePublisher
extends Node
## Tells React where the character is (`player_pose`, docs/BRIDGE_PROTOCOL.md) a
## few times a second so the shell can share it over the presence socket.
## Integers in decimetres and degrees, only when something changed, plus a
## heartbeat every second so a still player stays visible to others.

@export var player: Node3D
@export var rate_hz: float = 8.0
@export var heartbeat_secs: float = 1.0

var _accum: float = 0.0
var _since_send: float = 0.0
var _last: Dictionary = {}


func _process(delta: float) -> void:
	# Hidden = indoors (HouseNode): neighbours must not see a figure standing inside the walls; the pose resumes on the way out.
	if player == null or not player.visible:
		return
	_accum += delta
	_since_send += delta
	if _accum < 1.0 / maxf(rate_hz, 1.0):
		return
	_accum = 0.0
	var pose := current_pose()
	if pose == _last and _since_send < heartbeat_secs:
		return
	_last = pose
	_since_send = 0.0
	var bridge: Node = get_node_or_null(^"/root/Bridge")
	if bridge != null:
		bridge.send("player_pose", pose)


## `{ x_dm, z_dm, yaw_deg, anim }` of the character right now.
func current_pose() -> Dictionary:
	var p := player.global_position
	var yaw := 0.0
	var model: Node3D = player.get_node_or_null(^"%Model") as Node3D
	if model != null:
		yaw = rad_to_deg(model.rotation.y)
	var anim := "idle"
	var anim_player: AnimationPlayer = player.get_node_or_null(^"%AnimationPlayer") as AnimationPlayer
	if anim_player != null and anim_player.current_animation != "":
		anim = anim_player.current_animation
	return {
		"x_dm": roundi(p.x * 10.0),
		"z_dm": roundi(p.z * 10.0),
		"yaw_deg": posmod(roundi(yaw), 360),
		"anim": anim,
	}
