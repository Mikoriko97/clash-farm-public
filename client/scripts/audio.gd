extends Node
## Звук світу (UX етап 2, docs/UI.md «Звук»): дві шини (`SFX`, `Ambient`) над Master,
## кліпи дій за `play_fx` з мосту, ембієнт зони з кросфейдом при `set_zone`, гучність з
## меню React (`set_volume`) у `user://settings.cfg` [audio]. При старті й на `session_ready`
## відповідає `volume_changed`, щоб web виставив ті самі значення для своїх UI-звуків.
## Автолоад `Audio`. Звуки UI (кліки, монети, рівень) грає сам React — тут лише світ; кроки — компонент `Footsteps` у сценах персонажів,
## дощ і грім — дочірній `WeatherAudio`.

const SETTINGS := "user://settings.cfg"
const BUS_SFX := "SFX"
const BUS_AMBIENT := "Ambient"
const POOL_SIZE := 6
const FADE_SEC := 1.6
const DEFAULT_SFX := 80
const DEFAULT_AMBIENT := 60
## fx з `play_fx` → кліп; `water` = лиття води (полив), `fertilize` = добриво, `dig` = лопата (посадка грядки), `interact` = клік,
## `door_open`/`door_close` = двері будинку, `door_bell` = дзвіночок міської будівлі (їх викликають `HouseNode` і `Interaction`).
const CLIPS: Dictionary = {
	"plant": preload("res://assets/audio/plant.wav"),
	"harvest": preload("res://assets/audio/harvest.wav"),
	"fertilize": preload("res://assets/audio/fertilize.wav"),
	"water": preload("res://assets/audio/water.wav"),
	"place": preload("res://assets/audio/place.wav"),
	"chop": preload("res://assets/audio/chop.wav"),
	"mine": preload("res://assets/audio/mine.wav"),
	"gather": preload("res://assets/audio/gather.wav"),
	"lightning": preload("res://assets/audio/lightning.wav"),
	"sleep": preload("res://assets/audio/sleep.wav"),
	"sit": preload("res://assets/audio/sit.wav"),
	"dig": preload("res://assets/audio/dig.wav"),
	"interact": preload("res://assets/audio/interact.wav"),
	"door_open": preload("res://assets/audio/door_open.wav"),
	"door_close": preload("res://assets/audio/door_close.wav"),
	"door_bell": preload("res://assets/audio/door_bell.wav"),
}
const AMBIENT: Dictionary = {
	"island": preload("res://assets/audio/ambient_island.wav"),
	"mainland": preload("res://assets/audio/ambient_mainland.wav"),
	"cave": preload("res://assets/audio/ambient_cave.wav"),
}

var sfx_volume: int = DEFAULT_SFX
var ambient_volume: int = DEFAULT_AMBIENT
var zone: String = ""
var _pool: Array[AudioStreamPlayer] = []
var _next_player := 0
var _ambient: Array[AudioStreamPlayer] = []
var _live := 0
var _fade_left := 0.0


func _ready() -> void:
	_setup_buses()
	_load_settings()
	for i in POOL_SIZE:
		var p := AudioStreamPlayer.new()
		p.bus = BUS_SFX
		add_child(p)
		_pool.append(p)
	for i in 2:
		var a := AudioStreamPlayer.new()
		a.bus = BUS_AMBIENT
		add_child(a)
		_ambient.append(a)
	_apply_volumes()
	add_child(WeatherAudio.new())  # дощ і грім: стежить за погодою поточної зони (weather_audio.gd)
	var bridge: Node = get_node_or_null(^"/root/Bridge")
	if bridge != null:
		bridge.message_received.connect(_on_bridge_message)
	set_process(false)
	# React дізнається збережену гучність одразу, як і рівень графіки (Quality).
	_announce.call_deferred()


## Шини створюються в коді, щоб не тримати default_bus_layout.tres: два регулятори в меню.
func _setup_buses() -> void:
	for name in [BUS_SFX, BUS_AMBIENT]:
		if AudioServer.get_bus_index(name) != -1:
			continue
		var idx := AudioServer.bus_count
		AudioServer.add_bus(idx)
		AudioServer.set_bus_name(idx, name)
		AudioServer.set_bus_send(idx, &"Master")


