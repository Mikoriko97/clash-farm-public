extends SceneTree
## Headless test: MouseLook never captures on headless/touch, Esc and a React panel release,
## `wanted` follows capture/release, and the text helpers of the ripening timer format time.
## Run: godot --headless --path client --script res://tests/test_mouse_look.gd

var _failures := 0


func _check(ok: bool, what: String) -> void:
	if not ok:
		_failures += 1
		push_error("FAIL: " + what)


func _init() -> void:
	var look: Node = load("res://scripts/mouse_look.gd").new()
	look.name = "MouseLook"
	root.add_child(look)
	_check(look.call("enabled") == false, "headless never enables mouse look")
	look.call("capture")
	_check(look.get("wanted") == false, "capture is a no-op when disabled")
	_check(look.call("captured") == false, "cursor stays free on headless")
	look.set("wanted", true)
	look.call("_on_mode_changed", "ui")
	_check(Input.mouse_mode == Input.MOUSE_MODE_VISIBLE, "ui mode leaves the cursor visible")
	look.call("release")
	_check(look.get("wanted") == false, "release clears the wish")
	var timer := load("res://scripts/plot_timer.gd") as GDScript
	_check(timer.call("text_for", "ready", 0.0) == "Ready!", "ready text")
	_check(timer.call("text_for", "empty", 0.0) == "", "empty plot has no label")
	_check(timer.call("text_for", "growing", 30.0) == "<1 min", "under a minute")
	_check(timer.call("text_for", "growing", 12.0 * 60.0) == "12 min", "minutes")
	_check(timer.call("text_for", "growing", 125.0 * 60.0) == "2 h 05", "hours")
	print("RESULT: %s (%d failures)" % ["PASS" if _failures == 0 else "FAIL", _failures])
	quit(1 if _failures > 0 else 0)
