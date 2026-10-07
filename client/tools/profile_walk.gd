extends SceneTree
## Профіль ривків материка без сервера (вікно, не headless — рендер теж рахується):
##   godot --path client --script res://tools/profile_walk.gd -- [--from=-160,-40] [--to=-160,-300] [--step=2] [--every=6]
## Ставить у кожен чанк стільки ж вузлів, скільки дає рецепт сервера (ліс 10,
## луки 6, скелі 7, болото 5), і веде персонажа з `--from` до `--to` по `step` м
## кожні `every` кадрів. Друкує 12 найдовших кадрів з тим, що в них сталося
## (збудовані чанки, поставлені вузли), і підсумок: середній кадр, кадрів > 50 мс.

const RECIPES := {
	"F": ["tree_pine", "tree_pine", "tree_pine", "tree_pine", "tree_pine", "tree_pine", "tree_oak", "tree_oak", "mushroom", "herb"],
	"D": ["tree_oak", "tree_oak", "tree_oak", "tree_oak", "tree_oak", "tree_oak", "tree_pine", "tree_pine", "mushroom_rare"],
	"R": ["rock", "rock", "rock", "rock", "ore_copper", "ore_copper", "ore_iron"],
	"M": ["herb", "herb", "herb", "herb", "tree_oak", "rock"],
	"S": ["reed", "reed", "reed", "reed", "mushroom"],
}

var _scene: Node3D
var _terrain: MainlandTerrain
var _nodes: MainlandNodes
var _player: Node3D
var _from := Vector2(-160.0, -40.0)
var _to := Vector2(-160.0, -300.0)
var _step := 2.0
var _every := 6
var _frames := 0
var _pos: Vector2
var _last_usec := 0
var _worst: Array = []
var _total_ms := 0.0
var _slow := 0
var _prev_chunks := 0
var _prev_nodes := 0


func _init() -> void:
	for a: String in OS.get_cmdline_user_args():
		var v := a.get_slice("=", 1)
		if a.begins_with("--from="):
			_from = Vector2(float(v.get_slice(",", 0)), float(v.get_slice(",", 1)))
		elif a.begins_with("--to="):
			_to = Vector2(float(v.get_slice(",", 0)), float(v.get_slice(",", 1)))
		elif a.begins_with("--step="):
			_step = float(v)
		elif a.begins_with("--every="):
			_every = int(v)
	_scene = (load("res://scenes/mainland/Mainland.tscn") as PackedScene).instantiate()
	root.add_child(_scene)
	_terrain = _scene.get_node("Terrain")
	_nodes = _scene.get_node("Nodes")
	_player = _scene.get_node("Player")
	_pos = _from
	_player.global_position = Vector3(_from.x, 30.0, _from.y)
	_nodes.apply(_fake_chunks())
	_last_usec = Time.get_ticks_usec()


func _fake_chunks() -> Array:
	var zones: Variant = JSON.parse_string(FileAccess.get_file_as_string(ProjectSettings.globalize_path("res://") + "../shared/mainland_zones.json"))
	var rows: Array = zones.get("rows", []) if typeof(zones) == TYPE_DICTIONARY else []
	var rng := RandomNumberGenerator.new()
	rng.seed = 11
	var out: Array = []
	for cz: int in rows.size():
		for cx: int in str(rows[cz]).length():
			var kinds: Array = RECIPES.get(str(rows[cz])[cx], [])
			var entries: Array = []
			for i: int in kinds.size():
				var x := -512.0 + cx * 64.0 + rng.randf_range(3.0, 61.0)
				var z := -512.0 + cz * 64.0 + rng.randf_range(3.0, 61.0)
				entries.append({"id": "%d_%d_%d" % [cx, cz, i], "kind": kinds[i], "x_dm": int(x * 10.0), "z_dm": int(z * 10.0), "hits_left": 3})
			if not entries.is_empty():
				out.append({"cx": cx, "cz": cz, "nodes": entries})
	return out


func _process(_delta: float) -> bool:
	var now := Time.get_ticks_usec()
	var ms := float(now - _last_usec) / 1000.0
	_last_usec = now
	_frames += 1
	if _frames > 30:
		var built := _terrain.loaded_count() - _prev_chunks
		var spawned := _nodes.count() - _prev_nodes
		_total_ms += ms
		if ms > 50.0:
			_slow += 1
		_worst.append([ms, _frames, built, spawned, _pos])
		_worst.sort_custom(func(a: Array, b: Array) -> bool: return a[0] > b[0])
		if _worst.size() > 12:
			_worst.resize(12)
	_prev_chunks = _terrain.loaded_count()
	_prev_nodes = _nodes.count()
	if _frames > 30 and _frames % _every == 0:
		if _pos.distance_to(_to) <= _step:
			_report()
			quit()
			return true
		_pos = _pos.move_toward(_to, _step)
		_player.global_position = Vector3(_pos.x, _terrain.height_at(_pos.x, _pos.y) + 0.5, _pos.y)
	return false


func _report() -> void:
	var n := _frames - 30
	print("profile_walk: %d frames, mean %.1f ms, %d frames over 50 ms, chunks %d, nodes %d" % [n, _total_ms / maxf(1.0, float(n)), _slow, _terrain.loaded_count(), _nodes.count()])
	for w: Array in _worst:
		print("  %7.1f ms  frame %5d  chunks+%d nodes+%d  at (%.0f, %.0f)" % [w[0], w[1], w[2], w[3], w[4].x, w[4].y])
