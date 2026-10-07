@tool
class_name FloraGroup
extends Node3D
## Герой-група флори материка (етап 4 плану): ручна композиція — повалена колода з
## папороттю, пень із грибами, валуни, сухе дерево — як звичайна сцена
## (`scenes/mainland/groups/*.tscn`), яку власник ставить під `Mainland/Groups`.
## Запікання (`tools/flora_baker.gd`) вважає коло `radius_m` зайнятим: генерована
## флора й вузли ресурсів у групу не лізуть, а з `anchor` підлісок «під кроною»
## (папороть, гриби) тягнеться до неї. У редакторі видно кільце сліду; у грі
## `FloraGroups` садить групу на землю.

## Радіус сліду (м): генерована флора не росте ближче.
@export_range(0.5, 12.0, 0.1) var radius_m: float = 3.0:
	set(v):
		radius_m = v
		_update_ring()
## Група рахується як крона: підлісок з `under_canopy_m` росте довкола.
@export var anchor: bool = true

var _ring: MeshInstance3D


func _ready() -> void:
	if Engine.is_editor_hint():
		_update_ring()


## Кільце сліду лише в редакторі (не зберігається, не експортується).
func _update_ring() -> void:
	if not Engine.is_editor_hint() or not is_inside_tree():
		return
	if _ring == null:
		_ring = MeshInstance3D.new()
		_ring.name = "_ring"
		var mat := StandardMaterial3D.new()
		mat.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
		mat.albedo_color = Color(1.0, 0.8, 0.2, 0.8)
		mat.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
		_ring.material_override = mat
		add_child(_ring)
	var torus := TorusMesh.new()
	torus.inner_radius = maxf(radius_m - 0.08, 0.05)
	torus.outer_radius = radius_m
	torus.rings = 48
	torus.ring_segments = 6
	_ring.mesh = torus
	_ring.position = Vector3(0.0, 0.15, 0.0)
