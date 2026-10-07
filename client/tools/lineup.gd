extends SceneTree
## Шеренга для перевірки пропорцій (перевірка контенту 2026-10-02): персонаж гри і моделі з каталогу
## (`docs/models/catalog.tsv`, коди F002, B136…) стоять в один ряд на землі, ортографічна камера — тож
## висоти порівнюються чесно. Під кожною моделлю — код і габарити. Потрібне вікно (не --headless).
##   godot --path client --script res://tools/lineup.gd -- --codes=F002,B136,F079 --out=C:/tmp/ --name=decor_small

const GAP := 0.7
const WINDOW := Vector2i(1800, 760)
const PLAYER := "res://scenes/player/Player.tscn"

var _args: Dictionary = {}
var _keys: Dictionary = {}
var _packs: Dictionary = {}
var _frames := 0


func _init() -> void:
	for arg: String in OS.get_cmdline_user_args():
		var pair := arg.trim_prefix("--").split("=", true, 1)
		_args[pair[0]] = pair[1] if pair.size() > 1 else "true"
	_keys = _read_catalog()


func _process(_delta: float) -> bool:
	_frames += 1
	if _frames == 2:
		_build()
	elif _frames == 10:
		# Автолоад Quality вмикає glow на кожному середовищі (відкладено): на світлому тлі він розмиває моделі серпанком.
		for node: Node in root.find_children("*", "WorldEnvironment", true, false):
			if (node as WorldEnvironment).environment != null:
				(node as WorldEnvironment).environment.glow_enabled = false
	elif _frames == 40:
		var out := str(_args.get("out", "user://")) + str(_args.get("name", "lineup")) + ".png"
		root.get_viewport().get_texture().get_image().save_png(out)
		print("lineup: saved ", out)
		quit()
	return false


## code -> {key, size, name} з каталогу (TSV: code, category, name, key, tris, size_m, …).
func _read_catalog() -> Dictionary:
	var path := ProjectSettings.globalize_path("res://").path_join("../docs/models/catalog.tsv")
	var keys := {}
	var file := FileAccess.open(path, FileAccess.READ)
	if file == null:
		push_error("lineup: no catalog at " + path)
		return keys
	file.get_line()
	while not file.eof_reached():
		var cols := file.get_line().split("\t")
		if cols.size() >= 6:
			keys[cols[0]] = {"name": cols[2], "key": cols[3], "size": cols[5]}
	return keys


func _build() -> void:
	root.get_window().size = WINDOW
	var stage := Node3D.new()
	root.add_child(stage)
	_light(stage)
	var x := 0.0
	var stats: Node = root.get_node_or_null(^"DebugStats")
	if stats != null and stats.has_method("set_enabled"):
		stats.call("set_enabled", false)
	var player := (load(PLAYER) as PackedScene).instantiate() as Node3D
	stage.add_child(player)
	# Після `_ready` (до нього Godot сам вмикає обробку): без фізики персонаж не падає крізь землю без колайдера.
	player.set_physics_process(false)
	player.position = Vector3(x, 0.0, 0.0)
	_label(stage, Vector3(x, 2.05, 0.6), "Персонаж\n1,8 м")
	x += 0.6 + GAP
	var top := 2.0
	var n := 0
	for code: String in str(_args.get("codes", "")).split(",", false):
		var node := _load(code.strip_edges())
		if node == null:
			continue
		stage.add_child(node)
		var box := _box(node)
		node.position += Vector3(x - box.position.x, -box.position.y, -box.get_center().z)
		var entry: Dictionary = _keys.get(code, {})
		n += 1
		var lift := 0.25 if n % 2 == 1 else 0.85  # підпис над моделлю; сусідні на різній висоті, щоб не налазили
		_label(stage, Vector3(x + box.size.x * 0.5, box.size.y + lift, box.size.z * 0.5 + 0.3), "%s %s\n%s м" % [code, entry.get("name", ""), entry.get("size", "")])
		top = maxf(top, box.size.y)
		x += box.size.x + GAP
	_ground(stage, x)
	var camera := Camera3D.new()
	camera.projection = Camera3D.PROJECTION_ORTHOGONAL
	var aspect := float(WINDOW.x) / float(WINDOW.y)
	camera.size = maxf(top + 2.4, (x + 1.0) / aspect)
	stage.add_child(camera)
	camera.position = Vector3(x * 0.5 - 0.3, top * 0.5 + 0.9, 30.0)
	camera.rotation_degrees = Vector3(-6.0, 0.0, 0.0)
	camera.make_current()
	# Автолоади гри можуть мати своє середовище з туманом: у шерензі туман лише заважає порівнювати.
	for node: Node in root.find_children("*", "WorldEnvironment", true, false):
		var we := node as WorldEnvironment
		if we.environment != null:
			we.environment.fog_enabled = false
			we.environment.volumetric_fog_enabled = false
	camera.attributes = null


