extends SceneTree
## Renders farm_pack pieces (or a whole scene) to PNG so the world can be checked
## without a human screenshot. Needs a window (not --headless).
## Pieces:  godot --path client --script res://tools/snapshot.gd -- --pieces=tree_a,field_b --out=C:/tmp
##          godot --path client --script res://tools/snapshot.gd -- --all --out=C:/tmp   (contact sheet)
## Scene:   godot --path client --script res://tools/snapshot.gd -- --scene=res://scenes/island/Island.tscn --out=C:/tmp --cam=0,30,30 [--demo [--crops=seed_a,seed_b [--progress=0.5]]] [--hour=19.5] [--weather=rain] [--player=x,y,z | --player=door] [--burst=N] [--expand=n,e,s,w]
## Each piece is drawn as-is (no rotation) on a 1 m grid with an axis gizmo:
## red = +X, green = +Y (up in Godot), blue = +Z. A piece standing on its side is Z-up.

const PIECES_DIR := "res://assets/farm_pack/pieces/"
const TILE := 220
const COLUMNS := 6

var _out := "user://"
var _stage: Node3D
var _camera: Camera3D


var _started := false
var _frame_count := 0


## Work starts on the first frame (nodes only join the tree once the loop runs); a watchdog quits on hangs.
func _process(_delta: float) -> bool:
	_frame_count += 1
	if _frame_count > 3000:
		push_error("snapshot: watchdog timeout")
		quit(2)
	if not _started:
		_started = true
		_run()
	return false


func _run() -> void:
	var args := _parse_args()
	_out = str(args.get("out", "user://")).trim_suffix("/") + "/"
	DirAccess.make_dir_recursive_absolute(_out)
	if args.has("scene"):
		_run_scene(str(args["scene"]), str(args.get("cam", "0,28,28")), str(args.get("look", "0,0,0")), str(args.get("name", "scene")))
	else:
		var names: Array = []
		if args.has("all"):
			for file: String in DirAccess.get_files_at(PIECES_DIR):
				if file.ends_with(".tscn"):
					names.append(file.trim_suffix(".tscn"))
		elif args.has("props"):
			for file: String in DirAccess.get_files_at("res://scenes/props/"):
				if file.ends_with(".tscn"):
					names.append("res://scenes/props/" + file)
		else:
			names = str(args.get("pieces", "")).split(",", false)
		_run_pieces(names, args.has("all") or args.has("props") or args.has("sheet"))


func _parse_args() -> Dictionary:
	var out := {}
	for arg: String in OS.get_cmdline_user_args():
		var pair := arg.trim_prefix("--").split("=", true, 1)
		out[pair[0]] = pair[1] if pair.size() > 1 else true
	return out


func _run_scene(path: String, cam: String, look: String, name: String) -> void:
	root.get_window().size = Vector2i(1280, 720)
	var scene: Node = (load(path) as PackedScene).instantiate()
	root.add_child(scene)
	await _frames(2)
	var args := _parse_args()
	if args.has("demo"):
		_apply_demo(scene)
	if args.has("expand"):
		var e: PackedStringArray = str(args["expand"]).split(",")
		for shape: Node in scene.find_children("*", "IslandShape", true, false):
			shape.call("apply_expand", {"n": int(e[0]), "e": int(e[1]), "s": int(e[2]), "w": int(e[3])})
	if args.has("hour"):
		var cycle: Node = scene.find_child("DayCycle", true, false)
		if cycle != null:
			cycle.freeze(float(args["hour"]))
	if args.has("weather"):
		var weather: Node = scene.find_child("Weather", true, false)
		if weather != null:
			weather.force(Weather.state_from_name(str(args["weather"])), true)
	if args.has("player"):
		_place_player(scene, str(args["player"]))
	if args.has("nodes"):
		_place_nodes(scene, str(args["nodes"]))
	if args.has("highlight"):
		for ring: Node in scene.find_children("Highlight", "MeshInstance3D", true, false):
			(ring as MeshInstance3D).visible = true
	# Debug switches: hide the sea, drop the fog, draw ground with a plain vertex-colour material.
	if args.has("nosea"):
		var sea: Node3D = scene.find_child("Sea", true, false) as Node3D
		if sea != null:
			sea.visible = false
	# `--clean`: без накладки F3 (DebugStats у debug-збірці видно завжди) — для картинок назовні (превʼю посилань).
	if args.has("clean"):
		var stats: Node = root.get_node_or_null(^"DebugStats")
		if stats != null and stats.has_method("set_enabled"):
			stats.call("set_enabled", false)
	if args.has("nofog"):
		var env: WorldEnvironment = scene.find_child("WorldEnvironment", true, false) as WorldEnvironment
		if env != null:
			env.environment.fog_enabled = false
	if args.has("plain"):
		var plain := StandardMaterial3D.new()
		plain.vertex_color_use_as_albedo = true
		for mi: Node in scene.find_children("Mesh", "MeshInstance3D", true, false):
			(mi as MeshInstance3D).material_override = plain
	# Streamed worlds (mainland) build one chunk per frame: `--wait=N` gives them time.
	await _frames(maxi(6, int(args.get("wait", "6"))))
	if args.has("plain"):
		var plain2 := StandardMaterial3D.new()
		plain2.vertex_color_use_as_albedo = true
		for mi: Node in scene.find_children("Mesh", "MeshInstance3D", true, false):
			(mi as MeshInstance3D).material_override = plain2
	if args.has("anim"):
		_pose_player(scene, str(args["anim"]), float(args.get("at", "0.5")))
		await _frames(2)
	if args.has("sit"):
		# Sit on demo decor #n and let the glide finish.
		scene.call("_sit", int(args["sit"]))
		await _frames(30)
	var camera := Camera3D.new()
	root.add_child(camera)
	camera.position = _vec(cam)
	camera.look_at(_vec(look), Vector3.UP)
	camera.fov = 45.0
	camera.make_current()
	await _frames(4)
	_save(name)
	# `--burst=N`: N more consecutive frames (name_b00.png…) to measure shimmer and flicker.
	for i: int in int(args.get("burst", "0")):
		await _frames(1)
		_save("%s_b%02d" % [name, i])
	quit()


