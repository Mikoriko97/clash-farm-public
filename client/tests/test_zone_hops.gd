extends SceneTree
## Headless smoke test: справжній автолоад Bridge проводить світ крізь усі зони й назад, поки оболонка не вгаває (рух, вигляд, звуки,
## стан світу), і движок не має надрукувати жодної помилки. Саме так вилізла помилка зі скріншота власника 2026-10-01: після кожної
## зміни зони `message_received` кликав статичний callable з мертвим вузлом гравця («Cannot convert argument 3 from Object to Object»).
## Run: godot --headless --path client --script res://tests/test_zone_hops.gd

const ErrorLog := preload("res://tests/error_log.gd")
const HOPS: Array[String] = ["mainland", "island", "cave", "island", "mainland", "cave", "mainland", "island"]
const FRAMES_PER_HOP := 30

var _log := ErrorLog.new()
var _failures := 0
var _frames := 0
var _bridge: Node


func _process(_delta: float) -> bool:
	_frames += 1
	if _frames == 2:
		_bridge = root.get_node_or_null(^"Bridge")
		_check(_bridge != null, "the Bridge autoload exists")
		if _bridge == null:
			return _finish()
		OS.add_logger(_log)
	if _bridge == null or _frames < 3:
		return false
	var hop := (_frames - 3) / FRAMES_PER_HOP
	if hop >= HOPS.size():
		return _finish()
	if (_frames - 3) % FRAMES_PER_HOP == 0:
		_bridge.handle({"v": 1, "type": "set_zone", "payload": {"zone": HOPS[hop]}})
	_chatter()
	return false


## Те, що оболонка шле під час гри.
func _chatter() -> void:
	_bridge.handle({"v": 1, "type": "move_input", "payload": {"x": 0.4, "y": -0.9}})
	_bridge.handle({"v": 1, "type": "player_look", "payload": {"look": {"skin": "#c68642", "shirt": "#3366cc"}}})
	_bridge.handle({"v": 1, "type": "world_state", "payload": {"full": true, "plots": [], "decor": []}})
	if _frames % 7 == 0:
		_bridge.handle({"v": 1, "type": "play_fx", "payload": {"fx": "interact"}})
		_bridge.handle({"v": 1, "type": "set_mode", "payload": {"mode": "ui" if _frames % 14 == 0 else "walk"}})


func _finish() -> bool:
	OS.remove_logger(_log)
	_check(_log.errors.is_empty(), "no engine errors across %d zone changes: %s" % [HOPS.size(), "; ".join(_log.errors.slice(0, 8))])
	print("RESULT: %s (%d failures)" % ["PASS" if _failures == 0 else "FAIL", _failures])
	quit(1 if _failures > 0 else 0)
	return true


func _check(ok: bool, what: String) -> void:
	if not ok:
		_failures += 1
		push_error("FAIL: " + what)
