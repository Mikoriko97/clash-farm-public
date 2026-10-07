extends SceneTree
## Headless test: each world scene carries a ZoneSettle node that reports readiness
## only after its work sources drain (island wild growth, mainland warmup and chunks),
## never before `min_frames`, and the cave (no sources) settles on frames alone.
## Run: godot --headless --path client --script res://tests/test_zone_settle.gd

var _failures := 0
var _frames := 0
var _scene: Node3D
var _step := 0


func _check(ok: bool, what: String) -> void:
	if not ok:
		_failures += 1
		push_error("FAIL: " + what)


func _init() -> void:
	_load("res://scenes/island/Island.tscn")


func _load(path: String) -> void:
	if _scene != null:
		_scene.queue_free()
	_scene = (load(path) as PackedScene).instantiate()
	root.add_child(_scene)
	_frames = 0


func _process(_delta: float) -> bool:
	_frames += 1
	var settle: ZoneSettle = _scene.get_node("Settle")
	if _frames == 2:
		_check(settle != null and not settle.is_ready(), "%s: not ready on frame 2" % _scene.name)
	if _frames == 120:
		_check(settle.is_ready(), "%s: ready within 120 frames" % _scene.name)
		match _step:
			0:
				_check((_scene.get_node("Wild") as IslandWild).pending() == 0, "island wild growth drained before ready")
				_load("res://scenes/mainland/Mainland.tscn")
			1:
				var terrain: MainlandTerrain = _scene.get_node("Terrain")
				_check(terrain.pending_count() == 0 and terrain.flora_pending() == 0, "mainland chunks and flora drained before ready")
				_load("res://scenes/cave/Cave.tscn")
			2:
				print("RESULT: %s (%d failures)" % ["PASS" if _failures == 0 else "FAIL", _failures])
				quit(1 if _failures > 0 else 0)
				return true
		_step += 1
	return false
