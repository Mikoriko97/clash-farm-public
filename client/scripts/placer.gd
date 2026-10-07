class_name Placer
extends Node3D
## Build mode (`set_mode build`): a ghost follows the pointer and a click on the
## spot the ghost already sits on confirms it. Two kinds:
## - plot (no `item` in set_mode): the ghost snaps to the 3 m cell grid and
##   sends `cell_selected { col, row }`;
## - decor (`item` + `footprint_dm`): the ghost snaps to 0.5 m, turns in 15°
##   steps (`build_rotate`, key R) and sends `place_selected { x_dm, z_dm, yaw_deg }`.
## The ghost only hints (green = looks free); the server decides.

const SNAP_M := 0.5
const YAW_STEP := 15

@export var plot_grid: PlotGrid
@export var decor_grid: DecorGrid
@export var player: Node3D
## Island top the ghost may sit on; IslandShape replaces the rectangle on expansions.
@export var half_size: float = 24.0
@export var corner_radius: float = 7.0

var box_min: Vector2 = Vector2(-24.0, -24.0)
var box_max: Vector2 = Vector2(24.0, 24.0)
## Soil half width plus the rim kept clear at the edge (server: PLOT_HALF_M).
@export var plot_half: float = 2.0
@export var edge_margin: float = 0.8
@export var ok_material: Material
@export var bad_material: Material

var active: bool = false
var kind: String = "plot"
var item_id: String = ""
var footprint: float = 1.0
var yaw_deg: int = 0
var _cell: Vector2i = Vector2i(9999, 9999)
var _spot: Vector3 = Vector3(9999.0, 0.0, 9999.0)

@onready var _ghost: Node3D = %Ghost
@onready var _field: Node3D = %Field
@onready var _decor_holder: Node3D = %DecorGhost


func _ready() -> void:
	add_to_group(IslandShape.GROUP)
	box_min = Vector2(-half_size, -half_size)
	box_max = Vector2(half_size, half_size)
	_ghost.visible = false
	var bridge: Node = get_node_or_null(^"/root/Bridge")
	if bridge != null:
		bridge.mode_changed.connect(func(mode: String) -> void: set_active(mode == "build"))
		# Режим, змінений під час заміни зони, дійшов лише до старої сцени: беремо поточний.
		set_active(bridge.mode == "build")
		bridge.message_received.connect(_on_bridge_message)


func _on_bridge_message(type: String, payload: Dictionary) -> void:
	if type == "set_mode" and str(payload.get("mode", "")) == "build":
		var item := str(payload.get("item", ""))
		var fp: Variant = payload.get("footprint_dm", 10)
		var fp_m := float(fp) / 10.0 if typeof(fp) == TYPE_FLOAT or typeof(fp) == TYPE_INT else 1.0
		configure(item, clampf(fp_m if is_finite(fp_m) else 1.0, 0.1, 10.0))
	elif type == "build_rotate":
		var delta: Variant = payload.get("delta_deg", YAW_STEP)
		rotate_ghost(int(delta) if typeof(delta) == TYPE_FLOAT or typeof(delta) == TYPE_INT else YAW_STEP)


## What the ghost stands for: a plot (empty item) or a decor item with its footprint radius (m).
func configure(item: String, footprint_m: float) -> void:
	item_id = item
	kind = "plot" if item.is_empty() else "decor"
	footprint = maxf(footprint_m, 0.3)
	yaw_deg = 0
	_field.visible = kind == "plot"
	for child: Node in _decor_holder.get_children():
		child.queue_free()
	if kind == "decor" and decor_grid != null:
		var scene := decor_grid.piece_for(item)
		if scene != null:
			var ghost := scene.instantiate()
			_strip_colliders(ghost)
			_decor_holder.add_child(ghost)
	_decor_holder.rotation.y = 0.0
	_cell = Vector2i(9999, 9999)
	_spot = Vector3(9999.0, 0.0, 9999.0)


func set_active(on: bool) -> void:
	active = on
	_ghost.visible = false
	_cell = Vector2i(9999, 9999)
	_spot = Vector3(9999.0, 0.0, 9999.0)


static func cell_of(world: Vector3) -> Vector2i:
	return Vector2i(roundi(world.x / PlotGrid.CELL_M), roundi(world.z / PlotGrid.CELL_M))


static func snap(world: Vector3) -> Vector3:
	return Vector3(roundf(world.x / SNAP_M) * SNAP_M, 0.0, roundf(world.z / SNAP_M) * SNAP_M)


