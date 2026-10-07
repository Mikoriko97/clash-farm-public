extends SceneTree
## Headless test (ADR-035): «життя» персонажа — кожна дія, реакція й емоція таблиці `FxClips` має свій кліп; кліпання заплющує й
## розплющує очі; кліп дії ставить свій вираз і після нього повертається базовий; постоявши, свій персонаж робить дію спокою;
## реакція (рівень, невдача) чекає кінця поточної дії; голова повертається до цілі лише у свого (модифікатор після анімації),
## чужий лише кліпає. Без помилок рушія.
## Run: godot --headless --path client --script res://tests/test_character_life.gd

const ErrorLog := preload("res://tests/error_log.gd")

var _failures := 0
var _frames := 0
var _player: Node
var _peer: Node
var _log := ErrorLog.new()


func _check(ok: bool, what: String) -> void:
	if not ok:
		_failures += 1
		push_error("FAIL: " + what)


func _init() -> void:
	OS.add_logger(_log)
	_player = (load("res://scenes/player/Player.tscn") as PackedScene).instantiate()
	root.add_child(_player)
	_peer = (load("res://scenes/player/PeerAvatar.tscn") as PackedScene).instantiate()
	root.add_child(_peer)


func _process(_delta: float) -> bool:
	_frames += 1
	if _frames < 3:
		return false
	_check_table()
	var life := _player.get_node("Life") as CharacterLife
	_check(life != null and life.own, "the player has its own life")
	if life != null:
		_check_face(life)
		_check_idle(life)
		_check_react(life)
	_check_look()
	OS.remove_logger(_log)
	_check(_log.errors.is_empty(), "no engine errors: %s" % "; ".join(_log.errors.slice(0, 3)))
	print("RESULT: %s (%d failures)" % ["PASS" if _failures == 0 else "FAIL", _failures])
	quit(1 if _failures > 0 else 0)
	return true


func _check_table() -> void:
	var lib := load("res://assets/character/character_anims.tres") as AnimationLibrary
	for fx: String in FxClips.TABLE:
		var clip: StringName = (FxClips.TABLE[fx] as Dictionary)["clip"]
		_check(lib.has_animation(clip), "fx %s plays an existing clip %s" % [fx, clip])
	for emote: String in ["wave", "cheer", "laugh", "dance", "heart", "shrug", "thumbs_up", "clap"]:
		_check(FxClips.TABLE.has("emote_" + emote), "the emote wheel has %s" % emote)
	for clip: String in CharacterLife.IDLE_VARIANTS:
		_check(lib.has_animation(clip), "idle variant %s exists" % clip)
	for clip: String in CharacterLife.CLIP_FACES:
		var cell := int(CharacterLife.CLIP_FACES[clip])
		_check(cell >= 0 and cell < CharacterFace.COLS * CharacterFace.ROWS, "%s has an atlas cell" % clip)


func _face_mat() -> Material:
	var face := _player.find_child(CharacterFace.NODE, true, false) as MeshInstance3D
	return face.get_surface_override_material(0) if face != null else null


func _check_face(life: CharacterLife) -> void:
	var base := CharacterFace.material(CharacterFace.CELLS["face_neutral"])
	_check(_face_mat() == base, "the face starts with the look's expression")
	life._blink_wait = 0.0
	life._blink(0.01)
	_check(_face_mat() == CharacterFace.material(CharacterLife.BLINK), "a blink closes the eyes")
	life._blink(0.2)
	_check(_face_mat() == base, "and opens them again")
	life._on_clip(&"cheer")
	_check(life.current_cell() == CharacterLife.JOY and _face_mat() == CharacterFace.material(CharacterLife.JOY), "cheering shows joy")
	life._blink_wait = 0.0
	life._blink(0.01)
	_check(_face_mat() == CharacterFace.material(CharacterLife.JOY), "eyes already closed in joy do not blink")
	life._on_clip(&"walk_forward")
	_check(_face_mat() == base, "after the clip the look's expression is back")


func _check_idle(life: CharacterLife) -> void:
	var anim := _player.get_node("AnimationPlayer") as AnimationPlayer
	anim.play(&"idle")
	life._idle_for = 0.0
	life._idle_after = 0.1
	life._idle(0.2)
	_check(String(anim.current_animation) in CharacterLife.IDLE_VARIANTS, "standing still plays an idle variant (%s)" % anim.current_animation)


func _check_react(life: CharacterLife) -> void:
	var anim := _player.get_node("AnimationPlayer") as AnimationPlayer
	_check(bool((FxClips.TABLE["cheer"] as Dictionary).get("after", false)), "a level-up cheer waits for the current action")
	_player.call("play_action", &"reap", "scythe")
	life.react(&"cheer")
	_check(anim.current_animation == &"reap", "the reaction does not cut the harvest (%s)" % anim.current_animation)
	_player.set("_action_left", 0.0)
	life._react_pending(0.1)
	_check(anim.current_animation == &"cheer", "and plays right after it (%s)" % anim.current_animation)
	life.react(&"shake_no")
	_check(anim.current_animation == &"cheer", "a reaction during a reaction waits too")
	life._react_pending(CharacterLife.REACT_WAIT + 0.1)
	_check(anim.current_animation == &"cheer", "a stale reaction is dropped")


func _check_look() -> void:
	var look := _player.find_child("HeadLook", true, false) as LookAtModifier3D
	_check(look != null and look.bone_name == "Head" and look.influence == 0.0, "the player's head can turn to a target, straight by default")
	_check(_peer.find_child("HeadLook", true, false) == null, "a peer has no head look of its own")
	var peer_life := _peer.get_node("Life") as CharacterLife
	_check(peer_life != null and not peer_life.own, "a peer blinks but does not pick idle variants")
