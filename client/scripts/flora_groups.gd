@tool
class_name FloraGroups
extends Node3D
## Тримач герой-груп (`Mainland/Groups`): у грі садить кожну `FloraGroup` на
## землю (найнижча точка під слідом, як флора), у редакторі те саме робить
## кнопка «Посадити на землю» (коли `Terrain.editor_preview` показує карту).
## Запікання бере дітей звідси як зайняті кола (`tools/flora_baker.gd`).

@export var terrain: MainlandTerrain
@export_tool_button("Посадити на землю") var snap_action := snap_all


func _ready() -> void:
	if not Engine.is_editor_hint():
		snap_all()


func groups() -> Array[FloraGroup]:
	var out: Array[FloraGroup] = []
	for child: Node in get_children():
		if child is FloraGroup:
			out.append(child)
	return out


func snap_all() -> void:
	if terrain == null:
		return
	for g: FloraGroup in groups():
		var p := g.position
		g.position = Vector3(p.x, terrain.ground_min(p.x, p.z, g.radius_m * 0.5), p.z)
