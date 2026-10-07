extends SceneTree
## Prints the top-level object hierarchy of the farm pack FBX (name, children, meshes)
## so multi-part buildings can be re-extracted with their original part transforms.
## Run: godot --headless --path client --script res://tools/fbx_nodes.gd -- --filter=house

const FBX := "res://farm_builds/LowPoly_Farm_Pack_fbx.FBX"


func _initialize() -> void:
	var filter := ""
	for arg: String in OS.get_cmdline_user_args():
		if arg.begins_with("--filter="):
			filter = arg.trim_prefix("--filter=")
	var scene := load(FBX) as PackedScene
	if scene == null:
		push_error("fbx not loaded")
		quit(1)
		return
	var root: Node = scene.instantiate()
	print("root=%s children=%d" % [root.name, root.get_child_count()])
	for child: Node in root.get_children():
		if filter != "" and not child.name.to_lower().contains(filter):
			continue
		var meshes := child.find_children("*", "MeshInstance3D", true, false)
		var own := 1 if child is MeshInstance3D else 0
		print("%s class=%s children=%d meshes=%d xf=%s" % [child.name, child.get_class(), child.get_child_count(), meshes.size() + own, str((child as Node3D).transform.origin) if child is Node3D else "-"])
	root.free()
	quit()
