extends SceneTree
## Headless asset probe (Week 1 spike, PLAN §Тиждень 1 / client).
## Run: godot --headless --path client --script res://tests/probe_assets.gd
## Prints skeleton + animation info for the character and triangle counts for farm_pack pieces.

const CHARACTER_DIR := "res://assets/character/"
const PIECES_DIR := "res://assets/farm_pack/pieces/"
const TOP_N := 12


func _init() -> void:
	_probe_character()
	_probe_farm_pack()
	quit()


func _probe_character() -> void:
	var model: PackedScene = load(CHARACTER_DIR + "HumanM_Model.fbx")
	if model == null:
		push_error("character model failed to load")
		return
	var root: Node = model.instantiate()
	var skels: Array[Node] = root.find_children("*", "Skeleton3D", true, false)
	if skels.is_empty():
		print("character: NO Skeleton3D")
	else:
		var skel: Skeleton3D = skels[0] as Skeleton3D
		var names: PackedStringArray = []
		for i: int in mini(skel.get_bone_count(), 6):
			names.append(skel.get_bone_name(i))
		print("character: bones=%d first=%s" % [skel.get_bone_count(), ", ".join(names)])
	print("character: triangles=%d" % _count_tris(root))
	root.free()
	for f: String in DirAccess.get_files_at(CHARACTER_DIR):
		if not f.ends_with(".fbx") or f == "HumanM_Model.fbx":
			continue
		_probe_anim(f)


func _probe_anim(file: String) -> void:
	var scene: PackedScene = load(CHARACTER_DIR + file)
	if scene == null:
		print("anim %s: FAILED to load" % file)
		return
	var n: Node = scene.instantiate()
	var players: Array[Node] = n.find_children("*", "AnimationPlayer", true, false)
	if players.is_empty():
		print("anim %s: no AnimationPlayer" % file)
	else:
		var ap: AnimationPlayer = players[0] as AnimationPlayer
		for a: StringName in ap.get_animation_list():
			var anim: Animation = ap.get_animation(a)
			print("anim %s: '%s' len=%.2fs tracks=%d" % [file, a, anim.length, anim.get_track_count()])
	n.free()


func _probe_farm_pack() -> void:
	var rows: Array[Array] = []
	var total := 0
	for f: String in DirAccess.get_files_at(PIECES_DIR):
		if not f.ends_with(".tscn"):
			continue
		var scene: PackedScene = load(PIECES_DIR + f)
		if scene == null:
			print("piece %s: FAILED to load" % f)
			continue
		var n: Node = scene.instantiate()
		var tris := _count_tris(n)
		total += tris
		rows.append([tris, f])
		n.free()
	rows.sort_custom(func(a: Array, b: Array) -> bool: return a[0] > b[0])
	print("farm_pack: pieces=%d total_triangles=%d" % [rows.size(), total])
	for i: int in mini(TOP_N, rows.size()):
		print("  top %2d: %7d  %s" % [i + 1, rows[i][0], rows[i][1]])


func _count_tris(node: Node) -> int:
	var tris := 0
	for mi: Node in node.find_children("*", "MeshInstance3D", true, false):
		var mesh: Mesh = (mi as MeshInstance3D).mesh
		if mesh == null:
			continue
		for s: int in mesh.get_surface_count():
			var arrays: Array = mesh.surface_get_arrays(s)
			var idx: Variant = arrays[Mesh.ARRAY_INDEX]
			if idx != null and (idx as PackedInt32Array).size() > 0:
				tris += (idx as PackedInt32Array).size() / 3
			else:
				tris += (arrays[Mesh.ARRAY_VERTEX] as PackedVector3Array).size() / 3
	return tris
