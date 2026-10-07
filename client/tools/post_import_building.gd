@tool
extends EditorScenePostImport
## Пост-імпорт для glTF будівель з `client/blender` (ADR-021): світильники з
## KHR_lights_punctual отримують тіні (інакше OmniLight3D світить крізь стіни
## на газон), помірну енергію й короткий радіус замість ват Blender, а меш
## `collision_ramp` (невидимий пандус для колайдера) не кидає тінь. Підключається
## у `<файл>.glb.import` рядком `import_script/path="res://tools/post_import_building.gd"`.

const ENERGY := {"light_atrium": 1.6, "light_wing_l": 1.1, "light_wing_r": 0.9}
const RANGE := {"light_atrium": 9.0, "light_wing_l": 7.0, "light_wing_r": 6.0}


func _post_import(scene: Node) -> Object:
	_fix(scene)
	return scene


func _fix(node: Node) -> void:
	if node is OmniLight3D:
		var light := node as OmniLight3D
		light.light_energy = float(ENERGY.get(light.name, 1.0))
		light.omni_range = float(RANGE.get(light.name, 6.0))
		light.omni_attenuation = 1.5
		light.shadow_enabled = true
		light.shadow_bias = 0.08
	elif node is MeshInstance3D and node.name.begins_with("collision_ramp"):
		(node as MeshInstance3D).cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	for child: Node in node.get_children():
		_fix(child)
