class_name WeatherAudio
extends Node
## Звук погоди (UX: звук, docs/UI.md): дві петлі дощу на шині `Ambient` — легка й сильна — змішуються за `Weather.rain_level()`
## (дощ ≈ 0,55: легка на повну, сильна чверть; гроза 1,0: сильна на повну, легка тиха), дві петлі вітру (легкий бриз і шквал) — за
## `Weather.wind_level()` (ясно 0,14 ледь чутний бриз, гроза 0,5 — шквал), а спалах грози (`storm_flash`) дає грім із затримкою
## (звук доходить пізніше за світло). Вночі (`set_daylight` від `DayCycle` через групу `NightLight.GROUP`) до ембієнту зони
## додаються цвіркуни, а зрідка й сова; дощ їх заглушає. Ранком (`set_hour` від `DayCycle` через групу `DayCycle.CLOCK_GROUP`, години від передсвітанку
## до пізнього ранку) співають птахи — так само лише в суху погоду. Шелест листя (`ambient_leaves`) іде за тією самою силою вітру, що й бриз зі шквалом,
## але залежить від зони: на материку (ліс) чутний навіть у штиль, на острові вдвічі тихіший. Під землею (зона печери) ні дощу, ні вітру, ні ночі, ні птахів. Дочірній вузол автолоаду `Audio`, що раз
## на чверть секунди шукає вузол погоди поточної зони (групу `weather`) — сцени зон змінюються цілком. Нічого не вирішує.

const LIGHT: AudioStream = preload("res://assets/audio/ambient_rain_light.wav")
const HEAVY: AudioStream = preload("res://assets/audio/ambient_rain_heavy.wav")
const BREEZE: AudioStream = preload("res://assets/audio/ambient_wind_breeze.wav")
const GALE: AudioStream = preload("res://assets/audio/ambient_wind_gale.wav")
const THUNDER: AudioStream = preload("res://assets/audio/lightning.wav")
const CRICKETS: AudioStream = preload("res://assets/audio/ambient_night.wav")
const OWL: AudioStream = preload("res://assets/audio/night_owl.wav")
const MORNING: AudioStream = preload("res://assets/audio/ambient_morning.wav")
const LEAVES: AudioStream = preload("res://assets/audio/ambient_leaves.wav")
const BUS_RAIN := "Ambient"
const BUS_THUNDER := "SFX"
const TICK_SEC := 0.25
## Секунд, за які рівень звуку дощу наздоганяє погоду (без різких стрибків при зміні стану).
const FOLLOW_SEC := 3.0
const SILENT_DB := -60.0
const LIGHT_DB := -2.0
const HEAVY_DB := -2.0
const BREEZE_DB := -4.0
const GALE_DB := -4.0
const THUNDER_DB := -9.0
const CRICKETS_DB := -3.0
const OWL_DB := -7.0
const MORNING_DB := -3.0
const LEAVES_DB := -4.0
## Скільки листя в зоні: материк — ліс, острів — кілька дерев, печера (і невідома зона) — жодного.
const LEAVES_ZONE: Dictionary = {"mainland": 1.0, "island": 0.5}
## Шелест у повний штиль, частка від повного (листя ворушиться й без помітного вітру).
const LEAVES_CALM := 0.15
## Пауза між гуками сови вночі, секунд.
const OWL_GAP := Vector2(35.0, 90.0)
## Години ранкового хору: наростає від передсвітанку (перша пара), спадає до пізнього ранку (друга).
const MORNING_IN := Vector2(4.0, 5.5)
const MORNING_OUT := Vector2(8.5, 10.5)
## Сила вітру в погоді (`wind`) від тиші до шквалу: нижче — повітря нерухоме, вище — повна сила.
const WIND_CALM := 0.05
const WIND_GALE := 0.5

var level := 0.0
var wind := 0.0
## Денне світло 0..1 (0 — ніч) від `DayCycle`; допоки сцена без циклу доби (печера), лишається останнім.
var daylight := 1.0
## Година доби 0..24 від `DayCycle`; допоки сцена без циклу (печера), лишається останньою.
var hour := 12.0
var _light: AudioStreamPlayer
var _heavy: AudioStreamPlayer
var _breeze: AudioStreamPlayer
var _gale: AudioStreamPlayer
var _thunder: AudioStreamPlayer
var _crickets: AudioStreamPlayer
var _owl: AudioStreamPlayer
var _morning: AudioStreamPlayer
var _leaves: AudioStreamPlayer
var _owl_wait := 40.0
var _weather: Node
var _timer := 0.0


func _ready() -> void:
	_light = _player(LIGHT, BUS_RAIN)
	_heavy = _player(HEAVY, BUS_RAIN)
	_breeze = _player(BREEZE, BUS_RAIN)
	_gale = _player(GALE, BUS_RAIN)
	_thunder = _player(THUNDER, BUS_THUNDER)
	_crickets = _player(CRICKETS, BUS_RAIN)
	_owl = _player(OWL, BUS_RAIN)
	_morning = _player(MORNING, BUS_RAIN)
	_leaves = _player(LEAVES, BUS_RAIN)
	add_to_group(NightLight.GROUP)  # `DayCycle` розсилає `set_daylight` усій групі
	add_to_group(DayCycle.CLOCK_GROUP)  # …і `set_hour`


## Викликає `DayCycle` щокадру (`call_group(NightLight.GROUP, "set_daylight", …)`).
func set_daylight(value: float) -> void:
	daylight = value