## `--nodes=kind,kind,...` drops sample mainland nodes of those kinds in a ring around the
## character (no server needed): a quick look at how each kind's piece sits on the terrain.
func _place_nodes(scene: Node, kinds: String) -> void:
	var nodes: Node = scene.find_child("Nodes", true, false)
	var player: Node3D = scene.find_child("Player", true, false) as Node3D
	if nodes == null or player == null:
		return
	var list := kinds.split(",", false)
	var entries: Array = []
	for i: int in list.size():
		var a := TAU * float(i) / float(list.size())
		var p := player.global_position + Vector3(cos(a), 0.0, sin(a)) * 6.0
		entries.append({"id": "demo_%d" % i, "kind": list[i], "x_dm": int(p.x * 10.0), "z_dm": int(p.z * 10.0), "hits_left": 3})
	var c := Vector2i(floori((player.global_position.x + 512.0) / 64.0), floori((player.global_position.z + 512.0) / 64.0))
	nodes.set("spawn_budget_ms", 0.0)
	nodes.call("apply", [{"cx": c.x, "cz": c.y, "nodes": entries}])


## `--player=x,y,z` parks the character there; `--player=door` puts it on the house threshold.
func _place_player(scene: Node, where: String) -> void:
	var player: Node3D = scene.find_child("Player", true, false) as Node3D
	if player == null:
		return
	if where == "door":
		var house: HouseNode = scene.find_child("House", true, false) as HouseNode
		if house != null:
			player.global_position = house.threshold_position()
			player.face_point(house.door_position())
		return
	player.global_position = _vec(where)


## `--anim=<clip> --at=<seconds>` freezes the character mid-clip (checking which pose fits an action).
func _pose_player(scene: Node, clip: String, at: float) -> void:
	var player: Node = scene.find_child("Player", true, false)
	if player == null:
		return
	var anim: AnimationPlayer = player.get_node_or_null("AnimationPlayer") as AnimationPlayer
	if anim == null or not anim.has_animation(clip):
		push_warning("snapshot: no clip %s" % clip)
		return
	# Through play_action so the tool in hand stays up (locomotion would put it down).
	var tool := str(_parse_args().get("tool", ""))
	if player.has_method("play_action"):
		player.call("play_action", StringName(clip), tool)
	# Restart without the blend so the seeked frame is the pure clip pose.
	anim.stop(true)
	anim.play(clip)
	anim.seek(at, true)
	anim.pause()


