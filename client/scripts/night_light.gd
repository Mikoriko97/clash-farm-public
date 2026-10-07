class_name NightLight
extends OmniLight3D
## A warm light that comes on at dusk (house window, lantern). DayCycle drives
## every node in group "night_lights" with `set_daylight`; the light fades in as
## daylight drops so nothing pops at the sunset threshold. Pure view.

const GROUP := "night_lights"

## Energy at full night.
@export var night_energy: float = 1.6
## Daylight level below which the light starts to show (1 = full day).
@export var on_below: float = 0.55

var _base_set: bool = false


func _ready() -> void:
	add_to_group(GROUP)
	light_energy = 0.0
	visible = false


## 0 = night, 1 = full day (DayCycle.daylight()).
func set_daylight(daylight: float) -> void:
	var t := 1.0 - clampf(daylight / maxf(on_below, 0.01), 0.0, 1.0)
	light_energy = night_energy * t
	visible = t > 0.01
