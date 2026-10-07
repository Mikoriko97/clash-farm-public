class_name AnimProbe
extends RefCounted
## Вимірювання кліпів персонажа на базі 4,5 голови (аудит анімацій 2026-10-03): кліп у `SAMPLES` кадрах на справжньому скелеті
## сцени, звіт — у сантиметрах сцени (× масштаб `Model`). Підошва нижче землі чи обидві стопи над землею там, де персонаж стоїть;
## коліно назад чи лікоть уперед або всередину; кисть, пальці чи інструмент у тулубі, стегні чи голові (проксі — еліпси з вершин мешу
## тіла за домінантною кісткою: грубо, точна перевірка по мешу — `client/blender/characters/base45_check.py` під час запікання);
## ковзання стопи, що стоїть; швидкість стопи в ходьбі й бігу проти швидкості гри. Користуються `tools/anim_audit.gd` і
## `tests/test_anim_audit.gd`.

const SAMPLES := 30
## Персонаж не стоїть (стрибок, плавання, лежить, сидить, кермує): «над землею» тут не помилка.
const OFF_GROUND: Array[String] = ["jump", "jump_begin", "jump_land", "fall", "roll", "swim_forward", "swim_idle", "lie_down", "sleeping",
	"sitting_idle", "drive", "cheer", "run_forward", "sprint_forward"]
## Кліпи на місці: стопа, що стоїть, не має їхати.
const MOVING: Dictionary = {"walk_forward": 2.2, "run_forward": 4.8, "sprint_forward": 0.0, "walk_back": 0.0, "walk_left": 0.0,
	"walk_right": 0.0, "crouch_forward": 0.0, "swim_forward": 0.0, "push": 0.0, "roll": 0.0}
const TORSO_BONES: Array[String] = ["Hips", "Spine", "Chest", "LeftUpperLeg", "RightUpperLeg", "Head"]
## Кліп дії → інструмент у руці (як `FxClips`): його держак теж не має проходити крізь тіло.
const TOOL_OF: Dictionary = {"chop": "axe", "mine": "pickaxe", "dig": "spade", "reap": "scythe", "hammer": "hammer", "water": "can"}
## Точки інструмента ближче за це (одиниці моделі) до кулака — у руці, а не в тілі.
const GRIP_CLEAR := 0.11
const BANDS := 8
## Півосі еліпса — цей перцентиль відстаней вершин смуги: пахви й плечі не роздувають груди.
const PERCENTILE := 0.85

var _sk: Skeleton3D
var _anim: AnimationPlayer
var _scale := 1.0
var _proxies: Dictionary = {}  # кістка → [{y0, y1, cx, cz, rx, rz}]
var _rest: Dictionary = {}
var _tools: Dictionary = {}  # інструмент → точки в системі кістки RightHand


## `player` — сцена `Player` у дереві (вимкнена, щоб скрипт руху не перемикав кліпи).
func _init(player: Node3D) -> void:
	_sk = player.find_child("Skeleton3D", true, false) as Skeleton3D
	_anim = player.get_node("AnimationPlayer") as AnimationPlayer
	_scale = (player.get_node("Model") as Node3D).scale.y
	_build_rest()


func _b(name: String) -> int:
	return _sk.find_bone(name)


func _pos(name: String) -> Vector3:
	return _sk.get_bone_global_pose(_b(name)).origin


func _pose(name: String) -> Transform3D:
	return _sk.get_bone_global_pose(_b(name))


## Спокій: напрямок «уперед» у кістках тазу й грудей, точки підошви в кістках стопи й пальців, еліпси тіла з мешу.
func _build_rest() -> void:
	_sk.reset_bone_poses()
	for side: String in ["Left", "Right"]:
		var foot := _sk.get_bone_global_rest(_b(side + "Foot"))
		var toes := _sk.get_bone_global_rest(_b(side + "Toes"))
		var x := foot.origin.x
		_rest[side + "heel"] = foot.affine_inverse() * Vector3(x, 0.0, foot.origin.z - 0.04)
		_rest[side + "ball"] = toes.affine_inverse() * Vector3(x, 0.0, toes.origin.z)
		_rest[side + "tip"] = toes.affine_inverse() * Vector3(x, 0.0, toes.origin.z + 0.07)
	for bone: String in ["Hips", "Chest"]:
		_rest[bone + "fwd"] = _sk.get_bone_global_rest(_b(bone)).basis.inverse() * Vector3(0, 0, 1)
		_rest[bone + "left"] = _sk.get_bone_global_rest(_b(bone)).basis.inverse() * Vector3(1, 0, 0)
	_build_proxies()
	_build_tools()


