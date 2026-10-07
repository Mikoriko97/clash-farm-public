extends SceneTree
## Prints every animation of an imported scene (name, length, loop, tracks) to
## pick clips from a library such as Quaternius UAL.
## Run: godot --headless --path client --script res://tools/list_anims.gd -- --file=res://assets/character/ual2.glb


func _initialize() -> void:
	var file := "res://assets/character/ual2.glb"
	for arg: String in OS.get_cmdline_user_args():
		if arg.begins_with("--file="):
			file = arg.trim_prefix("--file=")
	var scene := load(file) as PackedScene
	if scene == null:
		push_error("cannot load " + file)
		quit(1)
		return
	var root: Node = scene.instantiate()
	var players: Array[Node] = root.find_children("*", "AnimationPlayer", true, false)
	if players.is_empty():
		print("no AnimationPlayer in " + file)
	for node: Node in players:
		var player := node as AnimationPlayer
		var names := player.get_animation_list()
		print("player %s: %d animations" % [player.name, names.size()])
		for n: StringName in names:
			var a := player.get_animation(n)
			print("  %-40s len=%5.2f tracks=%3d loop=%d" % [n, a.length, a.get_track_count(), a.loop_mode])
	root.free()
	quit()
