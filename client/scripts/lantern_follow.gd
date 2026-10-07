class_name LanternFollow
extends OmniLight3D
## The lantern's glow in the cave (docs/MAINLAND.md §3): a point light that
## rides at the character's raised hand so the dark opens only around them.
## Pure view; how long the oil lasts is a later server rule.

@export var target: Node3D
@export var offset: Vector3 = Vector3(0.35, 1.5, 0.35)
## Gentle flicker (energy ± this, a few times a second).
@export var flicker: float = 0.12

var _base_energy: float = 1.0
var _time: float = 0.0


func _ready() -> void:
	_base_energy = light_energy


func _process(delta: float) -> void:
	if target == null:
		return
	_time += delta
	global_position = target.global_position + offset
	light_energy = _base_energy + sin(_time * 9.0) * flicker * 0.5 + sin(_time * 23.0) * flicker * 0.5
