extends SceneTree
## Headless regressions of the client fixes 2026-10-06: Player and Placer start from the bridge mode (a mode changed during a
## zone swap reached only the old scene) and the held joystick is replayed after a swap (Bridge); a freed highlighted node still
## ends in `player_near none` (Interaction); an explicit empty `peers_state` drops every neighbour and stale ones go without a
## message (PeerAvatars); a React panel resets a half-made drag (IslandInput); no `player_pose` while the character is indoors
## (PosePublisher); a clip played seated returns to the seat pose and does not turn the body (Player); primitive collider boxes
## match their debug meshes (NavGrid).
## Run: godot --headless --path client --script res://tests/test_client_fixes.gd

const FakeBridge := preload("res://tests/fake_bridge.gd")
const BUDGET_FRAMES := 2000

var _failures := 0
var _frames := 0
var _stage := 0
var _real: Node
var _fake: Node
var _world: Node3D
var _replayed: Array = []
var _player: Player
var _peers: PeerAvatars
var _poses: PosePublisher
var _wait_frames := 0
var _wait_ms := 0


func _check(ok: bool, what: String) -> void:
	if not ok:
		_failures += 1
		push_error("FAIL: " + what)


func _init() -> void:
	_world = Node3D.new()
	root.add_child(_world)


func _process(_delta: float) -> bool:
	_frames += 1
	if _frames == 2:
		_real = root.get_node_or_null(^"Bridge")
		_check(_real != null, "the Bridge autoload exists")
		if _real == null:
			return _finish()
		_check_mode_on_ready()
		_start_move_replay()
		_stage = 1
	elif _stage == 1 and _frames >= _wait_frames:
		_check_move_replay()
		_swap_in_fake_bridge()
		_check_nav_aabbs()
		_check_interaction_freed_node()
		_check_island_input_reset()
		_start_timed()
		_stage = 2
	elif _stage == 2 and Time.get_ticks_msec() >= _wait_ms:
		_check_timed()
		_player.visible = true
		_wait_ms = Time.get_ticks_msec() + 300
		_stage = 3
	elif _stage == 3 and Time.get_ticks_msec() >= _wait_ms:
		_check(not _fake.call("sent_of", "player_pose").is_empty(), "the pose is published again once the character is out")
		return _finish()
	if _frames > BUDGET_FRAMES:
		_check(false, "did not finish within %d frames (stage %d)" % [BUDGET_FRAMES, _stage])
		return _finish()
	return false


## Item 3: new scenes read the current mode in `_ready` (the swap emitted `mode_changed` to the old scene only).
func _check_mode_on_ready() -> void:
	_real.set("mode", "ui")
	var player := (load("res://scenes/player/Player.tscn") as PackedScene).instantiate() as Player
	_world.add_child(player)
	_check(not player.input_enabled, "a player spawned while React holds the screen starts with input off")
	player.free()
	_real.set("mode", "build")
	var placer := (load("res://scenes/island/Placer.tscn") as PackedScene).instantiate() as Placer
	_world.add_child(placer)
	_check(placer.active, "a placer spawned in build mode starts active")
	placer.free()
	_real.set("mode", "walk")


## Item 3: the last `move_input` reaches the new scene after a swap, even if the shell sent nothing meanwhile.
func _start_move_replay() -> void:
	_real.call("handle", {"v": 1, "type": "move_input", "payload": {"x": 0.5, "y": -1.0}})
	_real.message_received.connect(_record)
	_real.set("_swaps", 1)
	_real.call("_announce_zone", {})
	_wait_frames = _frames + 5


func _record(type: String, payload: Dictionary) -> void:
	_replayed.append([type, payload])


func _check_move_replay() -> void:
	_real.message_received.disconnect(_record)
	var moves := _replayed.filter(func(m: Array) -> bool: return m[0] == "move_input")
	_check(moves.size() == 1 and is_equal_approx(float((moves[0][1] as Dictionary)["x"]), 0.5), "the held joystick is replayed after the swap: %s" % [_replayed])


func _swap_in_fake_bridge() -> void:
	_real.name = "RealBridge"
	_fake = FakeBridge.new()
	_fake.name = "Bridge"
	root.add_child(_fake)


## Item 5: primitive extents from parameters are the same boxes the debug meshes gave.
func _check_nav_aabbs() -> void:
	var box := BoxShape3D.new()
	box.size = Vector3(1.2, 0.7, 2.4)
	var sphere := SphereShape3D.new()
	sphere.radius = 0.45
	var cylinder := CylinderShape3D.new()
	cylinder.radius = 0.3
	cylinder.height = 1.7
	var capsule := CapsuleShape3D.new()
	capsule.radius = 0.25
	capsule.height = 1.4
	for shape: Shape3D in [box, sphere, cylinder, capsule]:
		var mine := NavGrid.local_aabb(shape)
		var debug := shape.get_debug_mesh().get_aabb()
		_check(mine.position.distance_to(debug.position) < 0.01 and mine.size.distance_to(debug.size) < 0.02, "%s extents %s match the debug mesh %s" % [shape.get_class(), mine, debug])


