class_name Footsteps
extends Node
## Кроки персонажа (UX: звук, docs/UI.md): компонент у сценах `Player` і `PeerAvatar` стежить за кліпом ходьби чи бігу й на кожен
## дотик стопи грає крок поверхні під ногами. Моменти дотику виміряно по кістках стоп кліпів (`CONTACTS`, частки циклу): ліва нога
## на початку опори, права — через півцикл. Поверхню дає промінь униз (`metadata/surface` колайдера, наприклад міст = `wood`), далі
## «постачальники» групи `surface_provider` (ділянки = земля; материк за висотою, схилом і біомом — трава, пісок, сніг, камінь), далі зона (печера = камінь, інше = трава).
## Свій персонаж (`local`) грає без просторовості, інші — `AudioStreamPlayer3D` лише в межах `RANGE_M` від камери (далі не чутно,
## тож і гравців для них не створюємо). Гучність береться з шини `SFX` (повзунок меню). Нічого не вирішує й не шле на сервер.

signal stepped(surface: String, loudness_db: float)

## Частки циклу кліпу, на яких стопа торкається землі (лівою, правою).
const CONTACTS: Dictionary = {
	&"walk_forward": [0.52, 0.02],
	&"walk_back": [0.72, 0.27],
	&"run_forward": [0.52, 0.02],
	&"sprint_forward": [0.15, 0.65],
}
## Гучність кроку, дБ відносно вирівняного кліпа: біг гучніший за ходьбу.
const LOUDNESS: Dictionary = {&"walk_forward": -13.0, &"walk_back": -15.0, &"run_forward": -9.0, &"sprint_forward": -7.0}
const STEPS: Dictionary = {
	"grass": [
		preload("res://assets/audio/step_grass_1.wav"), preload("res://assets/audio/step_grass_2.wav"),
		preload("res://assets/audio/step_grass_3.wav"), preload("res://assets/audio/step_grass_4.wav"),
	],
	"dirt": [
		preload("res://assets/audio/step_dirt_1.wav"), preload("res://assets/audio/step_dirt_2.wav"),
		preload("res://assets/audio/step_dirt_3.wav"), preload("res://assets/audio/step_dirt_4.wav"),
	],
	"stone": [
		preload("res://assets/audio/step_stone_1.wav"), preload("res://assets/audio/step_stone_2.wav"),
		preload("res://assets/audio/step_stone_3.wav"), preload("res://assets/audio/step_stone_4.wav"),
	],
	"wood": [
		preload("res://assets/audio/step_wood_1.wav"), preload("res://assets/audio/step_wood_2.wav"), preload("res://assets/audio/step_wood_3.wav"),
	],
	"sand": [
		preload("res://assets/audio/step_sand_1.wav"), preload("res://assets/audio/step_sand_2.wav"),
		preload("res://assets/audio/step_sand_3.wav"), preload("res://assets/audio/step_sand_4.wav"),
	],
	"snow": [
		preload("res://assets/audio/step_snow_1.wav"), preload("res://assets/audio/step_snow_2.wav"),
		preload("res://assets/audio/step_snow_3.wav"), preload("res://assets/audio/step_snow_4.wav"),
	],
}
const DEFAULT_SURFACE := "grass"
const PROVIDER_GROUP := "surface_provider"
const BUS := "SFX"
## Далі цієї відстані від камери чужі кроки не грають.
const RANGE_M := 22.0
const PLAYERS := 2

## Свій персонаж: звук без просторовості й без перевірки відстані до камери.
@export var local: bool = false

var _anim: AnimationPlayer
var _body: Node3D
var _clip: StringName = &""
var _phase := 0.0
var _last: Dictionary = {}
var _players: Array[Node] = []
var _next_player := 0


func _ready() -> void:
	_body = get_parent() as Node3D
	_anim = _body.get_node_or_null(^"%AnimationPlayer") as AnimationPlayer if _body != null else null
	if _anim == null:
		set_process(false)


func _process(_delta: float) -> void:
	var clip := StringName(_anim.current_animation)
	var contacts: Array = CONTACTS.get(clip, [])
	if contacts.is_empty() or not _anim.is_playing() or _anim.current_animation_length <= 0.0:
		_clip = &""
		return
	var phase := _anim.current_animation_position / _anim.current_animation_length
	if clip != _clip:
		_clip = clip
		_phase = phase
		return
	for contact: float in contacts:
		if crossed(_phase, phase, contact):
			_step(clip)
	_phase = phase


## Чи пройшла фаза `mark` між `from` і `to` (цикл зациклюється: `to` < `from` = обгорнули).
static func crossed(from: float, to: float, mark: float) -> bool:
	if to >= from:
		return mark > from and mark <= to
	return mark > from or mark <= to


func _step(clip: StringName) -> void:
	var position := _body.global_position
	if not local and not _audible(position):
		return
	var surface := surface_at(_body, position)
	var clips: Array = STEPS.get(surface, STEPS[DEFAULT_SURFACE])
	# Не той самий варіант двічі поспіль і невелика різниця висоти: серія кроків не звучить як луп.
	var index := randi() % clips.size()
	if index == int(_last.get(surface, -1)) and clips.size() > 1:
		index = (index + 1) % clips.size()
	_last[surface] = index
	var loudness: float = LOUDNESS.get(clip, -12.0)
	_play(clips[index] as AudioStream, position, loudness)
	stepped.emit(surface, loudness)


func _audible(position: Vector3) -> bool:
	var camera := get_viewport().get_camera_3d()
	return camera == null or camera.global_position.distance_to(position) <= RANGE_M


func _play(stream: AudioStream, position: Vector3, loudness: float) -> void:
	if _players.size() < PLAYERS:
		var fresh: Node = AudioStreamPlayer.new() if local else AudioStreamPlayer3D.new()
		fresh.set("bus", BUS)
		if not local:
			fresh.set("unit_size", 3.0)
			fresh.set("max_distance", RANGE_M)
			fresh.set("attenuation_model", AudioStreamPlayer3D.ATTENUATION_INVERSE_DISTANCE)
		add_child(fresh)
		_players.append(fresh)
	var player := _players[_next_player]
	_next_player = (_next_player + 1) % PLAYERS
	player.set("stream", stream)
	player.set("volume_db", loudness)
	player.set("pitch_scale", randf_range(0.94, 1.06))
	if not local:
		(player as Node3D).global_position = position
	player.call("play")


## Поверхня під `position`: метадані колайдера під ногами → постачальники групи `surface_provider` → зона.
static func surface_at(from: Node3D, position: Vector3) -> String:
	var query := PhysicsRayQueryParameters3D.create(position + Vector3.UP * 0.6, position + Vector3.DOWN * 1.2)
	if from is CollisionObject3D:
		var own: Array[RID] = [(from as CollisionObject3D).get_rid()]
		query.exclude = own
	var hit := from.get_world_3d().direct_space_state.intersect_ray(query)
	if not hit.is_empty():
		var collider := hit.get("collider") as Node
		if collider != null and collider.has_meta("surface") and STEPS.has(String(collider.get_meta("surface"))):
			return String(collider.get_meta("surface"))
	for provider in from.get_tree().get_nodes_in_group(PROVIDER_GROUP):
		var surface := String(provider.call("surface_at", position))
		if STEPS.has(surface):
			return surface
	var audio := from.get_node_or_null(^"/root/Audio")
	return "stone" if audio != null and str(audio.get("zone")) == "cave" else DEFAULT_SURFACE
