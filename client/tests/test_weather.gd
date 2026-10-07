extends SceneTree
## Headless test: the server weather wins over the local schedule, the dev
## override wins over both, malformed payloads are ignored, and a targeted
## strike points the flash towards the tree.
## Run: godot --headless --path client --script res://tests/test_weather.gd

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
	var weather := Weather.new()
	root.add_child(weather)
	_check(weather.is_in_group(Weather.GROUP), "weather joins its group")
	weather.apply_server({"state": "storm", "until": "2026-09-07T10:12:00Z"})
	_check(weather.effective_state() == Weather.State.STORM, "server storm applies")
	weather.apply_server({"state": "lots"})
	_check(weather.effective_state() == Weather.State.STORM, "unknown state ignored")
	weather.apply_server("storm")
	_check(weather.effective_state() == Weather.State.STORM, "non-dictionary ignored")
	weather.apply_server({"state": "Fog"})
	_check(weather.effective_state() == Weather.State.FOG, "case-insensitive state name")
	weather.force(Weather.State.RAIN)
	_check(weather.effective_state() == Weather.State.RAIN, "dev override beats the server")
	weather.auto()
	_check(weather.effective_state() == Weather.State.FOG, "back to the server state")

	var dirs: Array = []
	weather.lightning_struck.connect(func(d: Vector3) -> void: dirs.append(d))
	var follow := Node3D.new()
	follow.position = Vector3(0.0, 0.0, 0.0)
	root.add_child(follow)
	weather.follow = follow
	weather.strike_at(Vector3(10.0, 0.0, 0.0))
	_check(dirs.size() == 1 and dirs[0].x > 0.5 and dirs[0].y > 0.5, "strike towards +x from above: %s" % str(dirs))
	weather.strike_at(Vector3(0.0, 0.0, 0.0))
	_check(dirs.size() == 2 and dirs[1].is_equal_approx(Vector3.UP), "strike right overhead points up")