func _load_settings() -> void:
	if _headless():
		return
	var cfg := ConfigFile.new()
	if cfg.load(SETTINGS) != OK:
		return
	sfx_volume = _clamp_volume(cfg.get_value("audio", "sfx", DEFAULT_SFX))
	ambient_volume = _clamp_volume(cfg.get_value("audio", "ambient", DEFAULT_AMBIENT))


## Гучність з меню: 0…100 на кожну шину; 0 = шина вимкнена. Файл налаштувань читається
## перед записом, щоб не стерти [graphics] (Quality робить так само).
func set_volume(sfx: int, ambient: int) -> void:
	sfx_volume = _clamp_volume(sfx)
	ambient_volume = _clamp_volume(ambient)
	_apply_volumes()
	if not _headless():
		var cfg := ConfigFile.new()
		cfg.load(SETTINGS)
		cfg.set_value("audio", "sfx", sfx_volume)
		cfg.set_value("audio", "ambient", ambient_volume)
		cfg.save(SETTINGS)
	_announce()


func _apply_volumes() -> void:
	_apply_bus(BUS_SFX, sfx_volume)
	_apply_bus(BUS_AMBIENT, ambient_volume)


func _apply_bus(name: String, volume: int) -> void:
	var idx := AudioServer.get_bus_index(name)
	if idx == -1:
		return
	AudioServer.set_bus_mute(idx, volume == 0)
	# Перцептивна крива: 50 % повзунка ≈ −12 дБ, не половина амплітуди.
	var linear := pow(float(volume) / 100.0, 2.0)
	AudioServer.set_bus_volume_db(idx, linear_to_db(maxf(linear, 0.0001)))


## Один кліп дії; невелика випадкова висота, щоб серія однакових ударів не звучала як луп.
func play_fx(fx: String) -> bool:
	var stream: AudioStream = CLIPS.get(fx)
	if stream == null or _pool.is_empty():
		return false
	var p := _pool[_next_player]
	_next_player = (_next_player + 1) % _pool.size()
	p.stream = stream
	p.pitch_scale = randf_range(0.94, 1.06)
	p.play()
	return true


## Ембієнт зони: нова петля стартує одразу, стара гасне за FADE_SEC (кросфейд на двох плеєрах).
func set_zone(new_zone: String) -> void:
	if new_zone == zone or not AMBIENT.has(new_zone) or _ambient.size() < 2:
		return
	zone = new_zone
	var stream := AMBIENT[new_zone] as AudioStreamWAV
	# Імпорт уже ставить петлю на весь файл (edit/loop_mode=1); страховка на випадок нового ассета.
	if stream.loop_mode == AudioStreamWAV.LOOP_DISABLED:
		stream.loop_mode = AudioStreamWAV.LOOP_FORWARD
		stream.loop_end = int(stream.get_length() * stream.mix_rate)
	_live = 1 - _live
	var incoming := _ambient[_live]
	incoming.stream = stream
	incoming.volume_db = -60.0
	incoming.play()
	_fade_left = FADE_SEC
	set_process(true)


func _process(delta: float) -> void:
	_fade_left = maxf(_fade_left - delta, 0.0)
	var t := 1.0 - _fade_left / FADE_SEC
	_ambient[_live].volume_db = linear_to_db(maxf(t, 0.001))
	var outgoing := _ambient[1 - _live]
	outgoing.volume_db = linear_to_db(maxf(1.0 - t, 0.001))
	if _fade_left == 0.0:
		outgoing.stop()
		_ambient[_live].volume_db = 0.0
		set_process(false)


func _on_bridge_message(type: String, payload: Dictionary) -> void:
	match type:
		"play_fx":
			play_fx(str(payload.get("fx", "")))
		"set_zone":
			set_zone(str(payload.get("zone", "")))
		"set_volume":
			set_volume(int(payload.get("sfx", sfx_volume)), int(payload.get("ambient", ambient_volume)))
		"session_ready":
			_announce()
			if zone == "":
				set_zone("island")


func _announce() -> void:
	var bridge: Node = get_node_or_null(^"/root/Bridge")
	if bridge != null:
		bridge.call("send", "volume_changed", {"sfx": sfx_volume, "ambient": ambient_volume})


static func _clamp_volume(value: Variant) -> int:
	return clampi(int(value), 0, 100)


static func _headless() -> bool:
	return DisplayServer.get_name() == "headless"
