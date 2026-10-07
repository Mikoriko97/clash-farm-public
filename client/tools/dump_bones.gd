extends SceneTree
## Prints the Skeleton3D bone list (index, name, parent) of an imported scene,
## to plan animation retargeting between rigs.
## Run: godot --headless --path client --script res://tools/dump_bones.gd -- --file=res://assets/character/HumanM_Model.fbx


func _initialize() -> void:
	var file := "res://assets/character/HumanM_Model.fbx"
	for arg: String in OS.get_cmdline_user_args():
		if arg.begins_with("--file="):
			file = arg.trim_prefix("--file=")
	var scene := load(file) as PackedScene
	if scene == null:
		push_error("cannot load " + file)
		quit(1)
		return
	var root: Node = scene.instantiate()
	var found: Array[Node] = root.find_children("*", "Skeleton3D", true, false)
	if found.is_empty():
		print("no skeleton in " + file)
	for node: Node in found:
		var skeleton := node as Skeleton3D
		print("skeleton %s path=%s bones=%d" % [skeleton.name, root.get_path_to(skeleton), skeleton.get_bone_count()])
		for i: int in skeleton.get_bone_count():
			var rest := skeleton.get_bone_rest(i)
			print("  %2d %-32s parent=%2d rest=%s" % [i, skeleton.get_bone_name(i), skeleton.get_bone_parent(i), rest.origin])
	root.free()
	quit()
