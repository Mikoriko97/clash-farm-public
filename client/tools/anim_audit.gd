extends SceneTree
## Аудит усіх кліпів персонажа (`AnimProbe`, аудит анімацій 2026-10-03): рядок на кліп — «ok» або що виходить за межі.
## Run: godot --headless --path client --script res://tools/anim_audit.gd [-- --clip=idle --json=C:/tmp/audit.json]

const PLAYER := "res://scenes/player/Player.tscn"
const LIBRARY := "res://assets/character/character_anims.tres"

var _frames := 0
var _player: Node3D
var _args: Dictionary = {}


func _init() -> void:
	for arg: String in OS.get_cmdline_user_args():
		var pair := arg.trim_prefix("--").split("=", true, 1)
		_args[pair[0]] = pair[1] if pair.size() > 1 else "true"
	_player = (load(PLAYER) as PackedScene).instantiate() as Node3D
	root.add_child(_player)
	_player.process_mode = Node.PROCESS_MODE_DISABLED


func _process(_delta: float) -> bool:
	_frames += 1
	if _frames < 3:
		return false
	var probe := AnimProbe.new(_player)
	var report: Array = []
	var lib := load(LIBRARY) as AnimationLibrary
	for clip: StringName in lib.get_animation_list():
		if _args.has("clip") and String(clip) != str(_args["clip"]):
			continue
		var w := probe.audit(String(clip))
		var flags := AnimProbe.flags(w)
		print("%-15s %s" % [clip, "ok" if flags.is_empty() else " ".join(flags)])
		report.append(w)
	if _args.has("json"):
		var f := FileAccess.open(str(_args["json"]), FileAccess.WRITE)
		f.store_string(JSON.stringify(report, "  "))
	quit()
	return true