## Вершини кожного інструмента `HandTools` (до 200) у системі кістки кисті: інструмент рухається разом із нею.
func _build_tools() -> void:
	var hand := _sk.find_child("RightHand", false, false)
	if hand == null:
		return
	var inv := _sk.get_bone_global_rest(_b("RightHand")).affine_inverse()
	for mesh: Node in hand.get_children():
		if not mesh is MeshInstance3D or (mesh as MeshInstance3D).mesh == null:
			continue
		var verts: PackedVector3Array = (mesh as MeshInstance3D).mesh.surface_get_arrays(0)[Mesh.ARRAY_VERTEX]
		var step := maxi(1, verts.size() / 200)
		var points: Array[Vector3] = []
		for i in range(0, verts.size(), step):
			points.append(inv * verts[i])
		_tools[String(mesh.name)] = points


func _build_proxies() -> void:
	var body := _sk.find_child("Base45_Body", false, false) as MeshInstance3D
	var arrays := body.mesh.surface_get_arrays(0)
	var verts: PackedVector3Array = arrays[Mesh.ARRAY_VERTEX]
	var bones: PackedInt32Array = arrays[Mesh.ARRAY_BONES]
	var weights: PackedFloat32Array = arrays[Mesh.ARRAY_WEIGHTS]
	var per := bones.size() / verts.size()
	var points: Dictionary = {}
	for i in verts.size():
		var best := 0
		for k in per:
			if weights[i * per + k] > weights[i * per + best]:
				best = k
		var bone_name := String(body.skin.get_bind_name(bones[i * per + best])) if body.skin.get_bind_name(bones[i * per + best]) != &"" \
			else _sk.get_bone_name(body.skin.get_bind_bone(bones[i * per + best]))
		if bone_name in TORSO_BONES:
			var local := _sk.get_bone_global_rest(_b(bone_name)).affine_inverse() * verts[i]
			(points.get_or_add(bone_name, []) as Array).append(local)
	for bone: String in points:
		_proxies[bone] = _bands(points[bone] as Array)


## Еліпс у площині XZ кістки на кожну смугу висоти: центр і півосі (95-й перцентиль, щоб окремі вершини не роздували).
func _bands(points: Array) -> Array:
	var ys: Array = points.map(func(p: Vector3) -> float: return p.y)
	var lo: float = ys.min()
	var hi: float = ys.max()
	var out: Array = []
	for band in BANDS:
		var y0 := lo + (hi - lo) * band / BANDS
		var y1 := lo + (hi - lo) * (band + 1) / BANDS
		var inside: Array = points.filter(func(p: Vector3) -> bool: return p.y >= y0 and p.y <= y1)
		if inside.size() < 8:
			continue
		var cx: float = inside.map(func(p: Vector3) -> float: return p.x).reduce(func(a: float, b: float) -> float: return a + b) / inside.size()
		var cz: float = inside.map(func(p: Vector3) -> float: return p.z).reduce(func(a: float, b: float) -> float: return a + b) / inside.size()
		var dx: Array = inside.map(func(p: Vector3) -> float: return absf(p.x - cx))
		var dz: Array = inside.map(func(p: Vector3) -> float: return absf(p.z - cz))
		dx.sort()
		dz.sort()
		out.append({"y0": y0, "y1": y1, "cx": cx, "cz": cz, "rx": dx[int(dx.size() * PERCENTILE)], "rz": dz[int(dz.size() * PERCENTILE)]})
	return out


## Глибина точки в тілі (одиниці моделі): 0 — зовні.
func _depth(point: Vector3, bone: String) -> float:
	var local := _pose(bone).affine_inverse() * point
	for band: Dictionary in _proxies.get(bone, []):
		if local.y < band["y0"] or local.y > band["y1"]:
			continue
		var qx := (local.x - float(band["cx"])) / maxf(band["rx"], 0.001)
		var qz := (local.z - float(band["cz"])) / maxf(band["rz"], 0.001)
		var q := sqrt(qx * qx + qz * qz)
		return maxf(0.0, (1.0 - q) * minf(band["rx"], band["rz"]))
	return 0.0