## Fake world_state so crops and decor can be checked without a server.
func _apply_demo(scene: Node) -> void:
	var now := Time.get_unix_time_from_system()
	var iso := func(t: float) -> String: return Time.get_datetime_string_from_unix_time(int(t), true) + "Z"
	var plots := [
		{"idx": 0, "col": -1, "row": -3, "state": "growing", "crop_id": "seed_wheat", "planted_at": iso.call(now - 100), "ready_at": iso.call(now + 900)},
		{"idx": 1, "col": 0, "row": -3, "state": "growing", "crop_id": "seed_carrot", "planted_at": iso.call(now - 500), "ready_at": iso.call(now + 500)},
		{"idx": 2, "col": 1, "row": -3, "state": "growing", "crop_id": "seed_tomato", "planted_at": iso.call(now - 900), "ready_at": iso.call(now + 100)},
		{"idx": 3, "col": -1, "row": -2, "state": "ready", "crop_id": "seed_pumpkin"},
		{"idx": 4, "col": 0, "row": -2, "state": "ready", "crop_id": "seed_clashberry"},
		{"idx": 5, "col": 1, "row": -2, "state": "growing", "crop_id": "seed_wheat", "planted_at": iso.call(now - 600), "ready_at": iso.call(now + 400)},
		{"idx": 6, "col": 4, "row": 1, "state": "ready", "crop_id": "seed_wheat"},
		{"idx": 7, "col": 5, "row": 1, "state": "growing", "crop_id": "seed_wheat", "planted_at": iso.call(now - 850), "ready_at": iso.call(now + 150)},
	]
	var decor := [{"id": "d0", "item_id": "decor_fence", "x_dm": -80, "z_dm": -10, "yaw_deg": 90}, {"id": "d1", "item_id": "decor_bench", "x_dm": -80, "z_dm": 20, "yaw_deg": 0},
		{"id": "d2", "item_id": "decor_barrel", "x_dm": 80, "z_dm": -40, "yaw_deg": 0}, {"id": "d3", "item_id": "decor_lantern", "x_dm": 80, "z_dm": 0, "yaw_deg": 0},
		{"id": "d4", "item_id": "decor_windmill", "x_dm": -40, "z_dm": 80, "yaw_deg": 45}, {"id": "d5", "item_id": "decor_well", "x_dm": 40, "z_dm": 80, "yaw_deg": 0}]
	var resources := [{"idx": 0, "kind": "tree", "hits_left": 5}, {"idx": 1, "kind": "tree", "hits_left": 0, "respawn_at": iso.call(now + 3600)},
		{"idx": 2, "kind": "tree", "hits_left": 2}, {"idx": 3, "kind": "rock", "hits_left": 8}, {"idx": 4, "kind": "rock", "hits_left": 0, "respawn_at": iso.call(now + 600)}]
	var args := _parse_args()
	if args.has("crops"):
		# Crop gallery: the given seeds in a 6-wide grid of plots, all at `progress` (1 = ripe).
		plots = []
		var progress := float(args.get("progress", "1"))
		var seeds: PackedStringArray = str(args["crops"]).split(",")
		for i: int in seeds.size():
			var p := {"idx": i, "col": i % 6 - 3, "row": i / 6 + 1, "state": "ready", "crop_id": seeds[i]}
			if progress < 1.0:
				p.merge({"state": "growing", "planted_at": iso.call(now - 1000.0 * progress), "ready_at": iso.call(now + 1000.0 * (1.0 - progress))}, true)
			plots.append(p)
		decor = []
	var payload := {"full": true, "plots": plots, "decor": decor, "resources": resources, "level": 3, "now": iso.call(now),
		"expand": {"n": 0, "e": 1, "s": 0, "w": 0}}
	for grid: Node in [scene.find_child("PlotGrid", true, false), scene.find_child("DecorSlots", true, false), scene.find_child("Props", true, false), scene.find_child("IslandShape", true, false)]:
		if grid != null and grid.has_method("apply_world_state"):
			grid.apply_world_state(payload)
		elif grid != null and grid.has_method("apply_expand"):
			grid.apply_expand(payload["expand"])
	# Show the bed outline as if the character stood next to plot 5.
	var plot_grid: Node = scene.find_child("PlotGrid", true, false)
	if plot_grid != null and plot_grid.has_method("plot") and plot_grid.plot(5) != null:
		plot_grid.plot(5).set_highlighted(true)
	# A harvest burst on plot 3 and a planting puff on plot 5 so the effects show up in renders.
	var bridge: Node = root.get_node_or_null(^"/root/Bridge")
	if bridge != null:
		bridge.handle({"v": 1, "type": "play_fx", "payload": {"fx": "harvest", "target": {"plot_idx": 3}}})
		bridge.handle({"v": 1, "type": "play_fx", "payload": {"fx": "plant", "target": {"plot_idx": 5}}})


var _tile := TILE


func _run_pieces(names: Array, sheet: bool) -> void:
	_tile = int(_parse_args().get("tile", TILE))
	root.get_window().size = Vector2i(_tile, _tile)
	_build_stage()
	await _frames(2)
	var images: Array[Image] = []
	for name: Variant in names:
		var piece: Node = _load_piece(str(name))
		if piece == null:
			continue
		_stage.add_child(piece)
		_frame_camera(piece)
		await _frames(3)
		var image := root.get_viewport().get_texture().get_image()
		_stamp(image, str(name), piece)
		if sheet:
			images.append(image)
		else:
			image.save_png(_out + str(name).get_file().get_basename() + ".png")
		piece.queue_free()
		await _frames(1)
	if sheet:
		_save_sheet(images)
	print("snapshot: %d pieces -> %s" % [names.size(), _out])
	quit()


