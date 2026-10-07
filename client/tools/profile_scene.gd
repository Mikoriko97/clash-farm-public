extends SceneTree
## Профіль кадрів будь-якої сцени без сервера (вікно, не headless — рендер теж
## рахується). Доповнює `profile_walk.gd` (той веде персонажа материком):
##   godot --path client --disable-vsync --script res://tools/profile_scene.gd -- --scene=res://scenes/island/Island.tscn
##         [--warm=120] [--frames=600] [--player=x,y,z] [--walk=dx,dz]
## Чекає `warm` кадрів (стрімінг, прогрів шейдерів), потім `frames` кадрів міряє;
## з `--walk` персонаж щокадру зсувається на (dx, dz) м (стрімінг чанків, плитки
## трави). `--warm=0` показує ривки самого старту сцени. Друкує середній / p95 /
## найгірший кадр, кадрів > 25 і > 50 мс, і 10 найдовших кадрів з останньою
## поміткою `FrameMark` (що тоді робила гра). Числа нативні; у браузері (wasm,
## один потік) очікувати ×3–5 на CPU-частині.

var _args := {}
var _scene: Node
var _frame := 0
var _times: PackedFloat32Array = PackedFloat32Array()
var _worst: Array = []
var _last_tick := 0
var _marks: Node


func _init() -> void:
	for a: String in OS.get_cmdline_user_args():
		if a.begins_with("--") and a.contains("="):
			_args[a.substr(2, a.find("=") - 2)] = a.substr(a.find("=") + 1)
	var path := str(_args.get("scene", "res://scenes/island/Island.tscn"))
	var packed: PackedScene = load(path)
	if packed == null:
		push_error("profile_scene: cannot load " + path)
		quit(1)
		return
	_scene = packed.instantiate()
	root.add_child(_scene)
	_place_player()


func _place_player() -> void:
	var player := _scene.get_node_or_null(^"Player") as Node3D
	if player == null or not _args.has("player"):
		return
	var p := str(_args["player"]).split(",")
	# `_init` runs before the tree is live: global_* reads fail there; Player is a direct
	# child of the scene root at the origin, so the local position is the world one.
	player.position = Vector3(float(p[0]), float(p[1]), float(p[2]))


func _process(_delta: float) -> bool:
	_frame += 1
	var warm := int(_args.get("warm", 120))
	var frames := int(_args.get("frames", 600))
	var now := Time.get_ticks_usec()
	if _frame > warm:
		if _last_tick > 0:
			_record((now - _last_tick) / 1000.0)
		_walk()
	_last_tick = now
	if _frame > warm + frames:
		_report()
		quit(0)
		return true
	return false


func _walk() -> void:
	if not _args.has("walk"):
		return
	var player := _scene.get_node_or_null(^"Player") as Node3D
	if player == null:
		return
	var d := str(_args["walk"]).split(",")
	player.global_position += Vector3(float(d[0]), 0.0, float(d[1]))


func _record(ms: float) -> void:
	_times.append(ms)
	if _marks == null:
		_marks = root.get_node_or_null(^"DebugStats")
	var mark := str(_marks.get("_mark")) if _marks != null else ""
	_worst.append([ms, _frame, mark])
	_worst.sort_custom(func(a: Array, b: Array) -> bool: return a[0] > b[0])
	if _worst.size() > 10:
		_worst.resize(10)


func _report() -> void:
	var sorted := _times.duplicate()
	sorted.sort()
	var sum := 0.0
	var over25 := 0
	var over50 := 0
	for v: float in sorted:
		sum += v
		if v > 25.0:
			over25 += 1
		if v > 50.0:
			over50 += 1
	var p95 := sorted[mini(int(floor(sorted.size() * 0.95)), sorted.size() - 1)]
	print("profile %s: %d frames, mean %.1f ms, p95 %.1f ms, max %.1f ms, >25 ms: %d, >50 ms: %d" % [
		str(_args.get("scene", "")).get_file(), sorted.size(), sum / maxf(1.0, sorted.size()), p95, sorted[sorted.size() - 1], over25, over50])
	print("primitives %d, draw calls %d, video %.0f MB, nodes %d" % [
		Performance.get_monitor(Performance.RENDER_TOTAL_PRIMITIVES_IN_FRAME),
		Performance.get_monitor(Performance.RENDER_TOTAL_DRAW_CALLS_IN_FRAME),
		Performance.get_monitor(Performance.RENDER_VIDEO_MEM_USED) / 1048576.0,
		Performance.get_monitor(Performance.OBJECT_NODE_COUNT)])
	for w: Array in _worst:
		print("  %.1f ms at frame %d: %s" % [w[0], w[1], w[2]])