func audit(clip: String) -> Dictionary:
	var length := _anim.get_animation(clip).length
	var worst := {"clip": clip, "pen": 0.0, "pen_t": 0.0, "float": 0.0, "float_t": 0.0, "knee": 1.0, "knee_t": 0.0, "elbow": 0.0,
		"elbow_t": 0.0, "body": 0.0, "body_t": 0.0, "body_where": "", "head": 0.0, "head_t": 0.0, "slide": 0.0, "speed": 0.0, "cross": 9.0,
		"tool": 0.0, "tool_t": 0.0, "tool_where": ""}
	var planted: Dictionary = {}
	var planted_speed: Array = []
	var prev: Dictionary = {}
	for s in SAMPLES + 1:
		var t := length * s / SAMPLES
		_anim.play(clip)
		_anim.seek(t, true)
		_feet(worst, t, clip, planted, prev, planted_speed)
		_joints(worst, t)
		_contacts(worst, t)
		_tool_contacts(worst, t, str(TOOL_OF.get(clip, "")))
	var speed: float = MOVING.get(clip, -1.0)
	if speed > 0.0 and not planted_speed.is_empty():
		worst["speed"] = planted_speed.reduce(func(a: float, b: float) -> float: return a + b) / planted_speed.size() / speed
	return worst


func _sole(side: String) -> Array[Vector3]:
	var foot := _pose(side + "Foot")
	var toes := _pose(side + "Toes")
	return [foot * (_rest[side + "heel"] as Vector3), toes * (_rest[side + "ball"] as Vector3), toes * (_rest[side + "tip"] as Vector3)]


func _feet(worst: Dictionary, t: float, clip: String, planted: Dictionary, prev: Dictionary, speeds: Array) -> void:
	var lowest := INF
	for side: String in ["Left", "Right"]:
		var sole := _sole(side)
		var low := minf(sole[0].y, minf(sole[1].y, sole[2].y))
		lowest = minf(lowest, low)
		var ball: Vector3 = sole[1]
		# Ковзання — лише всередині одного торкання: крок (стопа піднялась і стала деінде) — не ковзання.
		if ball.y < 0.02 and not MOVING.has(clip):
			if planted.has(side):
				var d := Vector2(ball.x, ball.z).distance_to(planted[side] as Vector2) * _scale * 100.0
				worst["slide"] = maxf(worst["slide"], d)
			else:
				planted[side] = Vector2(ball.x, ball.z)
		elif planted.has(side):
			planted.erase(side)
		if ball.y < 0.02 and MOVING.has(clip) and prev.has(side) and prev.has("t") and t > float(prev["t"]):
			speeds.append((float((prev[side] as Vector3).z) - ball.z) * _scale / (t - float(prev["t"])))
		prev[side] = ball
	prev["t"] = t
	var pen := -lowest * _scale * 100.0
	if pen > worst["pen"]:
		worst["pen"] = pen
		worst["pen_t"] = t
	if not clip in OFF_GROUND and lowest * _scale * 100.0 > worst["float"]:
		worst["float"] = lowest * _scale * 100.0
		worst["float_t"] = t


## Коліно має дивитися вперед (−1 — назад), лікоть — не вперед і не в тулуб (1 — так і є).
func _joints(worst: Dictionary, t: float) -> void:
	var fwd := (_pose("Hips").basis * (_rest["Hipsfwd"] as Vector3)).normalized()
	var chest_fwd := (_pose("Chest").basis * (_rest["Chestfwd"] as Vector3)).normalized()
	var chest_left := (_pose("Chest").basis * (_rest["Chestleft"] as Vector3)).normalized()
	for side: String in ["Left", "Right"]:
		var knee := _bend(_pos(side + "UpperLeg"), _pos(side + "LowerLeg"), _pos(side + "Foot"))
		if knee != Vector3.ZERO and knee.dot(fwd) < worst["knee"]:
			worst["knee"] = knee.dot(fwd)
			worst["knee_t"] = t
		var elbow := _bend(_pos(side + "UpperArm"), _pos(side + "LowerArm"), _pos(side + "Hand"))
		var inward := -chest_left if side == "Left" else chest_left
		var bad := maxf(elbow.dot(chest_fwd), elbow.dot(inward))
		if elbow != Vector3.ZERO and bad > worst["elbow"]:
			worst["elbow"] = bad
			worst["elbow_t"] = t
	var knees := Vector2(_pos("LeftLowerLeg").x - _pos("RightLowerLeg").x, _pos("LeftLowerLeg").z - _pos("RightLowerLeg").z).length()
	worst["cross"] = minf(worst["cross"], knees * _scale * 100.0)


