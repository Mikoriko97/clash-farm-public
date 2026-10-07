extends Node
## Рівні якості (автолоад `Quality`, етап 2 docs/PLAN.md): low / medium / high
## або auto. Рівень множить густоту трави (`GrassGpu.apply_quality`), дальність
## тіней сонця (вузли групи `quality_sun`, відносно значення зі сцени), вмикає
## MSAA і glow. Вибір приходить з меню React мостом `set_quality { tier }`,
## відповідь — `quality_changed { tier, effective }`; зберігається у
## `user://settings.cfg` (локальна перевага клієнта, не стан гри). У режимі auto
## після того, як зона готова (`ZoneSettle` кличе `arm()`), міряється p95 кадру за
## `SAMPLE_SECS` (після `WARM_SECS` дотягування флори): понад `slow_ms` — рівень нижче,
## до двох разів, щоб слабкий ноутбук чи телефон сам зійшов на середній або низький.
## Вікна в секундах, не в кадрах: на 8 FPS 360 кадрів — це 45 с лагів на кожен крок.
## Кадри завантаження до `arm()` і кадр повернення з фонової вкладки (≥ `RESUME_MS`,
## браузер тоді не малює зовсім) у вибірку не йдуть; усі інші — йдуть, навіть дуже
## повільні: саме вони й кажуть, що рівень зависокий.

const GROUP := "quality"
const SUN_GROUP := "quality_sun"
## `view`: скільки чанків материка тримати навколо персонажа (LOD 1), скільки з них
## ближніх з колайдерами й повною флорою (LOD 0), з якої відстані звільняти; `grass_m` —
## радіус трави на GPU. Рішення власника 2026-09-16: ПК і ноутбуки бачать значно далі
## (≈ 121 чанк, до ~1 ГБ памʼяті в браузері), телефони й планшети лишаються на «середній».
## Трава — квадратна сітка зі стороною 2·радіус·√(24·grass): 2026-09-23 «висока» мала
## 345 тис. травинок (60 м × 1.0), «середня» 100 тис. — у браузері й на телефонах це лагало;
## тепер ≈ 155 тис. / 75 тис. / 27 тис., дальні травинки однаково менші за піксель.
const TIERS := {
	"low": {"grass": 0.35, "msaa": Viewport.MSAA_DISABLED, "glow": false, "shadow": 0.4, "view": {"load": 2, "lod0": 1, "unload": 4, "grass_m": 28.0}},
	"medium": {"grass": 0.6, "msaa": Viewport.MSAA_DISABLED, "glow": true, "shadow": 0.65, "view": {"load": 3, "lod0": 1, "unload": 5, "grass_m": 36.0}},
	"high": {"grass": 0.8, "msaa": Viewport.MSAA_2X, "glow": true, "shadow": 1.0, "view": {"load": 5, "lod0": 2, "unload": 7, "grass_m": 45.0}},
}
const ORDER := ["low", "medium", "high"]
const SETTINGS := "user://settings.cfg"
## Секунд пропустити після готовності зони (дотягування флори) і секунд міряти;
## менше `MIN_SAMPLES` кадрів — вибірка ще не рішення.
const WARM_SECS := 1.5
const SAMPLE_SECS := 3.0
const MIN_SAMPLES := 20
## Кадр після повернення з фонової вкладки: браузер тоді не малював — це не продуктивність.
const RESUME_MS := 1000.0
@export var slow_ms: float = 22.0

signal changed(effective: String)

var setting := "auto"
var effective := "high"
var _elapsed := 0.0
var _sample: PackedFloat32Array = PackedFloat32Array()
var _steps_down := 0
## Міряти лише після `arm()`: до готовності зони кадри — це завантаження.
var _armed := false


func _ready() -> void:
	# Headless runs (tests, tools) neither read nor keep the player's choice: each run starts clean.
	var cfg := ConfigFile.new()
	if not _headless() and cfg.load(SETTINGS) == OK:
		var t := str(cfg.get_value("graphics", "tier", "auto"))
		if t == "auto" or TIERS.has(t):
			setting = t
	effective = auto_start() if setting == "auto" else setting
	var bridge: Node = get_node_or_null(^"/root/Bridge")
	if bridge != null:
		bridge.message_received.connect(_on_bridge_message)
	get_tree().node_added.connect(_on_node_added)
	set_process(false)
	# Меню React дізнається збережений рівень одразу, а не після першого вибору.
	apply.call_deferred()


## Телефон чи планшет (web-збірка на Android/iOS або сенсорний екран) стартує в auto з
## «середньої», ПК і ноутбук — з «високої»; далі auto лише знижує за p95 кадру.
static func is_mobile() -> bool:
	return OS.has_feature("web_android") or OS.has_feature("web_ios") or OS.has_feature("android") or OS.has_feature("ios") or DisplayServer.is_touchscreen_available()


