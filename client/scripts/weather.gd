class_name Weather
extends Node
## Shared weather picked deterministically from the wall clock (same for every
## player; the server will own it later) and blended over a few seconds.
## States: clear, cloudy, fog, rain, storm. Drives the sky shader, the sun through
## DayCycle, fog density, grass wind, rain particles and lightning flashes.
## `lightning_struck` is the hook for future world events (server-decided).

enum State { CLEAR, CLOUDY, FOG, RAIN, STORM }

## Full-strength opacity of drops and splash rings (Rain.tscn materials).
const RAIN_ALPHA := 0.55
const SPLASH_ALPHA := 0.45

signal state_changed(state: State)
signal lightning_struck(direction: Vector3)
## A storm flash of the weather itself (not a server strike on a tree, which has its own `play_fx`): thunder follows.
signal storm_flash

@export var day_cycle: DayCycle
@export var environment: WorldEnvironment
## Rain emitter (scenes/weather/Rain.tscn) kept above `follow`.
@export var rain: CPUParticles3D
## Пилок, листя, світлячки (scenes/weather/Ambient.tscn): гаснуть у дощ.
@export var ambient: AmbientParticles
@export var follow: Node3D
## Grass material whose `wind_strength` follows the weather.
@export var grass_material: ShaderMaterial
## Island ground material (materials/island_ground.tres): `wetness` follows the rain.
@export var ground_material: ShaderMaterial
## Minutes one weather state lasts before the next roll.
@export_range(1.0, 120.0) var period_min: float = 12.0
@export var base_fog_density: float = 0.0012
@export_range(0.5, 10.0) var blend_seconds: float = 4.0
@export var lightning_min_gap: float = 5.0
@export var lightning_max_gap: float = 14.0

## Per-state targets: clouds 0..1, storm (sky darkening) 0..1, sun dim, fog multiplier, rain 0..1, wind.
const TARGETS: Dictionary = {
	State.CLEAR: {"clouds": 0.0, "storm": 0.0, "sun": 1.0, "fog": 1.0, "rain": 0.0, "wind": 0.14, "grey": 0.0, "wet": 0.0},
	State.CLOUDY: {"clouds": 0.55, "storm": 0.15, "sun": 0.7, "fog": 1.6, "rain": 0.0, "wind": 0.2, "grey": 0.35, "wet": 0.0},
	State.FOG: {"clouds": 0.5, "storm": 0.1, "sun": 0.55, "fog": 22.0, "rain": 0.0, "wind": 0.08, "grey": 0.9, "wet": 0.25},
	State.RAIN: {"clouds": 0.85, "storm": 0.6, "sun": 0.45, "fog": 3.5, "rain": 0.55, "wind": 0.28, "grey": 0.7, "wet": 0.8},
	State.STORM: {"clouds": 1.0, "storm": 1.0, "sun": 0.3, "fog": 5.0, "rain": 1.0, "wind": 0.5, "grey": 0.85, "wet": 1.0},
}

## Nodes that need the weather find it through this group (ResourceGrid for lightning).
const GROUP := "weather"

var state: State = State.CLEAR
## State the server put in `world_state.weather`; -1 until the first snapshot
## (then the local schedule below is only a fallback for headless runs and dev).
var _server_state: int = -1
var _current: Dictionary = TARGETS[State.CLEAR].duplicate()
var _forced: bool = false
var _flash: float = 0.0
var _next_strike: float = 8.0
var _rng := RandomNumberGenerator.new()


func _ready() -> void:
	_rng.seed = 12345
	add_to_group(GROUP)
	if rain != null:
		rain.physics_interpolation_mode = Node.PHYSICS_INTERPOLATION_MODE_OFF
	var bridge: Node = get_node_or_null(^"/root/Bridge")
	if bridge != null:
		bridge.message_received.connect(_on_bridge_message)


func _on_bridge_message(type: String, payload: Dictionary) -> void:
	if type == "world_state" and payload.has("weather"):
		apply_server(payload["weather"])


## `{ state, until }` from the server snapshot; malformed input is ignored (rules/godot.md).
func apply_server(raw: Variant) -> void:
	if typeof(raw) != TYPE_DICTIONARY or typeof((raw as Dictionary).get("state")) != TYPE_STRING:
		push_warning("Weather: bad weather payload")
		return
	var name := str((raw as Dictionary)["state"]).to_lower()
	if name not in ["clear", "cloudy", "fog", "rain", "storm"]:
		push_warning("Weather: unknown state %s" % name)
		return
	_server_state = state_from_name(name)


## Dev override first, then the server, then the local fallback schedule.
func effective_state() -> State:
	if _forced:
		return state
	if _server_state >= 0:
		return _server_state as State
	return _scheduled_state()


func _process(delta: float) -> void:
	if not _forced:
		var scheduled := effective_state()
		if scheduled != state:
			state = scheduled
			state_changed.emit(state)
	var target: Dictionary = TARGETS[state]
	var step := delta / blend_seconds
	for key: String in target:
		_current[key] = move_toward(float(_current[key]), float(target[key]), step * maxf(absf(float(target[key]) - float(_current[key])), 0.05) * 2.0)
	_tick_lightning(delta)
	_apply()
	if rain != null and follow != null:
		rain.global_position = follow.global_position + Vector3(0.0, 12.0, 0.0)


