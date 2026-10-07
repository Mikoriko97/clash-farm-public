extends SceneTree
## Headless test (аудит анімацій 2026-10-03): кожен кліп, який грає гра, — рух, спокій, дії з інструментами, «життя», реакції й
## емоції (`FxClips`, `CharacterLife`), — стоїть на землі, не ковзає, ходьба й біг рухають стопу зі швидкістю гри, а кисті й
## інструмент не проходять крізь тіло (`AnimProbe`; межі тут грубіші за точну перевірку в Blender під час запікання —
## ловлять те, що видно: проходження на 8–12 см, яке було до аудиту). Без помилок рушія.
## Run: godot --headless --path client --script res://tests/test_anim_audit.gd

const ErrorLog := preload("res://tests/error_log.gd")
## Кліпи руху, які вмикає `player.gd` (крім `FxClips` і варіантів спокою).
const LOCOMOTION: Array[String] = ["idle", "walk_forward", "run_forward", "jump", "sitting_idle", "idle_torch"]
## Лише в повітрі: підошва «під землею» відносно кореня там не видна.
const AIRBORNE: Array[String] = ["jump"]
## Межі (см сцени).
const MAX_PEN := 3.0
const MAX_FLOAT := 3.5
const MAX_INSIDE := 5.0
const MAX_SLIDE := 5.0
const SPEED_TOLERANCE := 0.2

var _failures := 0
var _frames := 0
var _player: Node3D
var _log := ErrorLog.new()


func _check(ok: bool, what: String) -> void:
	if not ok:
		_failures += 1
		push_error("FAIL: " + what)


func _init() -> void:
	OS.add_logger(_log)
	_player = (load("res://scenes/player/Player.tscn") as PackedScene).instantiate() as Node3D
	root.add_child(_player)
	_player.process_mode = Node.PROCESS_MODE_DISABLED


func _process(_delta: float) -> bool:
	_frames += 1
	if _frames < 3:
		return false
	var probe := AnimProbe.new(_player)
	for clip: String in _clips():
		_check_clip(probe.audit(clip))
	_check_stride()
	OS.remove_logger(_log)
	_check(_log.errors.is_empty(), "no engine errors: %s" % "; ".join(_log.errors.slice(0, 3)))
	print("RESULT: %s (%d failures)" % ["PASS" if _failures == 0 else "FAIL", _failures])
	quit(1 if _failures > 0 else 0)
	return true


func _clips() -> Array[String]:
	var out: Array[String] = LOCOMOTION.duplicate()
	for fx: String in FxClips.TABLE:
		var clip := String((FxClips.TABLE[fx] as Dictionary)["clip"])
		if not clip in out:
			out.append(clip)
	for clip: String in CharacterLife.IDLE_VARIANTS:
		if not clip in out:
			out.append(clip)
	return out


func _check_clip(w: Dictionary) -> void:
	var clip := String(w["clip"])
	if not clip in AIRBORNE:
		_check(float(w["pen"]) <= MAX_PEN, "%s: the soles stay above the ground (%.1f cm under at %.2f s)" % [clip, w["pen"], w["pen_t"]])
	_check(float(w["float"]) <= MAX_FLOAT, "%s: a standing character keeps a foot down (%.1f cm up at %.2f s)" % [clip, w["float"], w["float_t"]])
	_check(float(w["body"]) <= MAX_INSIDE, "%s: hands stay out of the body (%.1f cm, %s at %.2f s)" % [clip, w["body"], w["body_where"], w["body_t"]])
	_check(float(w["head"]) <= MAX_INSIDE, "%s: hands stay out of the head (%.1f cm at %.2f s)" % [clip, w["head"], w["head_t"]])
	_check(float(w["tool"]) <= MAX_INSIDE, "%s: the tool stays out of the body (%.1f cm, %s at %.2f s)" % [clip, w["tool"], w["tool_where"], w["tool_t"]])
	_check(float(w["slide"]) <= MAX_SLIDE, "%s: a planted foot does not slide (%.1f cm)" % [clip, w["slide"]])
	_check(float(w["knee"]) >= -0.3, "%s: knees bend forward (%.2f at %.2f s)" % [clip, w["knee"], w["knee_t"]])
	if float(w["speed"]) > 0.0:
		_check(absf(float(w["speed"]) - 1.0) <= SPEED_TOLERANCE, "%s: the planted foot moves with the character (%.0f %% of game speed)" % [clip, float(w["speed"]) * 100.0])


## Темп кліпу — від справжньої швидкості: на повному ходу 1, на пів-ходу — повільніше (стопа не ковзає).
func _check_stride() -> void:
	var stride := _player.get_node_or_null("StrideSync") as StrideSync
	_check(stride != null, "the player has a StrideSync")
	if stride == null:
		return
	var anim := _player.get_node("AnimationPlayer") as AnimationPlayer
	anim.play(&"walk_forward")
	var body := _player as CharacterBody3D
	body.velocity = Vector3(0.0, 0.0, -2.2)
	for i in 30:
		stride._process(1.0 / 60.0)
	_check(absf(anim.speed_scale - 1.0) < 0.05, "full walking speed plays the walk at its own pace (%.2f)" % anim.speed_scale)
	body.velocity = Vector3(1.1, 0.0, 0.0)
	for i in 60:
		stride._process(1.0 / 60.0)
	_check(absf(anim.speed_scale - 0.5) < 0.05, "half speed plays it at half pace (%.2f)" % anim.speed_scale)
	anim.play(&"chop")
	stride._process(1.0 / 60.0)
	_check(anim.speed_scale == 1.0, "an action clip is left at its own pace")
	body.velocity = Vector3.ZERO
