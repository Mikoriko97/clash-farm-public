extends SceneTree
## Вартість одного персонажа за відстанню від камери (ADR-028): трикутники, draw calls і обʼєкти в кадрі SubViewport 1280×720
## з fov 45 і тінями сонця — як у грі. Потрібне вікно (headless не малює), а також реальний GPU:
##   godot --path client --resolution 1280x720 --script res://tools/lod_probe.gd
## Друкує рядки `<вигляд> d=<м> prim=<трикутники> calls=<draw calls> objs=<обʼєкти>`; без LOD кожен рядок одного вигляду
## однаковий, з LOD трикутники падають зі зростанням відстані (тіло 10,6 тис. → 1,3 тис. на 120 м). Базова лінія — порожня сцена.

const LOOKS := {
	"bare": {"face": "face_smile"},
	"heavy": {"face": "face_smile", "hair": "hair_long", "hat": "hat_cowboy", "beard": "beard_full", "glasses": "glasses_round", "outfit": "outfit_jacket"},
	"heavy_f": {"body": "body_f", "face": "face_smile", "hair": "hair_bob", "hat": "hat_beanie", "glasses": "glasses_sun", "outfit": "outfit_dress"},
}
const DISTANCES := [0.0, 6.0, 15.0, 30.0, 60.0, 120.0]
## Кадрів очікування на вигляд: перший застосовує вигляд, далі камера й конвеєр встигають перерахувати LOD і тіні.
const SETTLE_FRAMES := 6

var _vp: SubViewport
var _avatar: PeerAvatar
var _cam: Camera3D
var _frames := 0
var _jobs: Array = []
var _job := 0
var _wait := 0
var _baseline: Array = [0, 0, 0]


func _info() -> Array:
	var rid := _vp.get_viewport_rid()
	var visible := RenderingServer.VIEWPORT_RENDER_INFO_TYPE_VISIBLE
	return [
		RenderingServer.viewport_get_render_info(rid, visible, RenderingServer.VIEWPORT_RENDER_INFO_PRIMITIVES_IN_FRAME),
		RenderingServer.viewport_get_render_info(rid, visible, RenderingServer.VIEWPORT_RENDER_INFO_DRAW_CALLS_IN_FRAME),
		RenderingServer.viewport_get_render_info(rid, visible, RenderingServer.VIEWPORT_RENDER_INFO_OBJECTS_IN_FRAME),
	]


func _build() -> void:
	_vp = SubViewport.new()
	_vp.size = Vector2i(1280, 720)
	_vp.render_target_update_mode = SubViewport.UPDATE_ALWAYS
	root.add_child(_vp)
	var sun := DirectionalLight3D.new()
	sun.rotation_degrees = Vector3(-50, 30, 0)
	sun.shadow_enabled = true
	_vp.add_child(sun)
	_avatar = (load("res://scenes/player/PeerAvatar.tscn") as PackedScene).instantiate() as PeerAvatar
	_vp.add_child(_avatar)
	_cam = Camera3D.new()
	_cam.fov = 45.0
	_cam.near = 0.2
	_cam.far = 4000.0
	_vp.add_child(_cam)
	_cam.position = Vector3(0.0, 1.4, 12.0)
	for look_name in LOOKS:
		for distance: float in DISTANCES:
			_jobs.append([look_name, distance])


func _process(_delta: float) -> bool:
	_frames += 1
	if _frames == 1:
		_build()
		return false
	if _job >= _jobs.size():
		quit()
		return true
	var job: Array = _jobs[_job]
	_wait += 1
	if _wait == 1:
		# персонаж на відстані d перед камерою (вона дивиться в −Z); d = 0 — порожня сцена
		_avatar.visible = float(job[1]) > 0.0
		_avatar.apply({"id": "p", "nick": "", "x_dm": 0, "z_dm": 0, "yaw_deg": 0, "anim": "idle", "look": LOOKS[job[0]]}, true)
		_avatar.position = Vector3(0.0, 0.0, _cam.position.z - float(job[1]))
		_cam.look_at(Vector3(0.0, 1.0, _cam.position.z - maxf(float(job[1]), 1.0)))
		(_avatar.get_node("%Nick") as Label3D).visible = false
	elif _wait == SETTLE_FRAMES:
		var info := _info()
		if float(job[1]) == 0.0:
			_baseline = info
		print("%s d=%.0f prim=%d calls=%d objs=%d" % [job[0], job[1], info[0] - _baseline[0], info[1] - _baseline[1], info[2] - _baseline[2]])
		_wait = 0
		_job += 1
	return false
