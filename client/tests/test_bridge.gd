extends SceneTree
## Headless test for scripts/bridge.gd (docs/BRIDGE_PROTOCOL.md): valid
## messages are accepted and emitted, invalid ones are dropped without errors.
## Run: godot --headless --path client --script res://tests/test_bridge.gd

var _failures := 0
var _received: Array[String] = []
var _modes: Array[String] = []


func _init() -> void:
	var bridge: Node = (load("res://scripts/bridge.gd") as GDScript).new()
	root.add_child(bridge)
	bridge.message_received.connect(func(t: String, _p: Dictionary) -> void: _received.append(t))
	bridge.mode_changed.connect(func(m: String) -> void: _modes.append(m))

	_check(bridge.handle({"v": 1, "type": "session_ready", "payload": {"island_id": "abc"}}), "session_ready accepted")
	_check(bridge.handle_raw('{"v":1,"type":"set_mode","payload":{"mode":"ui"}}'), "set_mode ui via JSON accepted")
	_check(bridge.mode == "ui" and _modes == ["ui"], "mode switched to ui once")
	_check(bridge.handle_raw('{"v":1,"type":"set_mode","payload":{"mode":"ui"}}'), "repeat set_mode accepted")
	_check(_modes.size() == 1, "repeat set_mode does not re-emit")
	_check(bridge.handle({"v": 1, "type": "world_state", "payload": {"full": true, "plots": [], "decor": []}}), "world_state accepted")
	_check(bridge.handle({"v": 1, "type": "inventory_changed", "payload": {}}), "empty payload type accepted")

	_check(not bridge.handle_raw("{not json"), "malformed JSON dropped")
	_check(not bridge.handle("string"), "non-object dropped")
	_check(not bridge.handle({"v": 2, "type": "set_mode", "payload": {"mode": "walk"}}), "wrong version dropped")
	_check(not bridge.handle({"v": 1, "type": "drain_wallet", "payload": {}}), "unknown type dropped")
	_check(not bridge.handle({"v": 1, "type": "set_mode", "payload": {"mode": "root"}}), "unknown mode dropped")
	_check(not bridge.handle({"v": 1, "type": "set_mode", "payload": {"mode": 7}}), "wrong payload type dropped")
	_check(not bridge.handle({"v": 1, "type": "world_state", "payload": {"full": true}}), "missing keys dropped")
	_check(not bridge.handle({"v": 1, "type": "session_ready"}), "missing payload dropped")
	# Character (ADR-025): the look is remembered for scenes that appear later; the preview switch is a bool.
	_check(bridge.handle({"v": 1, "type": "player_look", "payload": {"look": {"skin": "#ffffff"}}}), "player_look accepted")
	_check(bridge.look.get("skin") == "#ffffff", "look kept on the bridge")
	_check(not bridge.handle({"v": 1, "type": "player_look", "payload": {"look": "#ffffff"}}), "player_look needs an object")
	_check(bridge.handle({"v": 1, "type": "camera_preview", "payload": {"on": true, "yaw_deg": 30.0}}), "camera_preview accepted")
	_check(not bridge.handle({"v": 1, "type": "camera_preview", "payload": {"on": "yes"}}), "camera_preview needs a bool")
	_check(_received == ["session_ready", "set_mode", "set_mode", "world_state", "inventory_changed", "player_look", "camera_preview"], "only valid messages emitted")

	print("RESULT: %s (%d failures)" % ["PASS" if _failures == 0 else "FAIL", _failures])
	quit(1 if _failures > 0 else 0)


func _check(ok: bool, what: String) -> void:
	if not ok:
		_failures += 1
	print("%s %s" % ["ok  " if ok else "FAIL", what])