## Напрям згину суглоба (одиничний, від лінії «початок — кінець» до суглоба); нуль — кінцівка майже пряма.
func _bend(a: Vector3, j: Vector3, b: Vector3) -> Vector3:
	var u := (a - j).normalized()
	var v := (b - j).normalized()
	if rad_to_deg(u.angle_to(v)) > 160.0:
		return Vector3.ZERO
	var line := (b - a).normalized()
	var off := (j - a) - line * (j - a).dot(line)
	return off.normalized() if off.length() > 0.001 else Vector3.ZERO


func _contacts(worst: Dictionary, t: float) -> void:
	for side: String in ["Left", "Right"]:
		var points: Array[Vector3] = [_pos(side + "Hand"), _pos(side + "MiddleProximal"), _pos(side + "MiddleDistal"), _pos(side + "ThumbDistal"),
			(_pos(side + "LowerArm") + _pos(side + "Hand")) * 0.5]
		for p: Vector3 in points:
			for bone: String in ["Hips", "Spine", "Chest", "LeftUpperLeg", "RightUpperLeg"]:
				var d := _depth(p, bone) * _scale * 100.0
				if d > worst["body"]:
					worst["body"] = d
					worst["body_t"] = t
					worst["body_where"] = "%s→%s" % [side, bone]
			var h := _depth(p, "Head") * _scale * 100.0
			if h > worst["head"]:
				worst["head"] = h
				worst["head_t"] = t


func _tool_contacts(worst: Dictionary, t: float, tool: String) -> void:
	if tool == "" or not _tools.has(tool):
		return
	var hand := _pose("RightHand")
	var fists: Array[Vector3] = [_pos("RightMiddleProximal"), _pos("LeftMiddleProximal")]
	for local: Vector3 in _tools[tool]:
		var p := hand * local
		if p.distance_to(fists[0]) < GRIP_CLEAR or p.distance_to(fists[1]) < GRIP_CLEAR:
			continue
		for bone: String in ["Hips", "Spine", "Chest", "LeftUpperLeg", "RightUpperLeg", "Head"]:
			var d := _depth(p, bone) * _scale * 100.0
			if d > worst["tool"]:
				worst["tool"] = d
				worst["tool_t"] = t
				worst["tool_where"] = bone


## Що виходить за межі (порожньо — кліп чистий).
static func flags(w: Dictionary) -> Array[String]:
	var flags: Array[String] = []
	if w["pen"] > 2.0: flags.append("PEN %.1f@%.2f" % [w["pen"], w["pen_t"]])
	if w["float"] > 3.0: flags.append("FLOAT %.1f@%.2f" % [w["float"], w["float_t"]])
	if w["knee"] < -0.3: flags.append("KNEE %.2f@%.2f" % [w["knee"], w["knee_t"]])
	if w["elbow"] > 0.75: flags.append("ELBOW %.2f@%.2f" % [w["elbow"], w["elbow_t"]])
	if w["body"] > 1.5: flags.append("BODY %.1f %s@%.2f" % [w["body"], w["body_where"], w["body_t"]])
	if w["head"] > 1.5: flags.append("HEAD %.1f@%.2f" % [w["head"], w["head_t"]])
	if w["slide"] > 3.0: flags.append("SLIDE %.1f" % w["slide"])
	if w["speed"] > 0.0 and absf(w["speed"] - 1.0) > 0.2: flags.append("SPEED %.2f" % w["speed"])
	if w["cross"] < 6.0: flags.append("CROSS %.1f" % w["cross"])
	if w["tool"] > 1.5: flags.append("TOOL %.1f %s@%.2f" % [w["tool"], w["tool_where"], w["tool_t"]])
	return flags
