extends SceneTree
## Друкує матеріали й меші імпортованого GLB (перевірка, що всі шматки паку
## однакові за шейдером): godot --headless --path client --script res://tools/inspect_piece.gd -- --piece=tree_oak_regular


func _init() -> void:
	var piece := "tree_oak_regular"
	for a: String in OS.get_cmdline_user_args():
		if a.begins_with("--piece="):
			piece = a.trim_prefix("--piece=")
	var scene: PackedScene = load("res://assets/nature/pieces/%s.glb" % piece)
	var root := scene.instantiate()
	for n: Node in root.find_children("*", "MeshInstance3D", true, false):
		var mi := n as MeshInstance3D
		print("mesh %s surfaces=%d aabb=%s" % [mi.name, mi.mesh.get_surface_count(), mi.get_aabb()])
		for s: int in mi.mesh.get_surface_count():
			var m := mi.mesh.surface_get_material(s)
			if m is BaseMaterial3D:
				var b := m as BaseMaterial3D
				print("  mat %s tex=%s vcol=%s cull=%d transp=%d rough=%.2f metal=%.2f flags=%s" % [b.resource_name, b.albedo_texture != null, b.vertex_color_use_as_albedo, b.cull_mode, b.transparency, b.roughness, b.metallic, str(b.get("albedo_color"))])
				if b.albedo_texture != null:
					print("  tex size=%s" % str(b.albedo_texture.get_size()))
			else:
				print("  mat %s" % str(m))
		var arrays := mi.mesh.surface_get_arrays(0)
		print("  has_color=%s has_uv=%s" % [arrays[Mesh.ARRAY_COLOR] != null, arrays[Mesh.ARRAY_TEX_UV] != null])
	root.free()
	quit()
