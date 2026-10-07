extends SceneTree
## Огляд персонажа на базі 4,5 голови (ADR-032) у справжніх сценах гри: `PeerAvatar` у повних образах (тіло й статура,
## зачіска, вираз, борода, убір, окуляри, вбрання) і `Player` з інструментами в руці в кадрах дій. Масштаб, кольори
## (`CharacterLook`) і анімації — як у грі. Потрібне вікно (не --headless).
##   godot --path client --script res://tools/base45_sheet.gd -- --mode=looks|game|tools|tools_windup|faces|life|frames --out=C:/tmp/x.jpg [--zoom=3]
## looks — вісім образів зблизька; game — ті самі з ігрової камери (14 м, 45°, 1280 × 720); tools — інструменти в руці;
## faces — п'ять виразів, які обирає гравець; life — кліпи «життя» (ADR-035) у кульмінації з виразом обличчя на час кліпу;
## frames — довільні кадри `--frames=chop@0.07,idle@0` спереду й збоку (знахідки `tools/anim_audit.gd`).

const PEER := "res://scenes/player/PeerAvatar.tscn"
const PLAYER := "res://scenes/player/Player.tscn"
const LOOKS: Array[Dictionary] = [
	{"body": "body_m", "build": "build_avg", "skin": "#e2ab7c", "shirt": "#3f6fb5", "pants": "#26344f", "shoes": "#3a2f28", "hair_color": "#4a2e1c", "hair": "hair_short", "face": "face_smile"},
	{"body": "body_f", "build": "build_avg", "skin": "#c78b5c", "shirt": "#f0d98a", "pants": "#26344f", "shoes": "#1f1f22", "hair_color": "#1d1a18", "hair": "hair_long", "face": "face_joy", "outfit": "outfit_dress", "outfit_color": "#c2453b"},
	{"body": "body_m", "build": "build_sturdy", "skin": "#d9a878", "shirt": "#c2453b", "pants": "#6b5a3e", "shoes": "#3a2f28", "hair_color": "#7a4a22", "beard": "beard_full", "hat": "hat_straw", "outfit": "outfit_overalls", "outfit_color": "#3d5a80"},
	{"body": "body_f", "build": "build_slim", "skin": "#f1c9a5", "shirt": "#ffffff", "pants": "#3a3a40", "shoes": "#5a3a22", "hair_color": "#d9b45a", "hair": "hair_ponytail", "glasses": "glasses_round", "outfit": "outfit_jacket", "outfit_color": "#4f7f3f"},
	{"body": "body_m", "build": "build_slim", "skin": "#8d5a3b", "shirt": "#e6c34a", "pants": "#26344f", "shoes": "#1f1f22", "hair_color": "#1d1a18", "hair": "hair_spiky", "face": "face_cool", "outfit": "outfit_hoodie", "outfit_color": "#7a3d80"},
	{"body": "body_f", "build": "build_sturdy", "skin": "#d9a878", "shirt": "#5f8f4a", "pants": "#6b5a3e", "shoes": "#3a2f28", "hair_color": "#a33a22", "hair": "hair_bun", "outfit": "outfit_apron", "outfit_color": "#cfc6ac"},
	{"body": "body_m", "build": "build_avg", "skin": "#5c3a26", "shirt": "#3f6fb5", "pants": "#6b5a3e", "shoes": "#3a2f28", "hair_color": "#1d1a18", "hair": "hair_afro", "beard": "beard_moustache", "outfit": "outfit_vest", "outfit_color": "#7a5533"},
	{"body": "body_f", "build": "build_avg", "skin": "#e2ab7c", "shirt": "#e06fa4", "pants": "#26344f", "shoes": "#1f1f22", "hair_color": "#5a3a22", "hair": "hair_bob", "face": "face_surprised", "hat": "hat_flowers", "outfit": "outfit_skirt", "outfit_color": "#3d5a80"},
]
## Дія → (кліп, інструмент, момент удару, момент замаху, с) — як у `island_input.gd` (кліпи ADR-034).
const TOOL_POSES: Array = [["chop", "axe", 0.53, 0.33], ["mine", "pickaxe", 0.6, 0.4], ["dig", "spade", 0.47, 0.87], ["reap", "scythe", 0.67, 0.33],
	["hammer", "hammer", 0.47, 0.27], ["water", "can", 0.6, 0.33]]
