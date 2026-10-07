extends SceneTree
## Headless test: night lights are off in daylight, ramp in through dusk and are
## full at night; DayCycle reaches them through the group.
## Run: godot --headless --path client --script res://tests/test_night_light.gd

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
	var light := NightLight.new()
	light.night_energy = 2.0
	root.add_child(light)
	_check(light.is_in_group(NightLight.GROUP), "joins the night_lights group")
	light.set_daylight(1.0)
	_check(light.light_energy == 0.0 and not light.visible, "off in full day")
	light.set_daylight(0.0)
	_check(is_equal_approx(light.light_energy, 2.0) and light.visible, "full at night")
	light.set_daylight(0.275)
	_check(light.light_energy > 0.5 and light.light_energy < 1.5, "half-way through dusk: %.2f" % light.light_energy)
	var cycle := DayCycle.new()
	root.add_child(cycle)
	cycle.freeze(23.0)
	_check(light.light_energy > 1.9, "DayCycle at 23:00 drives the group: %.2f" % light.light_energy)
	cycle.freeze(12.0)
	_check(light.light_energy == 0.0, "DayCycle at noon switches it off")
	print("RESULT: %s (%d failures)" % ["PASS" if _failures == 0 else "FAIL", _failures])
	quit(1 if _failures > 0 else 0)
	return true
