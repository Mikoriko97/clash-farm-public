extends SceneTree
## Headless flow test of a cancelled house entry (item 2, 2026-10-06): every early exit of the walk in answers
## `house_entered { ok: false }` exactly once, so React stops waiting (its 20 s fallback used to open the panel anyway).
## A: `house { inside: false }` while the character still walks to the threshold. B: the same during the glide through the
## door — the character steps back out to the threshold, visible and free to walk. Real Player scene, fake Bridge.
## Run: godot --headless --path client --script res://tests/test_house_cancel.gd

const FakeBridge := preload("res://tests/fake_bridge.gd")
const BUDGET_FRAMES := 900

var _failures := 0
var _frames := 0
var _stage := 0
var _player: Player
var _house: HouseNode
var _fake: Node
var _mark := 0
var _world: Node3D


func _check(ok: bool, what: String) -> void:
	if not ok:
		_failures += 1
		push_error("FAIL: " + what)


func _init() -> void:
	_world = Node3D.new()
	root.add_child(_world)
	var floor_body := StaticBody3D.new()
	var floor_shape := CollisionShape3D.new()
	var box := BoxShape3D.new()
	box.size = Vector3(120.0, 1.0, 120.0)
	floor_shape.shape = box
	floor_body.add_child(floor_shape)
	floor_body.position = Vector3(0.0, -0.5, 0.0)
	_world.add_child(floor_body)


## The real Bridge autoload steps aside for the recording double before any scene node looks it up.
func _setup() -> void:
	var real := root.get_node_or_null(^"Bridge")
	if real != null:
		real.name = "RealBridge"
	_fake = FakeBridge.new()
	_fake.name = "Bridge"
	root.add_child(_fake)
	var world := _world
	_player = (load("res://scenes/player/Player.tscn") as PackedScene).instantiate()
	_player.position = Vector3(-8.0, 0.0, -2.0)
	world.add_child(_player)
	var interaction := Interaction.new()
	interaction.player = _player
	world.add_child(interaction)
	_house = HouseNode.new()
	_house.position = Vector3(-13.0, 0.0, -7.0)
	_house.player = _player
	_house.interaction = interaction
	world.add_child(_house)


func _answers() -> Array:
	return _fake.call("sent_of", "house_entered")


func _process(_delta: float) -> bool:
	_frames += 1
	if _frames == 2:
		_setup()
	elif _frames == 4:
		_house.set_inside(true)
		_stage = 1
		_mark = _frames + 20
	elif _stage == 1 and _frames >= _mark:
		_check(_player.is_walking_to_target(), "A: still walking to the threshold")
		_house.set_inside(false)
		_check(_answers() == [{"ok": false}], "A: cancelling the walk in answers ok:false at once: %s" % [_answers()])
		_stage = 2
		_mark = _frames + 400
	elif _stage == 2 and (not _player.is_walking_to_target() or _frames >= _mark):
		_check(_player.visible and not _player.is_scripted(), "A: the character stays outside, free to walk")
		_check(_answers().size() == 1, "A: exactly one answer: %s" % [_answers()])
		# B: from the threshold the glide starts at once; cancel it halfway through the door.
		_player.cancel_walk()
		_player.global_position = _house.threshold_position()
		_fake.set("sent", [])
		_house.set_inside(true)
		_check(_player.is_scripted(), "B: the glide through the door started")
		_stage = 3
		_mark = _frames + 10
	elif _stage == 3 and _frames >= _mark:
		_house.set_inside(false)
		_check(_answers() == [{"ok": false}], "B: cancelling mid-door answers ok:false: %s" % [_answers()])
		_stage = 4
	elif _stage == 4 and not _player.is_scripted():
		var at := _player.global_position
		var threshold := _house.threshold_position()
		_check(Vector2(at.x, at.z).distance_to(Vector2(threshold.x, threshold.z)) < 0.1, "B: back on the threshold, at %s" % at)
		_check(_player.visible, "B: the character is visible")
		_check(_answers().size() == 1, "B: no late ok:true after the cancel: %s" % [_answers()])
		_check(not _house.inside, "B: the house does not count the character as inside")
		return _finish()
	if _frames > BUDGET_FRAMES:
		_check(false, "flow did not finish within %d frames (stage %d)" % [BUDGET_FRAMES, _stage])
		return _finish()
	return false


func _finish() -> bool:
	print("RESULT: %s (%d failures)" % ["PASS" if _failures == 0 else "FAIL", _failures])
	quit(1 if _failures > 0 else 0)
	return true
