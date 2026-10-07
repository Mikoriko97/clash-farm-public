class_name PlotGrid
extends Node3D
## Instances one PlotNode per plot from `world_state`: the server names the
## cell (`col`, `row`, 3 m pitch, centred on the island), nothing is placed by
## hand. Side neighbours are linked into beds (soil fills the gap, the ring
## lights the whole bed) and fed their state. Unknown idx in a patch = new plot.

const CELL_M := 3.0

@export var plot_scene: PackedScene

var _plots: Dictionary = {}  # idx -> PlotNode


func _ready() -> void:
	add_to_group(Footsteps.PROVIDER_GROUP)
	var bridge: Node = get_node_or_null(^"/root/Bridge")
	if bridge != null:
		bridge.message_received.connect(_on_bridge_message)


func plot(idx: int) -> PlotNode:
	return _plots.get(idx) as PlotNode


func plot_count() -> int:
	return _plots.size()


## Поверхня для кроків (`Footsteps`): клітинка із ділянкою — земля, решта — не мій вибір (порожній рядок).
func surface_at(position: Vector3) -> String:
	var local := to_local(position)
	return "dirt" if cell_taken(roundi(local.x / CELL_M), roundi(local.z / CELL_M)) else ""


func cell_taken(col: int, row: int) -> bool:
	for node: PlotNode in _plots.values():
		if node.col == col and node.row == row:
			return true
	return false


static func cell_position(col: int, row: int) -> Vector3:
	return Vector3(col * CELL_M, 0.0, row * CELL_M)


func _on_bridge_message(type: String, payload: Dictionary) -> void:
	match type:
		"world_state":
			apply_world_state(payload)
		"play_fx":
			_play_fx(payload)


## `{ full, plots: [{idx, col, row, state, crop_id?, ready_at?}], now }`.
## A full snapshot drops plots it does not list; a patch only touches its entries.
func apply_world_state(payload: Dictionary) -> void:
	var now_unix := PlotNode._parse_iso(str(payload.get("now", "")))
	var local_now := Time.get_unix_time_from_system()
	if now_unix <= 0.0:
		now_unix = local_now
	var entries: Variant = payload.get("plots", [])
	if typeof(entries) != TYPE_ARRAY:
		return
	print_verbose("[plots] world_state: %d entries, full=%s" % [(entries as Array).size(), str(payload.get("full", false))])
	var listed: Dictionary = {}
	for entry: Variant in entries:
		if typeof(entry) != TYPE_DICTIONARY:
			continue
		var node := _ensure_plot(entry as Dictionary)
		if node == null:
			continue
		listed[node.idx] = true
		node.apply(entry as Dictionary, now_unix, local_now)
	if bool(payload.get("full", false)):
		for idx: int in _plots.keys():
			if not listed.has(idx):
				(_plots[idx] as Node).queue_free()
				_plots.erase(idx)
	_relink()
	if is_inside_tree():
		get_tree().call_group(NavGrid.GROUP, "mark_dirty")
		get_tree().call_group(GrassGpu.GROUP, "scatter")


## `cf.plots` CHECK (migration 0006): cells further out are not a valid payload.
const CELL_MAX := 16


## Existing plot by idx, or a new one on the entry's cell; null without a usable cell.
func _ensure_plot(entry: Dictionary) -> PlotNode:
	var idx := int(entry.get("idx", -1))
	if idx < 0:
		return null
	var node := plot(idx)
	var has_cell := _valid_cell(entry)
	if node != null:
		if has_cell:
			_move(node, int(entry["col"]), int(entry["row"]))
		return node
	if plot_scene == null or not has_cell:
		push_warning("PlotGrid: plot %d has no valid cell; ignored" % idx)
		return null
	node = plot_scene.instantiate() as PlotNode
	node.idx = idx
	node.name = "Plot%d" % idx
	add_child(node)
	_move(node, int(entry["col"]), int(entry["row"]))
	_plots[idx] = node
	return node


## Numbers within the server's cell range; anything else is malformed (rules/godot.md).
static func _valid_cell(entry: Dictionary) -> bool:
	for key: String in ["col", "row"]:
		var v: Variant = entry.get(key)
		if typeof(v) != TYPE_FLOAT and typeof(v) != TYPE_INT:
			return false
		if absi(int(v)) > CELL_MAX:
			return false
	return true


func _move(node: PlotNode, col: int, row: int) -> void:
	node.col = col
	node.row = row
	node.position = cell_position(col, row)


## Bed links: soil between side neighbours, and every plot knows its bed mates.
func _relink() -> void:
	var by_cell: Dictionary = {}
	for node: PlotNode in _plots.values():
		by_cell[Vector2i(node.col, node.row)] = node
	var seen: Dictionary = {}
	for node: PlotNode in _plots.values():
		var c := Vector2i(node.col, node.row)
		node.set_links(
			by_cell.has(c + Vector2i(0, -1)), by_cell.has(c + Vector2i(1, 0)),
			by_cell.has(c + Vector2i(0, 1)), by_cell.has(c + Vector2i(-1, 0)))
		if seen.has(c):
			continue
		var bed: Array = _flood(c, by_cell)
		for member: PlotNode in bed:
			seen[Vector2i(member.col, member.row)] = true
			member.bed_mates = bed


func _flood(start: Vector2i, by_cell: Dictionary) -> Array:
	var out: Array = []
	var stack: Array[Vector2i] = [start]
	var visited: Dictionary = {}
	while not stack.is_empty():
		var c: Vector2i = stack.pop_back()
		if visited.has(c) or not by_cell.has(c):
			continue
		visited[c] = true
		out.append(by_cell[c])
		for d: Vector2i in [Vector2i(1, 0), Vector2i(-1, 0), Vector2i(0, 1), Vector2i(0, -1)]:
			stack.append(c + d)
	return out


func _play_fx(payload: Dictionary) -> void:
	var target: Variant = payload.get("target", {})
	if typeof(target) != TYPE_DICTIONARY:
		return
	var node := plot(int((target as Dictionary).get("plot_idx", -1)))
	if node == null:
		return
	node.play_fx(str(payload.get("fx", "")))
	var tween := create_tween()
	tween.tween_property(node, "scale", Vector3(1.15, 1.15, 1.15), 0.12)
	tween.tween_property(node, "scale", Vector3.ONE, 0.18)
