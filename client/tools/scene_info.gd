extends SceneTree
## Prints the node tree of an imported scene with mesh bounds (metres), to size
## and orient props such as the hand tools before placing them in a scene.
## Run: godot --headless --path client --script res://tools/scene_info.gd -- --file=res://assets/tools/AxeV1.fbx


func _initialize() -> void:
	var files: PackedStringArray = []
	for arg: String in OS.get_cmdline_user_args():
		if arg.begins_with("--file="):
			files.append(arg.trim_prefix("--file="))
	for file: String in files:
		var scene := load(file) as PackedScene
		if scene == null:
			push_error("cannot load " + file)
			continue
		var root: Node = scene.instantiate()
		print("== %s" % file)
		_walk(root, root, 0)
		root.free()
	quit()


func _walk(node: Node, root: Node, depth: int) -> void:
	var line := "  ".repeat(depth) + "%s (%s)" % [node.name, node.get_class()]
	if node is MeshInstance3D:
		var mi := node as MeshInstance3D
		var box := mi.get_aabb()
		var xf := (root as Node3D).global_transform.affine_inverse() * mi.global_transform if root is Node3D else mi.global_transform
		line += " aabb=%s size=%s xf.origin=%s scale=%s surfaces=%d" % [box.position, box.size, xf.origin, xf.basis.get_scale(), mi.mesh.get_surface_count() if mi.mesh else 0]
		if mi.mesh != null and mi.mesh.get_surface_count() > 0:
			var mat := mi.mesh.surface_get_material(0)
			line += " mat=%s" % (mat.resource_name if mat != null else "none")
	elif node is Node3D:
		line += " origin=%s" % (node as Node3D).transform.origin
	print(line)
	for child: Node in node.get_children():
		_walk(child, root, depth + 1)
