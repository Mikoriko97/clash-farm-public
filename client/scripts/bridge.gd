extends Node
## Autoload "Bridge" — the only door between Godot and the React shell
## (docs/BRIDGE_PROTOCOL.md). Wire format: one JSON string per message,
## `{ "v": 1, "type": "...", "payload": {...} }`.
##   Godot -> React: window.cf.fromGodot(json)
##   React -> Godot: window.cf.toGodot(json)  (callback registered here)
## Invalid input is warned about and dropped; never thrown, never eval'd.

signal message_received(type: String, payload: Dictionary)
signal mode_changed(mode: String)

const PROTOCOL_VERSION := 1
const CLIENT_VERSION := "0.1.0"
const MODES: Array[String] = ["walk", "build", "ui"]
const OUTBOUND: Array[String] = ["ready", "plot_selected", "building_entered",
	"mode_changed", "error", "player_near", "cell_selected", "place_selected", "house_entered", "player_pose",
	"zone_changed", "player_chunk", "debug_probe", "quality_changed", "zone_ready", "volume_changed"]
## Zone -> scene (docs/MAINLAND.md): `set_zone` swaps the whole world.
const ZONE_SCENES: Dictionary = {
	"island": "res://scenes/island/Island.tscn",
	"mainland": "res://scenes/mainland/Mainland.tscn",
	"cave": "res://scenes/cave/Cave.tscn",
}
## Inbound type -> required payload keys with their Variant type (TYPE_NIL = any).
const INBOUND: Dictionary = {
	"session_ready": {"island_id": TYPE_STRING},
	"world_state": {"full": TYPE_BOOL, "plots": TYPE_ARRAY, "decor": TYPE_ARRAY},
	"inventory_changed": {},
	"set_mode": {"mode": TYPE_STRING},
	"play_fx": {"fx": TYPE_STRING},
	"camera_focus": {"target": TYPE_NIL},
	"theme": {"season_id": TYPE_STRING},
	"walk_to": {"kind": TYPE_STRING, "idx": TYPE_FLOAT},
	"move_input": {"x": TYPE_FLOAT, "y": TYPE_FLOAT},
	"jump": {},
	"debug_world": {},
	"build_rotate": {"delta_deg": TYPE_FLOAT},
	"house": {"inside": TYPE_BOOL},
	"peers_state": {"peers": TYPE_ARRAY},
	"set_zone": {"zone": TYPE_STRING},
	"mainland_nodes": {"chunks": TYPE_ARRAY},
	"mainland_node": {"id": TYPE_STRING, "hits_left": TYPE_FLOAT},
	"mainland_landmarks": {"landmarks": TYPE_ARRAY},
	"teleport": {"zone": TYPE_STRING, "x_dm": TYPE_FLOAT, "z_dm": TYPE_FLOAT},
	"set_quality": {"tier": TYPE_STRING},
	"set_volume": {"sfx": TYPE_FLOAT, "ambient": TYPE_FLOAT},
	"player_look": {"look": TYPE_DICTIONARY},
	"camera_preview": {"on": TYPE_BOOL},
}

var mode: String = "walk"
## Кольори вигляду гравця (`player_look`, «#rrggbb» за слотами): сцени зон беруть їх при появі.
var look: Dictionary = {}
## Останній `move_input` джойстика: оболонка шле лише зміни, тож після зміни зони його повторюємо новій сцені (як `look`).
var move_input: Dictionary = {}
var zone: String = "island"
## Скільки змін світу триває (стара сцена вже поза деревом, нова ще не зʼявилась) і повідомлення, що зачекали на їх кінець.
var _swaps := 0
var _held: Array = []
var _cf: JavaScriptObject
var _callback: JavaScriptObject


func _ready() -> void:
	if OS.has_feature("web"):
		_attach_js()
	send("ready", {"version": client_version()})


func _attach_js() -> void:
	var window: JavaScriptObject = JavaScriptBridge.get_interface("window")
	if window == null or window.cf == null:
		push_warning("Bridge: window.cf missing; running without shell")
		return
	_cf = window.cf
	_callback = JavaScriptBridge.create_callback(_on_js_message)
	_cf.toGodot = _callback


## Godot -> React. Only protocol types; payload must be JSON-serialisable.
func send(type: String, payload: Dictionary) -> void:
	if type not in OUTBOUND:
		push_warning("Bridge: refusing to send unknown type %s" % type)
		return
	var json := JSON.stringify({"v": PROTOCOL_VERSION, "type": type, "payload": payload})
	# Кожне повідомлення в web — це console.log (поза 8 Гц, джойстик безперервно):
	# лише з `--verbose`, не в збірці гравця.
	print_verbose("[bridge] -> %s" % type)
	if _cf == null:
		print_verbose("Bridge (no shell) -> %s" % json)
		return
	_cf.fromGodot(json)