func _load(code: String) -> Node3D:
	var entry: Dictionary = _keys.get(code, {})
	var key := str(entry.get("key", ""))
	if key.is_empty():
		push_warning("lineup: unknown code " + code)
		return null
	if not key.contains("#"):
		var scene := load(key) as PackedScene
		return scene.instantiate() as Node3D if scene != null else null
	var parts := key.split("#")
	if not _packs.has(parts[0]):
		_packs[parts[0]] = (load(parts[0]) as PackedScene).instantiate()
	var pack: Node = _packs[parts[0]]
	var node := pack.get_node_or_null(NodePath(parts[1])) as Node3D
	if node == null:
		push_warning("lineup: no %s in %s" % [parts[1], parts[0]])
		return null
	var copy := node.duplicate() as Node3D
	copy.transform = Transform3D(node.transform.basis, Vector3.ZERO)
	return copy


## Межі найближчого рівня деталізації: обʼєкт паку сам буває MeshInstance3D, далекі LOD і «лише тіні» не рахуються (і ховаються).
func _box(node: Node3D) -> AABB:
	var box := AABB()
	var first := true
	var all: Array = node.find_children("*", "MeshInstance3D", true, false)
	if node is MeshInstance3D:
		all.append(node)
	for mesh: Node in all:
		var m := mesh as MeshInstance3D
		if m.visibility_range_begin > 0.0:
			m.visible = false
			continue
		m.visibility_range_end = 0.0
		if not m.visible or m.mesh == null or m.cast_shadow == GeometryInstance3D.SHADOW_CASTING_SETTING_SHADOWS_ONLY:
			continue
		var b := _relative(node, m) * m.get_aabb()
		box = b if first else box.merge(b)
		first = false
	return box


## Перетворення сітки відносно кореня моделі (вузли ще можуть бути поза деревом).
func _relative(root_node: Node3D, child: Node3D) -> Transform3D:
	var t := Transform3D.IDENTITY
	var n: Node = child
	while n != null and n != root_node:
		if n is Node3D:
			t = (n as Node3D).transform * t
		n = n.get_parent()
	return root_node.transform * t


func _label(stage: Node3D, at: Vector3, text: String) -> void:
	var label := Label3D.new()
	label.text = text
	label.font_size = 44
	label.pixel_size = clampf(0.004 * float(_args.get("scale", "1.0")), 0.002, 0.03)
	label.modulate = Color(0.05, 0.05, 0.05)
	label.outline_size = 0
	label.no_depth_test = true
	label.vertical_alignment = VERTICAL_ALIGNMENT_BOTTOM
	stage.add_child(label)
	label.position = at


func _ground(stage: Node3D, width: float) -> void:
	var plane := MeshInstance3D.new()
	var mesh := PlaneMesh.new()
	mesh.size = Vector2(width + 4.0, 12.0)
	plane.mesh = mesh
	var mat := StandardMaterial3D.new()
	mat.albedo_color = Color(0.82, 0.84, 0.78)
	plane.material_override = mat
	stage.add_child(plane)
	plane.position = Vector3(width * 0.5, 0.0, 0.0)
	for i in int(width) + 2:  # мітки кожні 1 м уздовж ряду
		var tick := MeshInstance3D.new()
		var bar := BoxMesh.new()
		bar.size = Vector3(0.02, 0.005, 0.3)
		tick.mesh = bar
		var tm := StandardMaterial3D.new()
		tm.albedo_color = Color(0.45, 0.47, 0.42)
		tick.material_override = tm
		stage.add_child(tick)
		tick.position = Vector3(float(i), 0.003, 1.6)


func _light(stage: Node3D) -> void:
	var sun := DirectionalLight3D.new()
	sun.rotation_degrees = Vector3(-50.0, 30.0, 0.0)
	stage.add_child(sun)
	var env := WorldEnvironment.new()
	var e := Environment.new()
	e.background_mode = Environment.BG_COLOR
	e.background_color = Color(0.95, 0.96, 0.93)
	e.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	e.ambient_light_color = Color(0.85, 0.85, 0.85)
	e.ambient_light_energy = 0.7
	env.environment = e
	stage.add_child(env)
