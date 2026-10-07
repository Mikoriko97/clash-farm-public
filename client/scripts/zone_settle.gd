class_name ZoneSettle
extends Node
## Каже оболонці, коли світ справді готовий (етап 3, docs/PLAN.md): `zone_changed`
## іде через два кадри після зміни сцени, а далі ще секунду-дві триває прогрів
## шейдерів, перші чанки й флора — у браузері це виглядало як зависання одразу
## після появи. Вузол стоїть у кожній сцені світу; коли всі підключені джерела
## роботи вичерпані (або минув `timeout_s`) і минуло `min_frames` кадрів, шле
## `zone_ready { zone }` — React тримає завісу переходу до нього.

@export var warmup: ShaderWarmup
@export var terrain: MainlandTerrain
@export var wild: IslandWild
@export var min_frames: int = 6
@export var timeout_s: float = 6.0

var _frames := 0
var _elapsed := 0.0
var _sent := false


func _ready() -> void:
	var bridge: Node = get_node_or_null(^"/root/Bridge")
	if bridge != null:
		bridge.message_received.connect(_on_bridge_message)


## Стартова сцена прогрівається ще до входу гравця: коли оболонка підписалась
## (`session_ready`), готовність повторюється, інакше завіса чекала б страховки.
func _on_bridge_message(type: String, _payload: Dictionary) -> void:
	if type == "session_ready" and _sent:
		_announce()


func _process(delta: float) -> void:
	_frames += 1
	_elapsed += delta
	if _sent or _frames < min_frames:
		return
	if _elapsed < timeout_s and not _settled():
		return
	_sent = true
	set_process(false)
	FrameMark.mark("zone ready after %d frames" % _frames)
	_announce()
	# Auto quality measures real play, not the loading that came before.
	var quality: Node = get_node_or_null(^"/root/Quality")
	if quality != null and quality.has_method("arm"):
		quality.call("arm")


func _announce() -> void:
	var bridge: Node = get_node_or_null(^"/root/Bridge")
	if bridge != null:
		bridge.call("send", "zone_ready", {"zone": str(bridge.get("zone"))})


func is_ready() -> bool:
	return _sent


func _settled() -> bool:
	if warmup != null and not warmup.is_done():
		return false
	if terrain != null and (terrain.pending_count() > 0 or terrain.flora_pending() > 0):
		return false
	if wild != null and wild.pending() > 0:
		return false
	return true
