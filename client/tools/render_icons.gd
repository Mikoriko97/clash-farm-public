extends SceneTree
## Іконки предметів інвентаря (UX етап 3, 2026-09-24): кожен предмет — його модель у грі
## або модель з `blender/crops/make_items.py` — знімається в окремому прозорому вікні з
## однаковим світлом і ракурсом (3/4 зверху-спереду), кадрується за габаритами й
## зберігається PNG 256² у `--out`. Атлас і пакетики насіння збирає `tools/pack_icons.py`.
##   godot --path client --script res://tools/render_icons.gd -- --out=<dir> [--only=id,id]
## Потрібне вікно (headless не рендерить).

const SIZE := 256
const FOV := 24.0
## Ракурс за замовчуванням: поворот навколо Y і нахил камери вниз, градуси.
const YAW := 28.0
const PITCH := 30.0
## Окремі ракурси там, де «перед» моделі дивиться не туди, куди за замовчуванням:
## (поворот, нахил, крен кадру) у градусах; крен кладе довгі речі по діагоналі.
const VIEW := {
	"tool_axe": Vector3(0.0, 12.0, -40.0),
	"tool_pickaxe": Vector3(0.0, 12.0, -40.0),
	"decor_fence": Vector3(20.0, 22.0, 0.0),
	"decor_bench": Vector3(25.0, 26.0, 0.0),
	"seed_packet": Vector3(12.0, 14.0, 0.0),
	"permit_scroll": Vector3(10.0, 36.0, 0.0),
	"plot": Vector3(30.0, 42.0, 0.0),
}

## Предмет-послуга без власної моделі: одна частина з бібліотеки (решту вузлів ховаємо). Квиток зміни вигляду (0029,
## ADR-038) — солом'яний капелюх бази: «новий образ» видно навіть на 30 px.
const PART := {
	"restyle_ticket": ["res://assets/character/base45/head_parts.glb", "hat_straw"],
}

var _viewport: SubViewport
var _stage: Node3D
var _camera: Camera3D
var _queue: Array = []
var _current := ""
var _wait := 0
var _out := ""


func _init() -> void:
	var args := _args()
	_out = str(args.get("out", "user://icons"))
	DirAccess.make_dir_recursive_absolute(_out)
	var only: PackedStringArray = str(args.get("only", "")).split(",", false)
	for entry: Array in _items():
		if only.is_empty() or only.has(entry[0]):
			_queue.append(entry)
	_build_studio()


func _args() -> Dictionary:
	var out := {}
	for arg: String in OS.get_cmdline_user_args():
		var pair := arg.trim_prefix("--").split("=", true, 1)
		out[pair[0]] = pair[1] if pair.size() > 1 else true
	return out


## [id, scene path]: models of the game for placed things, make_items.py for the rest.
func _items() -> Array:
	var list: Array = []
	var items := DirAccess.get_files_at("res://assets/items")
	for file: String in items:
		if file.ends_with(".glb"):
			list.append([file.get_basename(), "res://assets/items/" + file])
	var decor: Node = (load("res://scenes/island/DecorItem.tscn") as PackedScene).instantiate()
	for id: String in decor.get("pieces"):
		list.append([id, (decor.get("pieces")[id] as PackedScene).resource_path])
	decor.free()
	list.append(["tool_axe", "res://assets/tools/AxeV1.fbx"])
	list.append(["tool_pickaxe", "res://assets/tools/PickAxeV1.fbx"])
	list.append(["plot", "res://assets/farm_pack/pieces/field_b.tscn"])
	for id: String in PART:
		list.append([id, PART[id][0]])
	return list


func _build_studio() -> void:
	_viewport = SubViewport.new()
	_viewport.size = Vector2i(SIZE, SIZE)
	_viewport.transparent_bg = true
	_viewport.own_world_3d = true
	_viewport.msaa_3d = Viewport.MSAA_4X
	_viewport.render_target_update_mode = SubViewport.UPDATE_ALWAYS
	root.add_child(_viewport)
	var env := Environment.new()
	env.background_mode = Environment.BG_CLEAR_COLOR
	env.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	env.ambient_light_color = Color(0.86, 0.9, 1.0)
	env.ambient_light_energy = 0.75
	env.tonemap_mode = Environment.TONE_MAPPER_AGX
	var world := WorldEnvironment.new()
	world.environment = env
	_viewport.add_child(world)
	var key := DirectionalLight3D.new()
	key.rotation_degrees = Vector3(-50.0, -35.0, 0.0)
	key.light_energy = 1.25
	key.shadow_enabled = false
	_viewport.add_child(key)
	var rim := DirectionalLight3D.new()
	rim.rotation_degrees = Vector3(-20.0, 150.0, 0.0)
	rim.light_energy = 0.45
	_viewport.add_child(rim)
	_camera = Camera3D.new()
	_camera.fov = FOV
	_viewport.add_child(_camera)
	_stage = Node3D.new()
	_viewport.add_child(_stage)


func _process(_delta: float) -> bool:
	if _wait > 0:
		_wait -= 1
		if _wait == 0:
			_save()
		return false
	if _queue.is_empty():
		print("render_icons: done -> ", _out)
		quit(0)
		return true
	var entry: Array = _queue.pop_front()
	_show(str(entry[0]), str(entry[1]))
	return false


func _show(id: String, path: String) -> void:
	for child: Node in _stage.get_children():
		child.free()
	_current = id
	var scene := load(path) as PackedScene
	if scene == null:
		push_warning("render_icons: cannot load %s" % path)
		return
	var node := scene.instantiate() as Node3D
	_stage.add_child(node)
	if PART.has(id):
		for m: Node in node.find_children("*", "MeshInstance3D", true, false):
			(m as MeshInstance3D).visible = String(m.name) == String(PART[id][1])
	_frame(node, VIEW.get(id, Vector3(YAW, PITCH, 0.0)))
	_wait = 4


## Camera on the model's bounding sphere at the given yaw/pitch, looking at its centre;
## roll turns the frame around the view axis.
func _frame(node: Node3D, view: Vector3) -> void:
	var box := AABB()
	var first := true
	for m: Node in node.find_children("*", "MeshInstance3D", true, false):
		var mi := m as MeshInstance3D
		if not mi.visible or mi.mesh == null:
			continue
		var b := mi.global_transform * mi.get_aabb()
		box = b if first else box.merge(b)
		first = false
	var centre := box.get_center()
	var radius := maxf(box.size.length() * 0.5, 0.01)
	var yaw := deg_to_rad(view.x)
	var pitch := deg_to_rad(view.y)
	var dir := Vector3(sin(yaw) * cos(pitch), sin(pitch), cos(yaw) * cos(pitch))
	var distance := radius / sin(deg_to_rad(FOV) * 0.5) * 1.02
	_camera.global_position = centre + dir * distance
	_camera.look_at(centre, Vector3.UP)
	_camera.rotate_object_local(Vector3.FORWARD, deg_to_rad(view.z))
	_camera.near = distance * 0.05
	_camera.far = distance * 4.0


func _save() -> void:
	var image := _viewport.get_texture().get_image()
	var path := _out.path_join(_current + ".png")
	image.save_png(path)
	print("  ", _current)
