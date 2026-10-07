extends SceneTree
## Headless flow test with the real Player scene: `house { inside: true }` walks
## the character to the threshold, glides it through the door and hides it;
## `house { inside: false }` brings it back out to the threshold. Runs across
## physics frames (no await in the runner) with a frame budget.
## Run: godot --headless --path client --script res://tests/test_house_flow.gd

const BUDGET_FRAMES := 900

var _failures := 0
var _frames := 0
var _stage := 0
var _player: Player
var _house: HouseNode
var _entered := false
var _slept := false
var _audio: Node
var _end_frame := 0


func _check(ok: bool, what: String) -> void:
	if not ok:
		_failures += 1
		push_error("FAIL: " + what)


func _init() -> void:
	_audio = load("res://tests/fake_audio.gd").new() as Node
	_audio.name = "Audio"
	root.add_child(_audio)
	var world := Node3D.new()
	root.add_child(world)
	var floor_body := StaticBody3D.new()
	var floor_shape := CollisionShape3D.new()
	var box := BoxShape3D.new()
	box.size = Vector3(120.0, 1.0, 120.0)
	floor_shape.shape = box
	floor_body.add_child(floor_shape)
	floor_body.position = Vector3(0.0, -0.5, 0.0)
	world.add_child(floor_body)

	_player = (load("res://scenes/player/Player.tscn") as PackedScene).instantiate()
	_player.position = Vector3(-8.0, 0.0, -2.0)
	world.add_child(_player)
	var interaction := Interaction.new()
	interaction.name = "Interaction"
	interaction.player = _player
	world.add_child(interaction)
	_house = HouseNode.new()
	_house.position = Vector3(-13.0, 0.0, -7.0)
	_house.player = _player
	_house.interaction = interaction
	_house.entered.connect(func() -> void: _entered = true)
	world.add_child(_house)


func _process(_delta: float) -> bool:
	_frames += 1
	if _frames == 2:
		_house.set_inside(true)
		_stage = 1
	elif _stage == 1 and _entered:
		_check(not _player.visible, "entered: character hidden")
		var at := _player.global_position
		_check(_flat_distance(at, _house.inside_position()) < 0.1, "entered: parked past the door, at %s" % at)
		_house.set_inside(false)
		_stage = 2
	elif _stage == 2 and not _player.is_scripted() and _frames > 5:
		_check(_player.visible, "left: character shown")
		var at := _player.global_position
		_check(_flat_distance(at, _house.threshold_position()) < 0.1, "left: standing on the threshold, at %s" % at)
		# Sleep sequence at test speed: lie down, sleep, get up, then free again.
		_house.sleep_secs = 0.2
		_house.slept.connect(func() -> void: _slept = true)
		_house.request_sleep()
		_stage = 3
	elif _stage == 3 and _slept and not _player.is_scripted():
		_check(_player.visible, "after sleep: character shown")
		_check(not _player.is_acting(), "after sleep: no clip lock left")
		_stage = 4
		_end_frame = _frames + 40  # the door shuts a quarter of a second after the last step out
	elif _stage == 4 and _frames >= _end_frame:
		var played: Array = _audio.get("played")
		_check(played.count("door_open") == 2 and played.count("door_close") == 2, "the door opens and shuts on the way in and on the way out: %s" % [played])
		return _finish()
	if _frames > BUDGET_FRAMES:
		_check(false, "flow did not finish within %d frames (stage %d)" % [BUDGET_FRAMES, _stage])
		return _finish()
	return false


func _flat_distance(a: Vector3, b: Vector3) -> float:
	return Vector2(a.x, a.z).distance_to(Vector2(b.x, b.z))


func _finish() -> bool:
	print("RESULT: %s (%d failures)" % ["PASS" if _failures == 0 else "FAIL", _failures])
	quit(1 if _failures > 0 else 0)
	return true
