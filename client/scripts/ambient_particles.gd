class_name AmbientParticles
extends Node3D
## Частинки атмосфери (docs/ARCHITECTURE.md §Godot): пилок і листя вдень,
## світлячки вночі — три CPUParticles3D навколо персонажа. Їде за `follow`,
## вмикає емітери за рівнем дня (`set_daylight` з DayCycle через групу
## night_lights) і гасить усе в дощ (`set_rain` з Weather). Лише вигляд.

@export var follow: Node3D
@export var pollen: CPUParticles3D
@export var leaves: CPUParticles3D
@export var fireflies: CPUParticles3D
## Над землею (м): острів 0, материк — висота терену під персонажем.
@export var height_offset: float = 0.0

var _daylight := 1.0
var _rain := 0.0


func _ready() -> void:
	add_to_group(NightLight.GROUP)


func _process(_delta: float) -> void:
	if follow != null:
		global_position = follow.global_position + Vector3(0.0, height_offset, 0.0)


func set_daylight(daylight: float) -> void:
	_daylight = daylight
	_apply()


func set_rain(rain: float) -> void:
	_rain = rain
	_apply()


func _apply() -> void:
	var dry := _rain < 0.2
	if pollen != null:
		pollen.emitting = dry and _daylight > 0.35
	if leaves != null:
		leaves.emitting = dry and _daylight > 0.1
	if fireflies != null:
		fireflies.emitting = dry and _daylight < 0.25
