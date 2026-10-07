extends SceneTree
## Headless test: cleared wild bushes (migration 0023). A bush id round-trips its position;
## `IslandWild` parses the server's `cleared` list (junk skipped), keeps every wild plant
## away from a cleared spot without moving the rest, and offers a WildBush per solid bush;
## a bush loses to any other interactable in reach.
## Run: godot --headless --path client --script res://tests/test_wild_bush.gd

var _failures := 0
var _frames := 0
var _island: Node3D
var _wild: IslandWild
var _victim := Vector2.INF
var _before: Dictionary = {}


func _check(ok: bool, what: String) -> void:
	if not ok:
		_failures += 1
		push_error("FAIL: " + what)


func _init() -> void:
	_island = (load("res://scenes/island/Island.tscn") as PackedScene).instantiate()
	root.add_child(_island)
	_wild = _island.get_node("Wild")


func _process(_delta: float) -> bool:
	_frames += 1
	if _frames == 200:
		_check_ids_and_priority()
		_pick_victim()
		# Clear it the way the bridge does: a `world_state` with `cleared` (decimetres).
		_wild.call("_on_bridge_message", "world_state", {"full": false, "plots": [], "decor": [], "cleared": [[roundi(_victim.x * 10.0), roundi(_victim.y * 10.0)], [1, "x"], "junk", [5]]})
	elif _frames == 500:
		_check_cleared()
		print("RESULT: %s (%d failures)" % ["PASS" if _failures == 0 else "FAIL", _failures])
		quit(1 if _failures > 0 else 0)
		return true
	return false


func _check_ids_and_priority() -> void:
	var id := WildBush.id_for(12.34, -5.66)
	_check(id == "123,-57", "id in decimetres: %s" % id)
	var back := WildBush.point_of(id, Vector3(1.0, 2.0, 3.0))
	_check(absf(back.x - 12.3) < 0.001 and absf(back.z + 5.7) < 0.001 and back.y == 2.0, "point_of reads it back: %s" % back)
	_check(WildBush.point_of("nope", Vector3(1.0, 2.0, 3.0)) == Vector3(1.0, 2.0, 3.0), "a bad id keeps the fallback point")
	var bush := WildBush.new()
	var fake := FakeOther.new()
	root.add_child(bush)
	root.add_child(fake)
	bush.global_position = Vector3(0.5, 0.0, 0.0)
	fake.global_position = Vector3(2.0, 0.0, 0.0)
	var best := Interaction.nearest([bush, fake], Vector3.ZERO, 2.4)
	_check(best.get("node") == fake, "a farther plot beats a nearer wild bush")
	_check(Interaction.nearest([bush], Vector3.ZERO, 2.4).get("node") == bush, "the bush is offered when alone")
	_check(str(Interaction.nearest([bush], Vector3.ZERO, 2.4).get("kind")) == "bush", "kind is bush")


func _pick_victim() -> void:
	var pts: PackedFloat32Array = _wild.placed("w_bush_big")
	for name: String in ["w_bush_big", "w_bush", "w_bush_c", "w_bush_d"]:
		pts = _wild.placed(name)
		if pts.size() >= 8:
			_victim = Vector2(pts[4], pts[5])
			break
	_check(_victim != Vector2.INF, "the island has bushes to clear")
	for name: String in _wild.placed_all().keys():
		_before[name] = (_wild.placed(name) as PackedFloat32Array).duplicate()


func _check_cleared() -> void:
	_check(_wild.get("_cleared").size() == 1, "junk entries skipped, one spot kept: %d" % _wild.get("_cleared").size())
	var kept := 0
	var lost := 0
	for name: String in _before.keys():
		var old: PackedFloat32Array = _before[name]
		var now: PackedFloat32Array = _wild.placed(name)
		for i in range(0, now.size(), 4):
			_check(Vector2(now[i], now[i + 1]).distance_to(_victim) >= IslandWild.CLEAR_RADIUS_M, "%s point stays out of the cleared spot" % name)
		# Everything far from the spot is exactly where it was.
		for i in range(0, old.size(), 4):
			var p := Vector2(old[i], old[i + 1])
			if p.distance_to(_victim) > 6.0:
				var found := false
				for j in range(0, now.size(), 4):
					if absf(now[j] - p.x) < 0.001 and absf(now[j + 1] - p.y) < 0.001:
						found = true
						break
				kept += 1 if found else 0
				lost += 0 if found else 1
			elif p.distance_to(_victim) < IslandWild.CLEAR_RADIUS_M:
				_check(true, "")
	_check(lost == 0 and kept > 20, "plants away from the spot did not move (kept %d, lost %d)" % [kept, lost])
	var proxies := _wild.find_children("*", "Node3D", false, false).filter(func(n: Node) -> bool: return n is WildBush)
	_check(proxies.size() > 3, "solid bushes offer WildBush targets: %d" % proxies.size())
	for p: Node in proxies:
		_check(Vector2((p as Node3D).global_position.x, (p as Node3D).global_position.z).distance_to(_victim) >= IslandWild.CLEAR_RADIUS_M, "no proxy left at the cleared spot")


class FakeOther:
	extends Node3D

	func interaction_kind() -> String:
		return "plot"

	func interaction_index() -> int:
		return 0
