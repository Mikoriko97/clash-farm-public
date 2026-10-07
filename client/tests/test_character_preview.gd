extends SceneTree
## Headless test for CameraRig.preview (ADR-025): the creator screen brings the camera close, shifts
## the character out from behind the panel (up in a portrait window) and turns the character to face
## the camera (plus `yaw_deg`); switching it off puts the camera back exactly as it was.
## Run: godot --headless --path client --script res://tests/test_character_preview.gd

var _failures := 0
var _frames := 0
var _rig: CameraRig
var _player: Node3D


func _check(ok: bool, what: String) -> void:
	if not ok:
		_failures += 1
		push_error("FAIL: " + what)


func _init() -> void:
	_player = (load("res://scenes/player/Player.tscn") as PackedScene).instantiate() as Node3D
	root.add_child(_player)
	_rig = (load("res://scenes/camera/CameraRig.tscn") as PackedScene).instantiate() as CameraRig
	_rig.target = _player
	root.add_child(_rig)


func _process(_delta: float) -> bool:
	_frames += 1
	if _frames == 3:
		_run()
	return false


func _run() -> void:
	var arm := _rig.get_node("%Arm") as SpringArm3D
	var pitch := _rig.get_node("Yaw/Pitch") as Node3D
	var camera := _rig.get_node("%Camera3D") as Camera3D
	var model := _player.get_node("%Model") as Node3D
	var length := arm.spring_length
	var pitch_x := pitch.rotation.x
	var min_distance := _rig.min_distance
	var pivot := _rig.pivot_height

	_rig._on_bridge_message("camera_preview", {"on": true})
	var toward := atan2(arm.global_basis.z.x, arm.global_basis.z.z)
	_check(is_equal_approx(arm.spring_length, _rig.preview_distance), "the camera comes close")
	_check(_rig.min_distance <= _rig.preview_distance, "close is allowed for the zoom limit")
	_check(is_equal_approx(pitch.rotation.x, -deg_to_rad(_rig.preview_pitch)), "the view drops to eye level")
	_check(is_equal_approx(camera.h_offset, _rig.preview_shift) and is_equal_approx(camera.v_offset, 0.0), "the character steps out from behind the panel")
	_check(is_equal_approx(angle_difference(model.rotation.y, toward), 0.0), "the character faces the arm's direction")
	# Where the camera really hangs, once the arm has moved it, is the way the model must look.
	await physics_frame
	await physics_frame
	var to_camera := camera.global_position - _player.global_position
	to_camera.y = 0.0
	var dot := to_camera.normalized().dot(model.global_basis.z.normalized())
	_check(dot > 0.99, "the model looks at the real camera position: dot %.3f" % dot)

	_rig._on_bridge_message("camera_preview", {"on": true, "yaw_deg": 90.0})
	_check(is_equal_approx(absf(angle_difference(model.rotation.y, toward)), PI / 2.0), "yaw_deg turns the character")
	_check(is_equal_approx(arm.spring_length, _rig.preview_distance), "turning again does not stack the saved state")

	# Head focus (hair, face and hat tabs): closer, and the pivot rises to the face.
	_rig._on_bridge_message("camera_preview", {"on": true, "focus": "head"})
	_check(is_equal_approx(_rig.pivot_height, _rig.preview_head_pivot), "the pivot rises to the face")
	await create_timer(0.6).timeout
	_check(is_equal_approx(arm.spring_length, _rig.preview_head_distance), "the camera moves in on the head: %.2f" % arm.spring_length)
	_rig._on_bridge_message("camera_preview", {"on": true, "focus": "body"})
	await create_timer(0.6).timeout
	_check(is_equal_approx(arm.spring_length, _rig.preview_distance) and is_equal_approx(_rig.pivot_height, pivot), "back to the whole body")

	_rig._on_bridge_message("camera_preview", {"on": false})
	_check(is_equal_approx(arm.spring_length, length) and is_equal_approx(pitch.rotation.x, pitch_x) and is_equal_approx(_rig.min_distance, min_distance) and is_equal_approx(_rig.pivot_height, pivot), "off restores the normal camera")
	_check(is_equal_approx(camera.h_offset, 0.0) and is_equal_approx(camera.v_offset, 0.0), "off puts the character back in the middle")
	_rig._on_bridge_message("camera_preview", {"on": false})
	_check(is_equal_approx(arm.spring_length, length), "a second off changes nothing")

	print("RESULT: %s (%d failures)" % ["PASS" if _failures == 0 else "FAIL", _failures])
	quit(1 if _failures > 0 else 0)