const FACES: Array[String] = ["face_neutral", "face_smile", "face_joy", "face_cool", "face_surprised"]
## Кліп «життя» → частка тривалості, де жест найвиразніший.
const LIFE_POSES: Array = [["idle_look", 0.3], ["idle_stretch", 0.5], ["idle_scratch", 0.5], ["cheer", 0.5], ["shake_no", 0.4], ["wave", 0.45],
	["laugh", 0.4], ["shrug", 0.5], ["thumbs_up", 0.5], ["heart", 0.5], ["clap", 0.45]]

var _args: Dictionary = {}
var _frames := 0


func _init() -> void:
	for arg: String in OS.get_cmdline_user_args():
		var pair := arg.trim_prefix("--").split("=", true, 1)
		_args[pair[0]] = pair[1] if pair.size() > 1 else "true"


func _process(_delta: float) -> bool:
	_frames += 1
	if _frames == 2:
		_build(str(_args.get("mode", "looks")))
	elif _frames == 10:
		# Автолоад Quality вмикає glow на кожному середовищі: на світлому тлі він розмиває силуети.
		for node: Node in root.find_children("*", "WorldEnvironment", true, false):
			if (node as WorldEnvironment).environment != null:
				(node as WorldEnvironment).environment.glow_enabled = false
	elif _frames == 50:
		var out := str(_args.get("out", "user://base45.png"))
		var image := root.get_viewport().get_texture().get_image()
		_save(image, out)
		var zoom := int(_args.get("zoom", "1"))
		if zoom > 1:
			# Середина кадру, збільшена без згладжування: ті самі пікселі гри, лише крупніше.
			var size := image.get_size() / zoom
			var crop := image.get_region(Rect2i((image.get_size() - size) / 2, size))
			crop.resize(image.get_width(), image.get_height(), Image.INTERPOLATE_NEAREST)
			_save(crop, out.get_basename() + "_zoom." + out.get_extension())
		quit()
	return false


func _save(image: Image, out: String) -> void:
	if out.ends_with(".jpg"):
		image.save_jpg(out, 0.88)
	else:
		image.save_png(out)
	print("base45_sheet: saved ", out)


func _build(mode: String) -> void:
	var stats: Node = root.get_node_or_null(^"DebugStats")
	if stats != null and stats.has_method("set_enabled"):
		stats.call("set_enabled", false)
	var stage := Node3D.new()
	root.add_child(stage)
	_world(stage)
	match mode:
		"tools", "tools_windup":
			root.get_window().size = Vector2i(1800, 640)
			var top := _args.has("top")  # ракурс, близький до ігрового: 3/4 зверху, персонажі повернуті боком
			for i in TOOL_POSES.size():
				_tool_pose(stage, TOOL_POSES[i], Vector3(float(i) * 1.25 - 3.1, 0, 0), 3 if mode == "tools_windup" else 2, 55.0 if top else 0.0)
			if top:
				_camera(stage, Vector3(0.0, 5.5, 7.5), Vector3(0.0, 0.5, 0.0), 32.0)
			else:
				_camera(stage, Vector3(0.0, 1.2, 9.5), Vector3(0.0, 0.7, 0.0), 30.0)
		"life":
			root.get_window().size = Vector2i(2000, 640)
			for i in LIFE_POSES.size():
				_life_pose(stage, i, Vector3(float(i) * 1.0 - 5.0, 0, 0))
			_camera(stage, Vector3(0.0, 1.1, 8.2), Vector3(0.0, 0.8, 0.0), 28.0)
		"frames":
			# До чотирьох кадрів: кожен — пара «спереду, збоку», камера рівно вміщує ряд.
			var shots := str(_args.get("frames", "idle@0")).split(",")
			root.get_window().size = Vector2i(600 * shots.size(), 760)
			for i in shots.size():
				var parts := shots[i].split("@")
				var at := float(parts[1]) if parts.size() > 1 else 0.0
				var x := (float(i) - (shots.size() - 1) * 0.5) * 1.9
				_peer(stage, LOOKS[0], parts[0], at, Vector3(x - 0.42, 0, 0))
				_peer(stage, LOOKS[0], parts[0], at, Vector3(x + 0.42, 0, 0), 90.0)
			var aspect := 600.0 * shots.size() / 760.0
			var half := atan(tan(deg_to_rad(15.0)) * aspect)
			_camera(stage, Vector3(0.0, 0.75, (shots.size() * 0.95 + 0.3) / tan(half)), Vector3(0.0, 0.62, 0.0), 30.0)
		"faces":
			root.get_window().size = Vector2i(1500, 340)
			for i in FACES.size():
				_peer(stage, {"face": FACES[i], "hair": "hair_short"}, "idle", 0.0, Vector3(float(i) * 0.5 - 1.0, 0, 0))
			_camera(stage, Vector3(0.0, 1.27, 4.2), Vector3(0.0, 1.22, 0.0), 22.0, 0.62)
		_:
			root.get_window().size = Vector2i(1280, 720) if mode == "game" else Vector2i(1800, 760)
			for i in LOOKS.size():
				_peer(stage, LOOKS[i], "idle", 0.6, Vector3(float(i) * 0.95 - 3.325, 0, 0))
			if mode == "game":
				var pitch := deg_to_rad(45.0)
				_camera(stage, Vector3(0.0, 1.2 + 14.0 * sin(pitch), 14.0 * cos(pitch)), Vector3(0.0, 1.0, 0.0), 45.0)
			else:
				_camera(stage, Vector3(0.0, 1.25, 10.5), Vector3(0.0, 0.72, 0.0), 26.0)


