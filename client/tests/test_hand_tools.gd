extends SceneTree
## Headless test (ADR-034): the right hand carries the seven tools as meshes skinned to the hand bone, an action shows
## the right one with its own clip (chop, mine, dig, reap, hammer, water) and locomotion hides it again; each tool lies at the hand.
## Run: godot --headless --path client --script res://tests/test_hand_tools.gd

var _failures := 0
var _player: Player
var _frames := 0


func _check(ok: bool, what: String) -> void:
	if not ok:
		_failures += 1
		push_error("FAIL: " + what)


func _init() -> void:
	_player = (load("res://scenes/player/Player.tscn") as PackedScene).instantiate()
	root.add_child(_player)


func _process(_delta: float) -> bool:
	_frames += 1
	if _frames < 3:
		return false
	var hand: HandTools = _player.get_node_or_null("%RightHand") as HandTools
	_check(hand != null, "RightHand attachment exists")
	if hand == null:
		_finish()
		return true
	var skeleton := _player.find_child("Skeleton3D", true, false) as Skeleton3D
	var hand_rest := skeleton.get_bone_global_rest(skeleton.find_bone("RightHand")).origin
	for tool: String in HandTools.TOOLS:
		var mesh := hand.get_node_or_null(NodePath(tool)) as MeshInstance3D
		_check(mesh != null and mesh.skin != null and mesh.get_node(mesh.skeleton) == skeleton, "tool %s is skinned to the character's skeleton" % tool)
		if mesh != null:
			# The grip is set in the rest pose: the tool's box reaches the hand (the handle runs through the fist).
			var gap := mesh.get_aabb().grow(0.02).has_point(hand_rest) or mesh.get_aabb().get_center().distance_to(hand_rest) < 0.6
			_check(gap, "tool %s lies at the hand in the rest pose (%s vs %s)" % [tool, mesh.get_aabb(), hand_rest])
	_check(hand.shown() == "", "nothing in hand at start")
	for pair: Array in [[&"chop", "axe"], [&"mine", "pickaxe"], [&"dig", "spade"], [&"reap", "scythe"], [&"hammer", "hammer"], [&"water", "can"]]:
		_check((_player.get_node("AnimationPlayer") as AnimationPlayer).has_animation(pair[0]), "clip %s exists" % pair[0])
		_player.play_action(pair[0], pair[1])
		_check(hand.shown() == pair[1], "%s shows the %s only" % [pair[0], pair[1]])
	_player.play_action(&"pickup", "")
	_check(hand.shown() == "", "bare-handed action hides tools")
	_player.play_action(&"reap", "scythe")
	_player._update_animation(true, false)
	_check(hand.shown() == "", "walking away puts the tool down")
	hand.show_tool("laser")
	_check(hand.shown() == "", "unknown tool shows nothing")
	_finish()
	return true


func _finish() -> void:
	print("RESULT: %s (%d failures)" % ["PASS" if _failures == 0 else "FAIL", _failures])
	quit(1 if _failures > 0 else 0)
