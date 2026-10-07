extends SceneTree
## Headless test: fingers on the 3D view (TouchGestures, owner 2026-10-04). The first finger owns
## the camera — past the slop it orbits, a short touch is a tap; a second finger on the view
## does not turn it, and takes over (no tap) when the first lifts; a cancelled touch is no tap;
## mouse events are echoes while a finger is down and for `echo_ms` after; the delta is taken from
## the finger's own positions, never from the engine's `relative` (wrong on the web with two fingers).
## Run: godot --headless --path client --script res://tests/test_touch_gestures.gd

var _failures := 0
var _orbits: Array[Vector2] = []
var _taps: Array[Vector2] = []


func _check(ok: bool, what: String) -> void:
	if not ok:
		_failures += 1
		push_error("FAIL: " + what)


func _orbit(dx: float, dy: float) -> void:
	_orbits.append(Vector2(dx, dy))


func _tap(at: Vector2) -> void:
	_taps.append(at)


func _touch(g: TouchGestures, index: int, at: Vector2, pressed: bool, canceled := false) -> void:
	var t := InputEventScreenTouch.new()
	t.index = index
	t.position = at
	t.pressed = pressed
	t.canceled = canceled
	g.handle(t, _orbit, _tap)


func _drag(g: TouchGestures, index: int, at: Vector2, relative: Vector2) -> void:
	var d := InputEventScreenDrag.new()
	d.index = index
	d.position = at
	d.relative = relative
	g.handle(d, _orbit, _tap)


func _init() -> void:
	var g := TouchGestures.new()
	g.echo_ms = 0
	# One finger: inside the slop nothing turns, past it the camera follows the finger.
	_touch(g, 0, Vector2(400, 300), true)
	_drag(g, 0, Vector2(405, 300), Vector2(5, 0))
	_check(_orbits.is_empty(), "a jitter inside the slop does not orbit")
	_drag(g, 0, Vector2(430, 300), Vector2(720, 0))
	_check(_orbits == [Vector2(25, 0)], "past the slop the camera finger orbits by its own move, not a bogus relative: %s" % [_orbits])
	_check(g.owns_pointer(), "mouse events are echoes while a finger is down")
	# A second finger on the view does not turn the camera.
	_touch(g, 1, Vector2(900, 500), true)
	_drag(g, 1, Vector2(960, 500), Vector2(60, 0))
	_check(_orbits.size() == 1, "a second finger does not orbit: %s" % [_orbits])
	# The camera finger lifts after a drag: no tap; the other finger takes over the camera.
	_touch(g, 0, Vector2(430, 300), false)
	_check(_taps.is_empty(), "a drag is never a tap")
	_drag(g, 1, Vector2(970, 496), Vector2(10, -4))
	_check(_orbits.back() == Vector2(10, -4), "the remaining finger carries on: %s" % [_orbits])
	_touch(g, 1, Vector2(970, 500), false)
	_check(_taps.is_empty(), "the finger that took over is no tap either")
	_check(not g.owns_pointer(), "no finger down, echo window 0: the mouse is the mouse again")
	# A short touch is a tap at its position; a cancelled one is not.
	_touch(g, 2, Vector2(100, 200), true)
	_drag(g, 2, Vector2(104, 203), Vector2(4, 3))
	_touch(g, 2, Vector2(104, 203), false)
	_check(_taps == [Vector2(104, 203)], "a short touch taps where it lifted: %s" % [_taps])
	_touch(g, 3, Vector2(10, 10), true)
	_touch(g, 3, Vector2(10, 10), false, true)
	_check(_taps.size() == 1, "a cancelled touch is no tap")
	# A panel took the screen mid-touch: the release never comes, reset() frees the camera.
	_touch(g, 4, Vector2(10, 10), true)
	g.reset()
	_touch(g, 5, Vector2(300, 300), true)
	_drag(g, 5, Vector2(340, 300), Vector2(40, 0))
	_check(_orbits.back() == Vector2(40, 0), "after reset the next finger owns the camera")
	# The echo window: just after a touch, mouse events still belong to it.
	var e := TouchGestures.new()
	_touch(e, 0, Vector2(1, 1), true)
	_touch(e, 0, Vector2(1, 1), false)
	_check(e.owns_pointer(), "right after a touch the mouse events are echoes")
	_check(not g.handle(InputEventMouseMotion.new(), _orbit, _tap), "mouse events are not touches")
	# A finger whose press never came (the joystick thumb in the same browser event) is ignored.
	var before := _orbits.size()
	_drag(g, 9, Vector2(50, 50), Vector2(300, 0))
	_check(_orbits.size() == before, "a drag of an unknown finger does nothing")
	print("RESULT: %s (%d failures)" % ["PASS" if _failures == 0 else "FAIL", _failures])
	quit(1 if _failures > 0 else 0)