## Headless (тести, інструменти) тримає «середній» — базу, від якої рахують тести.
static func _headless() -> bool:
	return DisplayServer.get_name() == "headless"


static func auto_start() -> String:
	if _headless():
		return "medium"
	return "medium" if is_mobile() else "high"


func current() -> Dictionary:
	return TIERS[effective]


func grass_scale() -> float:
	return float(current()["grass"])


## Вибір гравця: "auto" або назва рівня.
func set_tier(tier: String) -> void:
	if tier != "auto" and not TIERS.has(tier):
		push_warning("Quality: unknown tier " + tier)
		return
	setting = tier
	effective = auto_start() if tier == "auto" else tier
	_elapsed = 0.0
	_sample.clear()
	_steps_down = 0
	set_process(tier == "auto" and _armed)
	if not _headless():
		# Файл спільний з Audio ([audio]): читаємо перед записом, щоб не стерти чужу секцію.
		var cfg := ConfigFile.new()
		cfg.load(SETTINGS)
		cfg.set_value("graphics", "tier", setting)
		cfg.save(SETTINGS)
	apply()


func apply() -> void:
	var q := current()
	if get_tree() == null:
		return
	var vp := get_viewport()
	if vp != null:
		vp.msaa_3d = int(q["msaa"]) as Viewport.MSAA
	var env: Environment = vp.world_3d.environment if vp != null and vp.world_3d != null else null
	if env != null:
		env.glow_enabled = bool(q["glow"])
	for sun: Node in get_tree().get_nodes_in_group(SUN_GROUP):
		_apply_sun(sun as DirectionalLight3D, float(q["shadow"]))
	get_tree().call_group(GROUP, "apply_quality", q)
	changed.emit(effective)
	var bridge: Node = get_node_or_null(^"/root/Bridge")
	if bridge != null:
		bridge.call("send", "quality_changed", {"tier": setting, "effective": effective})


func _apply_sun(sun: DirectionalLight3D, factor: float) -> void:
	if sun == null:
		return
	if not sun.has_meta("shadow_m"):
		sun.set_meta("shadow_m", sun.directional_shadow_max_distance)
	sun.directional_shadow_max_distance = float(sun.get_meta("shadow_m")) * factor


## Нова сцена (острів ↔ материк): її сонце й середовище отримують поточний рівень.
func _on_node_added(node: Node) -> void:
	if node.is_in_group(SUN_GROUP):
		_apply_sun.call_deferred(node as DirectionalLight3D, float(current()["shadow"]))
	elif node is WorldEnvironment:
		(func() -> void:
			var env := (node as WorldEnvironment).environment
			if env != null:
				env.glow_enabled = bool(current()["glow"])
			if get_viewport() != null:
				get_viewport().msaa_3d = int(current()["msaa"]) as Viewport.MSAA).call_deferred()


func _on_bridge_message(type: String, payload: Dictionary) -> void:
	if type == "set_quality":
		set_tier(str(payload.get("tier", "auto")))
	elif type == "session_ready":
		# React могла завантажитись після старту Godot: повторюємо збережений рівень, як Audio гучність.
		apply()


## Зона готова (кличе `ZoneSettle`): у режимі auto почати заміри заново — материк
## важчий за острів, тож кожна нова зона має свій шанс знизити рівень.
func arm() -> void:
	_armed = true
	_elapsed = 0.0
	_sample.clear()
	set_process(setting == "auto" and _steps_down < 2 and effective != "low")


## Вкладка чи вікно втратили фокус: браузер гальмує кадри, вибірка вже нечесна.
func _notification(what: int) -> void:
	if what == NOTIFICATION_APPLICATION_FOCUS_OUT:
		_sample.clear()
		_elapsed = 0.0


## Auto: p95 кадру за SAMPLE_SECS після WARM_SECS; повільно — на рівень нижче.
func _process(delta: float) -> void:
	if not _armed:
		return
	var ms := delta * 1000.0
	if ms >= RESUME_MS:
		_sample.clear()
		_elapsed = 0.0
		return
	_elapsed += delta
	if _elapsed < WARM_SECS:
		return
	_sample.append(ms)
	if _elapsed < WARM_SECS + SAMPLE_SECS or _sample.size() < MIN_SAMPLES:
		return
	var sorted := _sample.duplicate()
	sorted.sort()
	var p95 := sorted[int(floor(sorted.size() * 0.95))]
	_sample.clear()
	_elapsed = 0.0
	if p95 > slow_ms and _steps_down < 2 and effective != "low":
		_steps_down += 1
		effective = ORDER[maxi(ORDER.find(effective) - 1, 0)]
		print("[quality] p95 %.1f ms > %.0f: auto -> %s" % [p95, slow_ms, effective])
		apply()
	elif _steps_down >= 2 or p95 <= slow_ms:
		set_process(false)
