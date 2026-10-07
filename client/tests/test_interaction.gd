extends SceneTree
## Headless test: Interaction picks the nearest interactable within radius,
## highlights it once and clears it when the player walks away; Player walks
## towards a point and reports arrival.
## Run: godot --headless --path client --script res://tests/test_interaction.gd


class FakeTarget:
	extends Node3D
	var kind: String = "plot"
	var idx: int = 0
	var highlighted: bool = false

	func interaction_kind() -> String:
		return kind

	func interaction_index() -> int:
		return idx

	func set_highlighted(on: bool) -> void:
		highlighted = on


var _failures := 0
var _events: Array = []


var _done := false


## Nodes only join the tree once it runs: do the work on the first frame.
func _process(_delta: float) -> bool:
	if _done:
		return true
	_done = true
	_test_nearest_and_highlight()
	_test_player_walks_to_point()
	print("RESULT: %s (%d failures)" % ["PASS" if _failures == 0 else "FAIL", _failures])
	quit(1 if _failures > 0 else 0)
	return true


func _test_nearest_and_highlight() -> void:
	var world := Node3D.new()
	root.add_child(world)
	var player := Node3D.new()
	world.add_child(player)
	var plots: Array[FakeTarget] = []
	for i: int in 3:
		var t := FakeTarget.new()
		t.idx = i
		t.position = Vector3(i * 3.0, 0.0, 0.0)
		t.add_to_group(Interaction.GROUP)
		world.add_child(t)
		plots.append(t)
	var slot := FakeTarget.new()
	slot.kind = "decor_slot"
	slot.idx = 7
	slot.position = Vector3(0.0, 0.0, 4.0)
	slot.add_to_group(Interaction.GROUP)
	world.add_child(slot)

	var interaction := Interaction.new()
	interaction.player = player
	interaction.radius = 1.6
	interaction.near_changed.connect(func(kind: String, idx: int, _d: float) -> void: _events.append([kind, idx]))
	world.add_child(interaction)

	player.position = Vector3(3.4, 0.0, 0.0)
	interaction.scan()
	interaction.scan()  # no change → no second event
	_check(_events == [["plot", 1]], "nearest plot #1 reported once: %s" % [_events])
	_check(plots[1].highlighted and not plots[0].highlighted, "only plot #1 highlighted")

	player.position = Vector3(0.2, 0.0, 3.6)
	interaction.scan()
	_check(_events.back() == ["decor_slot", 7], "decor slot wins when closer")
	_check(not plots[1].highlighted and slot.highlighted, "highlight moved to the slot")

	player.position = Vector3(20.0, 0.0, 20.0)
	interaction.scan()
	_check(_events.back() == ["none", -1], "walking away reports none")
	_check(not slot.highlighted, "highlight cleared")

	# A city building at the player's feet rings the door bell once (the nearest target changed to a building), nothing else does.
	# (the game's Audio autoload, if this run has it, steps aside while the recorder stands in)
	var real_audio := root.get_node_or_null(^"Audio")
	if real_audio != null:
		real_audio.name = "AudioReal"
	var audio := load("res://tests/fake_audio.gd").new() as Node
	audio.name = "Audio"
	root.add_child(audio)
	_check(audio.get("played") == [], "no door bell for plots and slots")
	var building := FakeTarget.new()
	building.kind = "building"
	building.position = Vector3(20.5, 0.0, 20.0)
	building.add_to_group(Interaction.GROUP)
	world.add_child(building)
	interaction.scan()
	interaction.scan()
	_check(audio.get("played") == ["door_bell"], "reaching a building door rings the bell once: %s" % [audio.get("played")])
	audio.free()
	if real_audio != null:
		real_audio.name = "Audio"
	var far := Interaction.nearest([plots[0]], Vector3(5.0, 0.0, 0.0), 1.0)
	_check(far.is_empty(), "nothing within radius → empty")
	var edge := Interaction.approach_point(Vector3(10.0, 0.0, 0.0), Vector3(4.0, 3.0, 0.0), 1.8)
	_check(edge.is_equal_approx(Vector3(8.2, 0.0, 0.0)), "approach point sits on the player's side of the target: %s" % edge)
	var fallback := Interaction.approach_point(Vector3.ZERO, Vector3.ZERO, 1.0)
	_check(fallback.is_equal_approx(Vector3(0.0, 0.0, 1.0)), "coincident positions still give a point")
	world.queue_free()


func _test_player_walks_to_point() -> void:
	var player: Player = (load("res://scenes/player/Player.tscn") as PackedScene).instantiate()
	root.add_child(player)
	var arrived := [false]
	player.arrived.connect(func() -> void: arrived[0] = true)
	player.walk_to_point(Vector3(4.0, 0.0, 0.0))
	var dir := player._move_direction()
	_check(dir.is_equal_approx(Vector3(1.0, 0.0, 0.0)), "heads towards the target: %s" % dir)
	player.global_position = Vector3(3.9, 0.0, 0.0)
	dir = player._move_direction()
	_check(dir == Vector3.ZERO and arrived[0], "stops and reports arrival inside arrive_distance")
	_check(not player.is_walking_to_target(), "target cleared after arrival")

	player.walk_to_point(Vector3(-4.0, 0.0, 0.0), 1.0)
	player.external_input = Vector2(1.0, 0.0)
	dir = player._move_direction()
	_check(not player.is_walking_to_target() and dir.x > 0.0, "joystick input cancels the walk target")
	player.queue_free()


func _check(ok: bool, label: String) -> void:
	if not ok:
		_failures += 1
	print("%s %s" % ["ok  " if ok else "FAIL", label])