func _peer(stage: Node3D, look: Dictionary, clip: String, at: float, pos: Vector3, yaw_deg: float = 0.0) -> void:
	var peer := (load(PEER) as PackedScene).instantiate() as PeerAvatar
	stage.add_child(peer)
	peer.apply({"id": "p", "nick": "", "x_dm": int(pos.x * 10.0), "z_dm": int(pos.z * 10.0), "yaw_deg": int(yaw_deg), "anim": "idle", "look": look}, true)
	_freeze(peer.find_child("AnimationPlayer", true, false) as AnimationPlayer, clip, at)


func _life_pose(stage: Node3D, i: int, pos: Vector3) -> void:
	var clip := String(LIFE_POSES[i][0])
	var lib := load("res://assets/character/character_anims.tres") as AnimationLibrary
	var length := lib.get_animation(clip).length if lib.has_animation(clip) else 1.0
	_peer(stage, LOOKS[i % LOOKS.size()], clip, length * float(LIFE_POSES[i][1]), pos)


func _tool_pose(stage: Node3D, pose: Array, pos: Vector3, moment: int, yaw_deg: float = 0.0) -> void:
	var player := (load(PLAYER) as PackedScene).instantiate() as Node3D
	stage.add_child(player)
	player.set_physics_process(false)
	player.set_process(false)
	player.position = pos
	(player.get_node("Model") as Node3D).rotation_degrees.y = yaw_deg
	CharacterLook.apply(player.get_node("Model"), LOOKS[0])
	var hand := player.find_child("RightHand", true, false) as HandTools
	if hand != null:
		hand.show_tool(String(pose[1]))
	_freeze(player.find_child("AnimationPlayer", true, false) as AnimationPlayer, String(pose[0]), float(pose[moment]))


func _freeze(player: AnimationPlayer, clip: String, at: float) -> void:
	if player == null or not player.has_animation(clip):
		return
	player.play(clip)
	player.seek(at, true)
	player.speed_scale = 0.0


func _camera(stage: Node3D, at: Vector3, target: Vector3, fov: float, ortho_size: float = 0.0) -> void:
	var camera := Camera3D.new()
	stage.add_child(camera)
	camera.position = at
	camera.look_at(target, Vector3.UP)
	if ortho_size > 0.0:
		camera.projection = Camera3D.PROJECTION_ORTHOGONAL
		camera.size = ortho_size
	else:
		camera.fov = fov
	camera.make_current()


func _world(stage: Node3D) -> void:
	var sun := DirectionalLight3D.new()
	sun.rotation_degrees = Vector3(-55.0, 35.0, 0.0)
	sun.shadow_enabled = true
	stage.add_child(sun)
	var env := WorldEnvironment.new()
	var e := Environment.new()
	e.background_mode = Environment.BG_COLOR
	e.background_color = Color(0.62, 0.78, 0.9)
	e.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	e.ambient_light_color = Color(0.8, 0.85, 0.8)
	e.ambient_light_energy = 0.45
	env.environment = e
	stage.add_child(env)
	var ground := MeshInstance3D.new()
	var plane := PlaneMesh.new()
	plane.size = Vector2(60.0, 60.0)
	ground.mesh = plane
	var grass := StandardMaterial3D.new()
	grass.albedo_color = Color(0.3, 0.47, 0.2)  # трава острова: перевіряємо, чи персонаж не губиться на ній
	ground.material_override = grass
	stage.add_child(ground)
