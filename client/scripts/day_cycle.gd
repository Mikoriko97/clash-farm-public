class_name DayCycle
extends Node
## Shared day/night: the game day is `day_length_min` real minutes long and derived
## from the wall clock, so every player sees the same time of day. Drives the sun
## light, the sky shader (materials/farm_sky.tres) and the fog colour.
## Visual only — nothing here affects gameplay (rules/godot.md).

## Nodes in this group get `set_hour(hour)` every frame (the morning bird chorus, see WeatherAudio).
const CLOCK_GROUP := "day_clock"

@export var sun: DirectionalLight3D
@export var environment: WorldEnvironment
## Ocean material (materials/ocean_toon.tres): foam and surface follow the light.
@export var water: ShaderMaterial
## Real minutes per full game day; 24 = a game hour per real minute.
@export_range(1.0, 1440.0) var day_length_min: float = 24.0
## Shifts the shared cycle (hours) so a deploy can pick a nicer phase.
@export var time_offset_hours: float = 8.0
@export var day_color: Color = Color(1.0, 0.96, 0.88)
@export var sunset_color: Color = Color(1.0, 0.62, 0.36)
@export var night_color: Color = Color(0.36, 0.46, 0.72)
@export var day_energy: float = 0.72
@export var night_energy: float = 0.4
## Sunrise/sunset windows (hours): light fades in over `sunrise`, out over `sunset`.
@export var sunrise: Vector2 = Vector2(5.0, 7.0)
@export var sunset: Vector2 = Vector2(18.5, 21.0)
@export var fog_day: Color = Color(0.72, 0.85, 0.94)
@export var fog_night: Color = Color(0.08, 0.1, 0.18)

var hour: float = 12.0
var _frozen: bool = false
var _weather_dim: float = 1.0
var _weather_grey: float = 0.0


func _process(_delta: float) -> void:
	if not _frozen:
		var day_sec := day_length_min * 60.0
		var t := fmod(Time.get_unix_time_from_system(), day_sec) / day_sec
		hour = fmod(t * 24.0 + time_offset_hours, 24.0)
	apply()


## Fixes the clock (tools/snapshot.gd, editor previews).
func freeze(at_hour: float) -> void:
	_frozen = true
	hour = fmod(at_hour, 24.0)
	apply()


func unfreeze() -> void:
	_frozen = false


## Weather multiplies the sun (1 = clear sky).
func set_weather_dim(dim: float) -> void:
	_weather_dim = clampf(dim, 0.1, 2.5)


## 0 = coloured sky fog, 1 = flat grey (fog, storm).
func set_weather_grey(grey: float) -> void:
	_weather_grey = clampf(grey, 0.0, 1.0)


## Sun elevation in degrees: -90 at midnight, +90 at noon (0 at 6:00 and 18:00).
func elevation() -> float:
	return sin((hour - 6.0) / 24.0 * TAU) * 90.0


## 0 at night, 1 in full day, ramping through the sunrise/sunset windows.
func daylight() -> float:
	return smoothstep(sunrise.x, sunrise.y, hour) * (1.0 - smoothstep(sunset.x, sunset.y, hour))


func apply() -> void:
	var elev := elevation()
	var daylight := daylight()
	# Warm tint while the light is low: around both windows.
	var golden := maxf(1.0 - absf(hour - sunrise.y) / 2.0, 1.0 - absf(hour - sunset.x) / 2.0)
	golden = clampf(golden, 0.0, 1.0) * daylight
	if sun != null:
		# One light plays sun and moon. The swap happens only at full dark, when the
		# sun's energy has faded to almost nothing, and the moon ramps up over half an
		# hour after it — no visible jump (owner 2026-09-12: night came on instantly).
		var is_day := daylight > 0.0
		# Never below 16° while it is day (a grazing sun blackened the ground long
		# before the sky), never above 52° (an overhead sun flattened everything).
		var light_elev := clampf(elev, 16.0, 52.0) if is_day else maxf(-elev, 20.0)
		var azimuth := (hour / 24.0) * 360.0 + (0.0 if is_day else 180.0)
		sun.rotation_degrees = Vector3(-light_elev, azimuth, 0.0)
		var color := day_color.lerp(sunset_color, golden)
		sun.light_color = color.lerp(night_color, 1.0 - daylight)
		if is_day:
			# sqrt keeps the sun strong through the golden hour; it dies only at full dusk.
			sun.light_energy = lerpf(0.03, day_energy, sqrt(daylight)) * _weather_dim
		else:
			sun.light_energy = night_energy * _moon_rise() * _weather_dim
	if water != null:
		water.set_shader_parameter("foam_color", Color(0.95, 0.98, 1.0).lerp(Color(0.18, 0.24, 0.36), 1.0 - daylight))
		water.set_shader_parameter("surface_color", Color(0.16, 0.5, 0.63).lerp(Color(0.03, 0.09, 0.2), 1.0 - daylight))
		water.set_shader_parameter("depth_color", Color(0.05, 0.17, 0.36).lerp(Color(0.01, 0.03, 0.09), 1.0 - daylight))
	if environment != null and environment.environment != null:
		var env := environment.environment
		var fog_color := fog_night.lerp(fog_day, daylight)
		var grey := Color(0.08, 0.09, 0.12).lerp(Color(0.62, 0.66, 0.7), daylight)
		env.fog_light_color = fog_color.lerp(grey, _weather_grey)
		env.ambient_light_sky_contribution = lerpf(0.6, 0.45, daylight)
		# Golden hour keeps the ground readable: a warm ambient lift while the sun is low.
		env.ambient_light_energy = 1.0 + golden * 0.7
		var sky_mat := env.sky.sky_material if env.sky != null else null
		if sky_mat is ShaderMaterial:
			var sm := sky_mat as ShaderMaterial
			sm.set_shader_parameter("day_night_mix", daylight)
			# Continuous: held at 6° through the day, sliding to the true (negative)
			# elevation across the dusk and dawn windows (sqrt keeps the orange twilight
			# on screen longer) so the sky fades and never flips.
			sm.set_shader_parameter("sun_dir", _sun_direction(lerpf(elev, maxf(elev, 6.0), sqrt(daylight))))
	if is_inside_tree():
		get_tree().call_group(NightLight.GROUP, "set_daylight", daylight)
		get_tree().call_group(CLOCK_GROUP, "set_hour", hour)


func _sun_direction(elev: float) -> Vector3:
	var az := deg_to_rad((hour / 24.0) * 360.0)
	var el := deg_to_rad(elev)
	return Vector3(cos(el) * sin(az), sin(el), cos(el) * cos(az)).normalized()


## 0 right after dusk and right before dawn, 1 in deep night: the moon rises over half an hour.
func _moon_rise() -> float:
	var since := fmod(hour - sunset.y + 24.0, 24.0)
	var until := fmod(sunrise.x - hour + 24.0, 24.0)
	return smoothstep(0.0, 0.5, minf(since, until))
