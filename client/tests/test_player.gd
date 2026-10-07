extends SceneTree
## Headless test: Player.tscn loads, its AnimationLibrary has every clip and
## every animation track resolves to a real node/bone (no silent no-op clips).
## Run: godot --headless --path client --script res://tests/test_player.gd

const EXPECTED := ["idle", "walk_forward", "walk_back", "walk_left", "walk_right",
	"run_forward", "sprint_forward", "jump_begin", "jump_land", "fall",
	"harvest", "plant", "water", "till", "mine", "sitting_idle", "sleeping", "lie_down",
	"jump", "swim_idle", "swim_forward", "crouch_idle", "crouch_forward",
	"idle_torch", "interact", "pickup", "kneel", "swing", "push", "dance", "roll", "drive", "hit",
	"chop", "mine", "dig", "reap", "hammer", "water"]

var _failures := 0
var _player: Node


func _init() -> void:
	var player: Node = (load("res://scenes/player/Player.tscn") as PackedScene).instantiate()
	root.add_child(player)
	var anim: AnimationPlayer = player.get_node("AnimationPlayer") as AnimationPlayer
	# Tracks resolve against the player's root node (`Model/Base45`: the base scene keeps its skeleton one level deeper).
	var model: Node = anim.get_node_or_null(anim.root_node) if anim != null else null
	_check(anim != null and model != null and player.get_node_or_null("Model") != null, "Player has AnimationPlayer and Model")
	for clip: String in EXPECTED:
		_check(anim.has_animation(clip), "clip present: " + clip)
	for clip: StringName in anim.get_animation_list():
		_check_tracks(anim.get_animation(clip), model, String(clip))
	_check(anim.autoplay == "idle", "autoplay is idle")
	_player = player


## @onready fields only exist once the loop runs: the action check waits for the first frame.
func _process(_delta: float) -> bool:
	_check_action_clip(_player, _player.get_node("AnimationPlayer") as AnimationPlayer)
	_check_stick_run()
	_check_stand_up_keeps_stick()
	_check_bridge_connection()
	print("RESULT: %s (%d failures)" % ["PASS" if _failures == 0 else "FAIL", _failures])
	_player.queue_free()
	quit(1 if _failures > 0 else 0)
	return true


## Regression: the locomotion update used to cut an action clip after one frame.
func _check_action_clip(player: Node, anim: AnimationPlayer) -> void:
	player.play_action(&"harvest")
	player._update_animation(false, false)
	_check(anim.current_animation == "harvest", "standing still keeps the action clip playing")
	player._update_animation(true, false)
	_check(anim.current_animation == "walk_forward", "walking away interrupts the action clip")
	_check(not player.is_acting(), "action flag cleared by movement")
	player.play_action(&"harvest")
	player._action_left = 0.0
	player._update_animation(false, false)
	_check(anim.current_animation == "idle", "back to idle once the clip ran out")


## Джойстик біля краю кола біжить, ближче до центру йде кроком; гістерезис не смикає на межі.
func _check_stick_run() -> void:
	_check(StickRun.runs(Vector2(0.0, -1.0), false), "stick at the rim runs")
	_check(not StickRun.runs(Vector2(0.0, -0.8), false), "a stick short of the rim walks")
	_check(StickRun.runs(Vector2(0.0, -0.8), true), "a running stick keeps running a little below the threshold")
	_check(not StickRun.runs(Vector2(0.0, -0.5), true), "a stick pulled back to half walks again")
	_check(not StickRun.runs(Vector2.ZERO, true), "a released stick stops")


## Регресія: вставання з лавки не обнуляє джойстик (React шле лише зміни, тож з утримуваним стіком гравець застряг би стоячи),
## а прохід крізь двері його обнуляє, як і раніше.
func _check_stand_up_keeps_stick() -> void:
	_player._sitting = true
	_player._scripted = true
	_player._scripted_clip = &"sitting_idle"
	_player._sit_exit = _player.global_position + Vector3(0.0, 0.0, 1.0)
	_player.external_input = Vector2(0.0, -1.0)
	_player.stand_up()
	_check(_player.external_input == Vector2(0.0, -1.0), "standing up from a seat keeps the held joystick")
	_player.glide_to(_player.global_position + Vector3(0.0, 0.0, 1.0), 0.1)
	_check(_player.external_input == Vector2.ZERO, "a doorway glide still clears the joystick")


## Регресія: зʼєднання гравця з мостом живе рівно стільки, скільки гравець (інакше після зміни зони мертвий вузол сипав «Cannot convert argument 3»).
func _check_bridge_connection() -> void:
	var bridge := root.get_node_or_null(^"Bridge")
	if bridge == null:
		return
	var before: int = bridge.message_received.get_connections().size()
	var extra: Node = (load("res://scenes/player/Player.tscn") as PackedScene).instantiate()
	root.add_child(extra)
	_check(bridge.message_received.get_connections().size() == before + 1, "a player listens to the bridge")
	extra.free()
	_check(bridge.message_received.get_connections().size() == before, "a freed player leaves no connection on the bridge")


func _check_tracks(animation: Animation, model: Node, clip: String) -> void:
	var bad := 0
	for t: int in animation.get_track_count():
		var path := animation.track_get_path(t)
		var node := model.get_node_or_null(NodePath(String(path).get_slice(":", 0)))
		var skeleton := node as Skeleton3D
		if skeleton == null or path.get_subname_count() == 0 \
				or skeleton.find_bone(String(path.get_subname(0))) == -1:
			bad += 1
	_check(bad == 0, "%s: all %d tracks resolve (unresolved=%d)" % [clip, animation.get_track_count(), bad])


func _check(ok: bool, what: String) -> void:
	if not ok:
		_failures += 1
	print("%s %s" % ["ok  " if ok else "FAIL", what])
