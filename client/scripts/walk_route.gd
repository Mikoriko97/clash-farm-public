class_name WalkRoute
extends RefCounted
## Route the character follows after a click or `walk_to` (Player.walk_path): the current
## waypoint, the rest of the NavGrid path and the stall counter. Pure logic, no nodes:
## Player asks for the direction each physics tick and learns when the walk is over.

## Blocked this many ticks in a row while walking: counts as arrived instead of pushing forever.
const STALL_FRAMES := 12

var active: bool = false
var target: Vector3 = Vector3.ZERO
## Remaining waypoints after `target`; empty for a direct walk.
var path: PackedVector3Array = PackedVector3Array()
## Applies to the last waypoint only.
var stop_distance: float = 0.35
var _stalled: int = 0


func start(points: PackedVector3Array, stop: float) -> void:
	path = points.slice(1)
	target = points[0]
	stop_distance = stop
	active = true
	_stalled = 0


func cancel() -> void:
	active = false
	path = PackedVector3Array()


## Unit direction from `from` to the current waypoint; at the last one the route ends
## (`active` false) and the result is zero — the caller reports the arrival.
func direction(from: Vector3, arrive_distance: float) -> Vector3:
	var to := target - from
	to.y = 0.0
	var last := path.is_empty()
	if to.length() <= (stop_distance if last else arrive_distance):
		if last:
			active = false
			return Vector3.ZERO
		target = path[0]
		path = path.slice(1)
		return direction(from, arrive_distance)
	return to.normalized()


## True on the tick the walk is given up as arrived (blocked by a collider for STALL_FRAMES ticks).
func stalled(dir: Vector3, speed: float) -> bool:
	if not active or dir == Vector3.ZERO:
		_stalled = 0
		return false
	_stalled = _stalled + 1 if speed < 0.15 else 0
	if _stalled < STALL_FRAMES:
		return false
	_stalled = 0
	active = false
	return true