## Fixes the weather (dev menu, tools/snapshot.gd); the blend still animates.
func force(new_state: State, instant: bool = false) -> void:
	_forced = true
	if new_state != state:
		state = new_state
		state_changed.emit(state)
	if instant:
		_current = TARGETS[state].duplicate()
		_apply()


## Back to the shared schedule.
func auto() -> void:
	_forced = false


## One lightning flash now somewhere in the sky (dev menu, ambient storm strikes).
func strike() -> void:
	_strike_dir(Vector3(_rng.randf_range(-0.6, 0.6), 0.7, _rng.randf_range(-0.6, 0.6)).normalized())
	storm_flash.emit()


## Smoothed wind strength (clear ≈ 0.14, rain 0.28, storm 0.5): the wind loops of `WeatherAudio` follow it.
func wind_level() -> float:
	return float(_current["wind"])


## Smoothed rain strength 0..1 (rain state ≈ 0.55, storm 1.0): the rain loops of `WeatherAudio` follow it.
func rain_level() -> float:
	return float(_current["rain"])


## A flash from the sky above `target` as seen from the character (server strike on a tree).
func strike_at(target: Vector3) -> void:
	var from := follow.global_position if follow != null else Vector3.ZERO
	var flat := target - from
	flat.y = 0.0
	var dir := Vector3.UP
	if flat.length() > 0.1:
		dir = (flat.normalized() * 0.7 + Vector3.UP * 0.7).normalized()
	_strike_dir(dir)


func _strike_dir(dir: Vector3) -> void:
	_flash = 1.0
	var sky := _sky_material()
	if sky != null:
		sky.set_shader_parameter("lightning_dir", dir)
	lightning_struck.emit(dir)


static func state_from_name(name: String) -> State:
	match name.to_lower():
		"storm":
			return State.STORM
		"rain":
			return State.RAIN
		"fog":
			return State.FOG
		"cloudy":
			return State.CLOUDY
		_:
			return State.CLEAR


static func state_name(s: State) -> String:
	return ["clear", "cloudy", "fog", "rain", "storm"][s]


## Deterministic roll per period: 40 % clear, 22 % cloudy, 10 % fog, 18 % rain, 10 % storm.
func _scheduled_state() -> State:
	var slot := int(Time.get_unix_time_from_system() / (period_min * 60.0))
	var roll := float(hash(slot) % 1000) / 1000.0
	if roll < 0.10:
		return State.STORM
	if roll < 0.28:
		return State.RAIN
	if roll < 0.38:
		return State.FOG
	if roll < 0.60:
		return State.CLOUDY
	return State.CLEAR


func _tick_lightning(delta: float) -> void:
	_flash = maxf(_flash - delta * 3.5, 0.0)
	if state != State.STORM or float(_current["storm"]) < 0.8:
		return
	_next_strike -= delta
	if _next_strike <= 0.0:
		_next_strike = _rng.randf_range(lightning_min_gap, lightning_max_gap)
		strike()


func _sky_material() -> ShaderMaterial:
	if environment == null or environment.environment == null or environment.environment.sky == null:
		return null
	return environment.environment.sky.sky_material as ShaderMaterial


func _apply() -> void:
	var c := _current
	if day_cycle != null:
		day_cycle.set_weather_dim(float(c["sun"]) + _flash * 1.5)
		day_cycle.set_weather_grey(float(c["grey"]))
	if environment != null and environment.environment != null:
		environment.environment.fog_density = base_fog_density * float(c["fog"])
	var sky := _sky_material()
	if sky != null:
		sky.set_shader_parameter("cloud_coverage", lerpf(0.2, 0.8, float(c["clouds"])))
		sky.set_shader_parameter("cloud_density", lerpf(0.32, 0.8, float(c["clouds"])))
		sky.set_shader_parameter("storm", float(c["storm"]))
		sky.set_shader_parameter("lightning", _flash * 2.0)
	if rain != null:
		var amount := clampf(float(c["rain"]), 0.0, 1.0)
		# Drops fade in with the blend instead of the emitter resizing (a resize restarts every
		# particle, which reads as the rain starting in jerks); the emitter keeps its full amount.
		rain.emitting = amount > 0.02
		_set_alpha(rain, RAIN_ALPHA * amount)
	if grass_material != null:
		grass_material.set_shader_parameter("wind_strength", float(c["wind"]))
	if ambient != null:
		ambient.set_rain(float(c["rain"]))
	if ground_material != null:
		ground_material.set_shader_parameter("wetness", float(c["wet"]))
	var splash := rain.get_node_or_null(^"Splash") as CPUParticles3D if rain != null else null
	if splash != null:
		var amount := clampf(float(c["rain"]), 0.0, 1.0)
		splash.emitting = amount > 0.15
		_set_alpha(splash, SPLASH_ALPHA * amount)


## Opacity of a CPUParticles3D whose mesh carries a StandardMaterial3D (Rain.tscn).
static func _set_alpha(emitter: CPUParticles3D, alpha: float) -> void:
	var material := emitter.mesh.material as StandardMaterial3D if emitter.mesh != null else null
	if material != null:
		material.albedo_color.a = alpha