## Викликає `DayCycle` щокадру (`call_group(DayCycle.CLOCK_GROUP, "set_hour", …)`).
func set_hour(value: float) -> void:
	hour = value


## Зона, у якій зараз гра (`Audio.zone`); порожній рядок, поки зони ще нема (`str`, бо `String(null)` — помилка скрипта).
func _zone() -> String:
	return str(get_parent().get("zone"))


func _player(stream: AudioStream, bus: String) -> AudioStreamPlayer:
	var player := AudioStreamPlayer.new()
	player.stream = stream
	player.bus = bus
	player.volume_db = SILENT_DB
	add_child(player)
	return player


func _process(delta: float) -> void:
	_timer += delta
	if _timer < TICK_SEC:
		return
	var step := _timer
	_timer = 0.0
	_tick(step)


## Один крок змішування: `dt` секунд відтоді, як рахували востаннє.
func _tick(dt: float) -> void:
	_find_weather()
	var rain_target := 0.0
	var wind_target := 0.0
	if _weather != null and _zone() != "cave":
		rain_target = clampf(float(_weather.call("rain_level")), 0.0, 1.0)
		wind_target = clampf(inverse_lerp(WIND_CALM, WIND_GALE, float(_weather.call("wind_level"))), 0.0, 1.0)
	level = move_toward(level, rain_target, dt / FOLLOW_SEC)
	wind = move_toward(wind, wind_target, dt / FOLLOW_SEC)
	_mix(_light, light_weight(level), LIGHT_DB)
	_mix(_heavy, heavy_weight(level), HEAVY_DB)
	_mix(_breeze, breeze_weight(wind), BREEZE_DB)
	_mix(_gale, gale_weight(wind), GALE_DB)
	var night := night_weight(daylight) * dry_weight(level) if _zone() != "cave" else 0.0
	_mix(_crickets, night, CRICKETS_DB)
	_mix(_leaves, leaves_weight(wind) * float(LEAVES_ZONE.get(_zone(), 0.0)), LEAVES_DB)
	_mix(_morning, morning_weight(hour) * dry_weight(level) if _zone() != "cave" else 0.0, MORNING_DB)
	_owl_wait -= dt
	if _owl_wait <= 0.0:
		_owl_wait = randf_range(OWL_GAP.x, OWL_GAP.y)
		if night > 0.7 and not _owl.playing:
			_owl.volume_db = OWL_DB + randf_range(-3.0, 0.0)
			_owl.pitch_scale = randf_range(0.95, 1.05)
			_owl.play()


## Вага легкого дощу: на повну до рівня дощу (0,55), далі стихає до третини під грозою.
static func light_weight(rain: float) -> float:
	return clampf(rain / 0.55, 0.0, 1.0) * (1.0 - 0.7 * clampf((rain - 0.55) / 0.45, 0.0, 1.0))


## Вага сильного дощу: від 0,4 наростає до повної під грозою.
static func heavy_weight(rain: float) -> float:
	return clampf((rain - 0.4) / 0.6, 0.0, 1.0)


## Вага бризу (вітер 0..1 між тишею й шквалом): наростає до 0,6, далі поступається шквалу.
static func breeze_weight(strength: float) -> float:
	return clampf(strength / 0.6, 0.0, 1.0) * (1.0 - 0.6 * clampf((strength - 0.6) / 0.4, 0.0, 1.0))


## Вага шквалу: від 0,45 наростає до повної.
static func gale_weight(strength: float) -> float:
	return clampf((strength - 0.45) / 0.55, 0.0, 1.0)


## Вага ночі: 0 удень, наростає в сутінках і на повну, коли денного світла майже нема.
static func night_weight(light: float) -> float:
	return 1.0 - smoothstep(0.05, 0.4, light)


## Вага ранкового хору за годиною доби: 0 уночі й удень, на повну від світанку до початку ранку.
static func morning_weight(at_hour: float) -> float:
	return smoothstep(MORNING_IN.x, MORNING_IN.y, at_hour) * (1.0 - smoothstep(MORNING_OUT.x, MORNING_OUT.y, at_hour))


## Вага шелесту листя за силою вітру 0..1: штиль — `LEAVES_CALM`, далі плавно до повної на помірному вітрі (0,7); шквал його не глушить.
static func leaves_weight(strength: float) -> float:
	return LEAVES_CALM + (1.0 - LEAVES_CALM) * smoothstep(0.1, 0.7, strength)


## Множник «сухо»: цвіркуни замовкають, щойно починається дощ (повністю вже за рівня дощу 0,35).
static func dry_weight(rain: float) -> float:
	return 1.0 - clampf(rain / 0.35, 0.0, 1.0)


func _mix(player: AudioStreamPlayer, weight: float, base_db: float) -> void:
	if weight < 0.01:
		if player.playing:
			player.stop()
		player.volume_db = SILENT_DB
		return
	player.volume_db = base_db + linear_to_db(weight)
	if not player.playing:
		player.play()


func _find_weather() -> void:
	var found := get_tree().get_first_node_in_group("weather")
	if found == _weather:
		return
	_weather = found
	if _weather != null and _weather.has_signal("storm_flash"):
		_weather.connect("storm_flash", _on_flash)


## Грім слідом за спалахом: затримка й тембр різні, щоб серія не звучала однаково.
func _on_flash() -> void:
	if _zone() == "cave":
		return
	await get_tree().create_timer(randf_range(0.6, 2.6)).timeout
	_thunder.volume_db = THUNDER_DB + randf_range(-4.0, 0.0)
	_thunder.pitch_scale = randf_range(0.82, 1.0)
	_thunder.play()
