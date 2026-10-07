extends SceneTree
## Headless test: HouseNode is an interactable of kind "house" whose walk target
## is the threshold before the door; `house { inside }` parks the character just
## inside the doorway (hidden) and brings it back out to the threshold. The glide
## itself needs a Player with tweens and is checked by eye in the exported build.
## Run: godot --headless --path client --script res://tests/test_house.gd

var _failures := 0
var _done := false


func _check(ok: bool, what: String) -> void:
	if not ok:
		_failures += 1
		push_error("FAIL: " + what)


func _process(_delta: float) -> bool:
	if _done:
		return true
	_done = true
	_run()
	print("RESULT: %s (%d failures)" % ["PASS" if _failures == 0 else "FAIL", _failures])
	quit(1 if _failures > 0 else 0)
	return true


func _run() -> void:
	var world := Node3D.new()
	root.add_child(world)
	var player := Node3D.new()
	player.position = Vector3(10.0, 0.0, 10.0)
	world.add_child(player)
	var house := HouseNode.new()
	house.position = Vector3(-13.0, 0.0, -7.0)
	house.player = player
	world.add_child(house)

	_check(house.is_in_group(Interaction.GROUP), "house is interactable")
	var door := house.door_position()
	_check(door.distance_to(Vector3(-13.86, 0.0, -5.65)) < 0.05, "door on the south face, left of centre: %s" % door)
	var threshold := house.threshold_position()
	_check(threshold.distance_to(Vector3(-13.86, 0.0, -4.65)) < 0.05, "threshold is 1 m south of the door: %s" % threshold)
	_check(house.inside_position().distance_to(Vector3(-13.86, 0.0, -7.05)) < 0.05, "inside point is 1.4 m past the door")
	_check(house.approach_target() == threshold, "walk_to goes to the threshold")
	var found := Interaction.nearest([house], threshold, 2.4)
	_check(found.get("kind", "") == "house" and int(found.get("idx", -1)) == 0, "nearest reports the house from the threshold despite the 2.4 m default")
	_check(Interaction.nearest([house], Vector3(-13.0, 0.0, -1.0), 2.4).is_empty(), "6 m south of the house is too far")

	house.set_inside(true)
	_check(not player.visible, "inside: character hidden")
	_check(player.global_position.distance_to(house.inside_position()) < 0.01, "inside: character parked past the door")
	house.set_inside(false)
	_check(player.visible, "outside: character shown")
	_check(player.global_position.distance_to(threshold) < 0.01, "outside: character stands on the threshold")
	house.set_inside(false)
	_check(player.visible, "leaving twice is harmless")