## Item 1: the highlighted node is freed (bush cleared, decor removed): the next pass reports `none`, no engine error.
func _check_interaction_freed_node() -> void:
	var source := GDScript.new()
	source.source_code = "extends Node3D\nvar lit := false\nfunc interaction_kind() -> String:\n\treturn \"decor\"\nfunc interaction_index() -> int:\n\treturn 3\nfunc set_highlighted(on: bool) -> void:\n\tlit = on\n"
	source.reload()
	var target := Node3D.new()
	target.set_script(source)
	target.add_to_group(Interaction.GROUP)
	_world.add_child(target)
	var stand := Node3D.new()
	_world.add_child(stand)
	var interaction := Interaction.new()
	interaction.player = stand
	_world.add_child(interaction)
	interaction.scan()
	_check(interaction.current_kind == "decor" and bool(target.get("lit")), "the decor is near and lit")
	target.free()
	interaction.scan()
	var near: Array = _fake.call("sent_of", "player_near")
	_check(near.size() == 2 and str((near[1] as Dictionary)["kind"]) == Interaction.KIND_NONE, "a freed target ends in player_near none: %s" % [near])
	interaction.scan()
	_check(_fake.call("sent_of", "player_near").size() == 2, "nothing near: no repeated report")
	interaction.free()
	stand.free()


## Item 10: a press that began before a React panel must not keep orbiting the camera after it closes.
func _check_island_input_reset() -> void:
	var scene := Node3D.new()
	scene.set_script(load("res://scripts/island_input.gd"))
	var player := (load("res://scenes/player/Player.tscn") as PackedScene).instantiate()
	player.name = "Player"
	scene.add_child(player)
	var rig := (load("res://scenes/camera/CameraRig.tscn") as PackedScene).instantiate()
	rig.name = "CameraRig"
	scene.add_child(rig)
	var interaction := Interaction.new()
	interaction.name = "Interaction"
	scene.add_child(interaction)
	_world.add_child(scene)
	scene.set("_pressed", true)
	scene.set("_dragging", true)
	_fake.call("set_mode", "ui")
	_check(not bool(scene.get("_pressed")) and not bool(scene.get("_dragging")), "a panel resets the half-made press and drag")
	_fake.call("set_mode", "walk")
	scene.free()


## Items 7, 8, 11 need real time: a clip seated, peers going stale, a hidden character.
func _start_timed() -> void:
	_peers = PeerAvatars.new()
	_peers.avatar_scene = load("res://scenes/player/PeerAvatar.tscn")
	_world.add_child(_peers)
	_peers.apply([{"id": "aa", "nick": "A", "x_dm": 10, "z_dm": 0, "yaw_deg": 0, "anim": "idle"}])
	_check(_peers.count() == 1, "one neighbour spawned")
	_peers.apply([])
	_check(_peers.count() == 0, "an explicit empty peers_state drops every neighbour at once")
	_peers.grace_ms = 100
	_peers.purge_secs = 0.0
	_peers.apply([{"id": "bb", "nick": "B", "x_dm": 20, "z_dm": 0, "yaw_deg": 0, "anim": "idle"}])
	_check(_peers.count() == 1, "a second neighbour spawned")
	_player = (load("res://scenes/player/Player.tscn") as PackedScene).instantiate() as Player
	_world.add_child(_player)
	_player.visible = false
	_poses = PosePublisher.new()
	_poses.player = _player
	_poses.rate_hz = 60.0
	_world.add_child(_poses)
	var anim := _player.get_node(^"%AnimationPlayer") as AnimationPlayer
	var model := _player.get_node(^"%Model") as Node3D
	_player.set("_sitting", true)
	_player.set("_scripted", true)
	_player.set("_scripted_clip", &"sitting_idle")  # what `sit_at` leaves behind once seated
	anim.play(&"sitting_idle")
	var yaw := model.rotation.y
	_player.face_point(_player.global_position + Vector3(5.0, 0.0, -3.0))
	_check(is_equal_approx(model.rotation.y, yaw), "seated, the body keeps its back to the backrest")
	_player.play_action(&"harvest")
	_check(anim.current_animation == "harvest", "the clip plays seated")
	_wait_ms = Time.get_ticks_msec() + int(anim.get_animation(&"harvest").length * 1000.0) + 400


func _check_timed() -> void:
	_check(_peers.count() == 0, "a neighbour unseen past the grace goes without a new message")
	_check(_fake.call("sent_of", "player_pose").is_empty(), "no player_pose while the character is hidden indoors")
	var anim := _player.get_node(^"%AnimationPlayer") as AnimationPlayer
	_check(anim.assigned_animation == "sitting_idle", "after the clip the character is back in the seat pose, not frozen: %s" % anim.assigned_animation)
	_check(not _player.is_acting(), "no clip lock left after the seated action")


func _finish() -> bool:
	print("RESULT: %s (%d failures)" % ["PASS" if _failures == 0 else "FAIL", _failures])
	quit(1 if _failures > 0 else 0)
	return true