## Moves the ghost to the spot under `world` (pointer hover / first tap).
func hover(world: Vector3) -> void:
	if not active:
		return
	if kind == "plot":
		var cell := cell_of(world)
		if cell == _cell and _ghost.visible:
			return
		_cell = cell
		_ghost.global_position = PlotGrid.cell_position(cell.x, cell.y)
		_set_ghost_material(ok_material if is_free(cell) else bad_material)
	else:
		var spot := snap(world)
		if spot == _spot and _ghost.visible:
			return
		_spot = spot
		_ghost.global_position = spot
		_set_ghost_material(ok_material if is_spot_free(spot, footprint) else bad_material)
	_ghost.visible = true


## Click: confirms the hovered spot, otherwise just moves the ghost there (touch).
func click(world: Vector3) -> void:
	if not active:
		return
	var bridge: Node = get_node_or_null(^"/root/Bridge")
	if kind == "plot":
		var cell := cell_of(world)
		if cell != _cell or not _ghost.visible:
			hover(world)
			return
		if is_free(cell) and bridge != null:
			bridge.send("cell_selected", {"col": cell.x, "row": cell.y})
		return
	var spot := snap(world)
	if spot != _spot or not _ghost.visible:
		hover(world)
		return
	if is_spot_free(spot, footprint) and bridge != null:
		bridge.send("place_selected", {"x_dm": roundi(spot.x * 10.0), "z_dm": roundi(spot.z * 10.0), "yaw_deg": yaw_deg})


## Turns the decor ghost by `delta` degrees (multiples of 15).
func rotate_ghost(delta: int) -> void:
	if kind != "decor":
		return
	yaw_deg = posmod(yaw_deg + int(roundf(float(delta) / YAW_STEP)) * YAW_STEP, 360)
	_decor_holder.rotation.y = deg_to_rad(float(yaw_deg))


## Plot cell: not taken and inside the island top.
func is_free(cell: Vector2i) -> bool:
	if plot_grid != null and plot_grid.cell_taken(cell.x, cell.y):
		return false
	var centre := Vector2(cell.x * PlotGrid.CELL_M, cell.y * PlotGrid.CELL_M)
	return _inside(centre, plot_half)


## Decor spot: inside the island, off every plot cell, and touching no collider
## (trees, rocks, other decor). The server repeats the real checks.
func is_spot_free(spot: Vector3, r: float) -> bool:
	if not _inside(Vector2(spot.x, spot.z), r + edge_margin):
		return false
	if plot_grid != null:
		var lo := cell_of(spot - Vector3(r, 0.0, r))
		var hi := cell_of(spot + Vector3(r, 0.0, r))
		for row in range(lo.y, hi.y + 1):
			for col in range(lo.x, hi.x + 1):
				if plot_grid.cell_taken(col, row):
					return false
	return not _touches_body(spot, r)


func set_extents(new_min: Vector2, new_max: Vector2) -> void:
	box_min = new_min
	box_max = new_max


func _inside(p: Vector2, margin: float) -> bool:
	return WorldBounds.clamp_rect(p, box_min + Vector2(margin, margin), box_max - Vector2(margin, margin), maxf(corner_radius - margin, 0.0)) == p


func _touches_body(spot: Vector3, r: float) -> bool:
	if not is_inside_tree():
		return false
	var shape := CylinderShape3D.new()
	shape.radius = r
	shape.height = 1.0
	var query := PhysicsShapeQueryParameters3D.new()
	query.shape = shape
	query.transform = Transform3D(Basis.IDENTITY, spot + Vector3(0.0, 0.6, 0.0))
	if player != null and player is CollisionObject3D:
		query.exclude = [(player as CollisionObject3D).get_rid()]
	return not get_world_3d().direct_space_state.intersect_shape(query, 1).is_empty()


## The ghost must be untouchable: its prop collider would catch the pointer ray
## (the ghost would chase itself) and count as an obstacle in the clearance check.
static func _strip_colliders(node: Node) -> void:
	for body: Node in node.find_children("*", "CollisionObject3D", true, false):
		body.get_parent().remove_child(body)
		body.queue_free()


func _set_ghost_material(material: Material) -> void:
	for mesh: Node in _ghost.find_children("*", "MeshInstance3D", true, false):
		(mesh as MeshInstance3D).material_override = material
