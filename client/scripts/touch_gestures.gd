class_name TouchGestures
extends RefCounted
## Fingers on the 3D view (owner 2026-10-04: one thumb on the joystick, the other turning the
## camera — and the camera "saw two fingers"). On the web export only touches that START on the
## canvas reach Godot as InputEventScreenTouch/Drag; a finger on the joystick or any HUD control
## never does, and the shell keeps that finger's `pointermove` away from Godot too
## (`web/src/hud/touchIsolation.ts`). The first finger on the view owns the camera: a drag past
## the slop orbits, a short touch is a tap; other fingers do nothing while it is down and take
## over (as a drag, never a tap) when it lifts. The mouse Godot emulates from touch, and the
## mouse moves the browser reports for the same fingers (window `pointermove`), are echoes:
## `owns_pointer()` tells the caller to drop mouse events while a finger is down and just after.
## The drag delta is our own (last position per finger): the web export computes `relative`
## by the finger's place in the browser event, not by its id, so a thumb on the joystick in the
## same event once made the camera finger jump 720 px (half a turn) — seen in the smoke test.

## Mouse events this long after the last touch are still echoes of it.
var echo_ms := 400
var slop_px := 14.0

var _start := {}  # finger index → where it went down, every finger on the view
var _last := {}  # finger index → its latest position
var _camera := -1
var _moved := false
var _last_ms := -1_000_000


## True when the event was a touch (handled here, the caller stops). A drag of the camera finger
## calls `on_orbit(dx_px, dy_px)`, a tap calls `on_tap(position)`.
func handle(event: InputEvent, on_orbit: Callable, on_tap: Callable) -> bool:
	if event is InputEventScreenTouch:
		_on_touch(event as InputEventScreenTouch, on_tap)
		return true
	if event is InputEventScreenDrag:
		_on_drag(event as InputEventScreenDrag, on_orbit)
		return true
	return false


## While a finger is on the view (and a moment after), mouse events are echoes of touches.
func owns_pointer() -> bool:
	return not _start.is_empty() or Time.get_ticks_msec() - _last_ms < echo_ms


## React took the screen (`set_mode ui`): the fingers' releases may never come here.
func reset() -> void:
	_start.clear()
	_last.clear()
	_camera = -1


func _on_touch(t: InputEventScreenTouch, on_tap: Callable) -> void:
	_last_ms = Time.get_ticks_msec()
	if t.pressed:
		_start[t.index] = t.position
		_last[t.index] = t.position
		if _camera == -1:
			_camera = t.index
			_moved = false
		return
	_start.erase(t.index)
	_last.erase(t.index)
	if t.index != _camera:
		return
	_camera = -1
	if not _moved and not t.canceled:
		on_tap.call(t.position)
	elif not _start.is_empty():
		# Another finger is still down: it carries on turning the camera.
		_camera = int(_start.keys()[0])
		_moved = true


func _on_drag(d: InputEventScreenDrag, on_orbit: Callable) -> void:
	_last_ms = Time.get_ticks_msec()
	if not _last.has(d.index):
		return  # not a finger of the view (its press never came here)
	var delta: Vector2 = d.position - (_last[d.index] as Vector2)
	_last[d.index] = d.position
	if d.index != _camera:
		return
	if not _moved and d.position.distance_to(_start.get(d.index, d.position)) > slop_px:
		_moved = true
	if _moved:
		on_orbit.call(delta.x, delta.y)
