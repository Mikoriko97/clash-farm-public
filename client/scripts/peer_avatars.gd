class_name PeerAvatars
extends Node3D
## Keeps one PeerAvatar per neighbour from `peers_state { peers: [...] }`
## (docs/BRIDGE_PROTOCOL.md). Unknown ids spawn, listed ones update, unlisted
## ones are dropped after a short grace so a missed frame never blinks them out.
## Malformed entries are ignored (rules/godot.md). Pure view.

@export var avatar_scene: PackedScene
## An avatar absent from the list for longer than this disappears (ms).
@export var grace_ms: int = 1500
## How often (s) avatars are checked for staleness between messages: the shell may simply stop sending.
@export var purge_secs: float = 0.5

var _avatars: Dictionary = {}
## Терен зони (сусідній вузол `Terrain` з `height_at`): аватари стоять на землі, а не під нею.
var _ground: Node = null
var _since_purge: float = 0.0


func _ready() -> void:
	if _ground == null:
		_ground = get_node_or_null(^"../Terrain")
	var bridge: Node = get_node_or_null(^"/root/Bridge")
	if bridge != null:
		bridge.message_received.connect(_on_bridge_message)


func _on_bridge_message(type: String, payload: Dictionary) -> void:
	if type == "peers_state":
		apply(payload.get("peers", []))


func _process(delta: float) -> void:
	_since_purge += delta
	if _since_purge < purge_secs or _avatars.is_empty():
		return
	_since_purge = 0.0
	_purge({}, grace_ms)


func apply(raw: Variant) -> void:
	if typeof(raw) != TYPE_ARRAY:
		push_warning("PeerAvatars: peers is not an array")
		return
	if (raw as Array).is_empty():
		_purge({}, 0)  # an explicit empty list means nobody is around: no grace
		return
	var listed := {}
	for entry: Variant in raw:
		if not _valid(entry):
			continue
		var id := str(entry["id"])
		listed[id] = true
		var avatar: PeerAvatar = _avatars.get(id)
		if avatar == null:
			avatar = _spawn(id)
			if avatar == null:
				continue
			avatar.apply(entry, true)
		else:
			avatar.apply(entry)
	_purge(listed, grace_ms)


## Drops avatars not in `listed` and unseen for `grace` ms.
func _purge(listed: Dictionary, grace: int) -> void:
	for id: String in _avatars.keys():
		var avatar: PeerAvatar = _avatars[id]
		# `>=`: нульова пауза означає «одразу» (з `>` два `apply` в одну й ту саму мілісекунду лишали пропалого гравця — нестабільний тест)
		if not listed.has(id) and avatar.seen_ms_ago() >= grace:
			_avatars.erase(id)
			avatar.queue_free()


func count() -> int:
	return _avatars.size()


func avatar(id: String) -> PeerAvatar:
	return _avatars.get(id)


static func _valid(entry: Variant) -> bool:
	if typeof(entry) != TYPE_DICTIONARY:
		return false
	var e := entry as Dictionary
	if typeof(e.get("id")) != TYPE_STRING or str(e["id"]).is_empty():
		return false
	for key: String in ["x_dm", "z_dm", "yaw_deg"]:
		var v: Variant = e.get(key)
		if typeof(v) != TYPE_FLOAT and typeof(v) != TYPE_INT:
			return false
	return true


func _spawn(id: String) -> PeerAvatar:
	if avatar_scene == null:
		push_warning("PeerAvatars: no avatar scene")
		return null
	var avatar := avatar_scene.instantiate() as PeerAvatar
	if avatar == null:
		return null
	avatar.peer_id = id
	avatar.ground = _ground
	avatar.name = "Peer_" + id
	add_child(avatar)
	_avatars[id] = avatar
	return avatar