func _on_js_message(args: Array) -> void:
	if args.is_empty() or typeof(args[0]) != TYPE_STRING:
		push_warning("Bridge: toGodot expects one JSON string")
		return
	handle_raw(args[0])


## Parses and dispatches one message. Returns true if accepted (used by tests).
func handle_raw(raw: String) -> bool:
	var json := JSON.new()
	if json.parse(raw) != OK:
		push_warning("Bridge: malformed JSON at line %d: %s" % [json.get_error_line(), json.get_error_message()])
		return false
	return handle(json.data)


func handle(msg: Variant) -> bool:
	if typeof(msg) != TYPE_DICTIONARY:
		push_warning("Bridge: message is not an object")
		return false
	var dict: Dictionary = msg
	var version: Variant = dict.get("v")
	if not (typeof(version) in [TYPE_INT, TYPE_FLOAT]) or int(version) != PROTOCOL_VERSION:
		push_warning("Bridge: unsupported protocol version %s" % str(version))
		return false
	var type: Variant = dict.get("type")
	if typeof(type) != TYPE_STRING or not INBOUND.has(type):
		push_warning("Bridge: unknown type %s" % str(type))
		return false
	var payload: Variant = dict.get("payload", {})
	if typeof(payload) != TYPE_DICTIONARY or not _valid_payload(type, payload):
		push_warning("Bridge: invalid payload for %s" % type)
		return false
	if type == "set_mode":
		_set_mode(payload["mode"])
	if type == "player_look":
		look = payload["look"]
	if type == "move_input":
		move_input = payload
	# Поки світ міняється, повідомлення оболонки чекають (стан `mode`/`look` уже оновлено вище): інакше їх дістали б вузли старої
	# сцени без дерева й засипали консоль помилками («Parameter data.tree is null»). Оболонка після `zone_changed` і так оновлює стан.
	if _swaps > 0 and type != "set_zone":
		_held.append([type, payload])
		return true
	print_verbose("[bridge] <- %s" % type)
	message_received.emit(type, payload)
	if type == "set_zone":
		_set_zone(payload["zone"], payload)
	return true


func _valid_payload(type: String, payload: Dictionary) -> bool:
	var schema: Dictionary = INBOUND[type]
	for key: String in schema:
		if not payload.has(key):
			return false
		var expected: int = schema[key]
		if expected != TYPE_NIL and typeof(payload[key]) != expected:
			return false
	if type == "set_mode" and payload["mode"] not in MODES:
		return false
	if type == "set_zone" and not ZONE_SCENES.has(payload["zone"]):
		return false
	return true


## Swaps the world scene; `zone_changed` follows once the new scene is in the tree
## (React then re-sends the island state or fetches mainland chunks).
## `spawn` may carry `x_dm`/`z_dm`: where the character stands in the new zone
## (leaving the cave puts it at the mine gate, not the dock).
func _set_zone(new_zone: String, spawn: Dictionary = {}) -> void:
	if new_zone == zone:
		send("zone_changed", {"zone": zone})
		return
	zone = new_zone
	FrameMark.mark("zone load " + zone)
	_swaps += 1
	get_tree().change_scene_to_file(ZONE_SCENES[zone])
	_announce_zone(spawn)


func _announce_zone(spawn: Dictionary) -> void:
	await get_tree().process_frame
	await get_tree().process_frame
	var scene: Node = get_tree().current_scene
	if spawn.has("x_dm") and spawn.has("z_dm") and scene != null and scene.has_method("_teleport"):
		scene.call("_teleport", {"zone": zone, "x_dm": float(spawn["x_dm"]), "z_dm": float(spawn["z_dm"])})
	_swaps -= 1
	if _swaps == 0:
		var held := _held
		_held = []
		# Утримуваний стік: нова сцена має почути його, навіть якщо за час заміни оболонка нічого не надіслала.
		if not move_input.is_empty():
			held.push_front(["move_input", move_input])
		for message: Array in held:
			message_received.emit(message[0], message[1])
	send("zone_changed", {"zone": zone})


func _set_mode(new_mode: String) -> void:
	if new_mode == mode:
		return
	mode = new_mode
	mode_changed.emit(mode)
	send("mode_changed", {"mode": mode})


## Версія клієнта зі штампом збірки: `res://build.txt` кладе `addons/build_stamp`
## під час експорту (час UTC і git-коміт); у редакторі файлу нема — «dev».
static func client_version() -> String:
	var stamp := "dev"
	if FileAccess.file_exists("res://build.txt"):
		stamp = FileAccess.get_file_as_string("res://build.txt").strip_edges()
	return CLIENT_VERSION + " " + stamp