func _build_stage() -> void:
	_stage = Node3D.new()
	root.add_child(_stage)
	var env := WorldEnvironment.new()
	var e := Environment.new()
	e.background_mode = Environment.BG_COLOR
	e.background_color = Color(0.13, 0.16, 0.2)
	e.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	e.ambient_light_color = Color(0.7, 0.7, 0.75)
	env.environment = e
	_stage.add_child(env)
	var sun := DirectionalLight3D.new()
	sun.rotation_degrees = Vector3(-50, 30, 0)
	_stage.add_child(sun)
	var grid := MeshInstance3D.new()
	var plane := PlaneMesh.new()
	plane.size = Vector2(40, 40)
	grid.mesh = plane
	var mat := StandardMaterial3D.new()
	mat.albedo_color = Color(0.22, 0.26, 0.3)
	grid.material_override = mat
	_stage.add_child(grid)
	for axis: Array in [[Vector3.RIGHT, Color.RED], [Vector3.UP, Color.GREEN], [Vector3.BACK, Color.DODGER_BLUE]]:
		var bar := MeshInstance3D.new()
		var box := BoxMesh.new()
		box.size = Vector3(0.08, 0.08, 0.08) + (axis[0] as Vector3) * 2.0
		bar.mesh = box
		bar.position = (axis[0] as Vector3) * 1.0
		var m := StandardMaterial3D.new()
		m.albedo_color = axis[1]
		m.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
		bar.material_override = m
		_stage.add_child(bar)
	_camera = Camera3D.new()
	_camera.fov = 40.0
	_stage.add_child(_camera)
	_camera.make_current()


func _load_piece(name: String) -> Node:
	var scene := load(name if name.begins_with("res://") else PIECES_DIR + name + ".tscn") as PackedScene
	if scene == null:
		push_warning("snapshot: no piece " + name)
		return null
	return scene.instantiate()


## Merged world-space AABB of all meshes under `node`.
static func bounds(node: Node) -> AABB:
	var box := AABB()
	var first := true
	for mesh: Node in node.find_children("*", "MeshInstance3D", true, false):
		var b: AABB = (mesh as MeshInstance3D).global_transform * (mesh as MeshInstance3D).get_aabb()
		box = b if first else box.merge(b)
		first = false
	return box


func _frame_camera(piece: Node) -> void:
	var box := bounds(piece)
	var radius := maxf(box.size.length() * 0.5, 0.5)
	var centre := box.get_center()
	var dir := Vector3(1.0, 0.8, 1.2).normalized()
	_camera.position = centre + dir * radius * 2.6
	_camera.look_at(centre, Vector3.UP)


func _stamp(image: Image, name: String, piece: Node) -> void:
	# Yellow strip marks tile edges on sheets; bounds go to stdout so wrappers can be offset to y = 0.
	image.fill_rect(Rect2i(0, image.get_height() - 4, image.get_width(), 4), Color(0.9, 0.8, 0.2))
	var b := bounds(piece)
	print("piece %s min=%.2f,%.2f,%.2f size=%.2f,%.2f,%.2f" % [name.get_file().get_basename(), b.position.x, b.position.y, b.position.z, b.size.x, b.size.y, b.size.z])


## Contact sheets of COLUMNS x COLUMNS tiles: sheet_0.png, sheet_1.png, ... (row-major, order as printed).
func _save_sheet(images: Array[Image]) -> void:
	var per_sheet := COLUMNS * COLUMNS
	for start: int in range(0, images.size(), per_sheet):
		var chunk := images.slice(start, start + per_sheet)
		var rows := int(ceil(float(chunk.size()) / COLUMNS))
		var sheet := Image.create(COLUMNS * _tile, rows * _tile, false, chunk[0].get_format())
		for i: int in chunk.size():
			var img: Image = chunk[i]
			if img.get_format() != sheet.get_format():
				img.convert(sheet.get_format())
			sheet.blit_rect(img, Rect2i(0, 0, _tile, _tile), Vector2i((i % COLUMNS) * _tile, (i / COLUMNS) * _tile))
		sheet.save_png(_out + "sheet_%d.png" % (start / per_sheet))


func _save(name: String) -> void:
	root.get_viewport().get_texture().get_image().save_png(_out + name + ".png")
	print("snapshot %s: %d primitives, %d draw calls, %.1f MB video" % [name, Performance.get_monitor(Performance.RENDER_TOTAL_PRIMITIVES_IN_FRAME), Performance.get_monitor(Performance.RENDER_TOTAL_DRAW_CALLS_IN_FRAME), Performance.get_monitor(Performance.RENDER_VIDEO_MEM_USED) / 1048576.0])


func _frames(n: int) -> void:
	for i: int in n:
		await process_frame


static func _vec(text: String) -> Vector3:
	var p := text.split(",")
	return Vector3(float(p[0]), float(p[1]), float(p[2])) if p.size() == 3 else Vector3.ZERO
