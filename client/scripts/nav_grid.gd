class_name NavGrid
extends Node
## Walkable-area planner for click-to-walk: a coarse grid over the island top
## (same rounded square as WorldBounds) where every StaticBody3D collision box
## under `obstacle_root` (plots, props) is solid, inflated by the character
## radius. Paths come from the engine's AStarGrid2D and are string-pulled to a
## few straight legs. View logic only: the server never sees positions.

const GROUP := "nav_grid"

## Base island; IslandShape replaces the rectangle when the server reports expansions.
@export var half_size: float = 24.0
@export var corner_radius: float = 7.0

var box_min: Vector2 = Vector2(-24.0, -24.0)
var box_max: Vector2 = Vector2(24.0, 24.0)
@export var margin: float = 0.8
@export var cell: float = 0.5
## Character capsule radius plus a little clearance.
@export var agent_radius: float = 0.4
## Subtree scanned for StaticBody3D colliders; the parent when unset.
@export var obstacle_root: Node
## Colliders whose top is below this height are ground, not obstacles (m).
@export var min_top_y: float = 0.05
## Colliders wider than this are ground-like and ignored (m).
@export var max_span: float = 30.0

var _astar := AStarGrid2D.new()
var _cells: int = 0
var _cells_x: int = 0
var _cells_y: int = 0
var _dirty: bool = true
## Cells outside the island shape (index x + y * _cells_x): the shape only changes with the extents,
## so `mark_dirty` (a plot placed, decor swapped) refills the obstacles without recomputing it.
var _outside: PackedInt32Array = PackedInt32Array()
## Inputs `_outside` was computed for; any difference recomputes it.
var _outside_key: Array = []


func _ready() -> void:
	add_to_group(GROUP)
	add_to_group(IslandShape.GROUP)
	box_min = Vector2(-half_size, -half_size)
	box_max = Vector2(half_size, half_size)
	configure()


func set_extents(new_min: Vector2, new_max: Vector2) -> void:
	box_min = new_min
	box_max = new_max
	configure()


## (Re)creates the grid for the current exports; obstacles are rasterised lazily.
func configure() -> void:
	var size := box_max - box_min
	_cells_x = ceili(size.x / cell)
	_cells_y = ceili(size.y / cell)
	_cells = maxi(_cells_x, _cells_y)
	_astar.region = Rect2i(0, 0, _cells_x, _cells_y)
	_astar.cell_size = Vector2(cell, cell)
	_astar.offset = box_min + Vector2(cell * 0.5, cell * 0.5)
	_astar.diagonal_mode = AStarGrid2D.DIAGONAL_MODE_ONLY_IF_NO_OBSTACLES
	_astar.default_compute_heuristic = AStarGrid2D.HEURISTIC_OCTILE
	_astar.default_estimate_heuristic = AStarGrid2D.HEURISTIC_OCTILE
	_astar.update()
	_dirty = true


## Obstacles changed (plots placed, decor swapped): rasterise again before the next path.
func mark_dirty() -> void:
	_dirty = true


## Waypoints from `from` to `to` (y kept from `from`), ending at `to` itself when its
## cell is free, else at the nearest free cell. Empty when no route exists.
func find_path(from: Vector3, to: Vector3) -> PackedVector3Array:
	if _dirty:
		rebuild()
	var start := nearest_free(to_cell(Vector2(from.x, from.z)))
	var goal_cell := to_cell(Vector2(to.x, to.z))
	var goal_free := not _solid(goal_cell)
	var goal := goal_cell if goal_free else nearest_free(goal_cell)
	if start.x < 0 or goal.x < 0:
		return PackedVector3Array()
	var cells := _astar.get_point_path(start, goal)
	if cells.is_empty():
		return PackedVector3Array()
	var points := _string_pull(cells)
	if not _solid(to_cell(Vector2(from.x, from.z))):
		points[0] = Vector2(from.x, from.z)
	if goal_free:
		points[points.size() - 1] = Vector2(to.x, to.z)
	var out := PackedVector3Array()
	for p: Vector2 in points:
		out.append(Vector3(p.x, from.y, p.y))
	return out


func rebuild() -> void:
	_astar.fill_solid_region(_astar.region, false)
	var key := [box_min, box_max, margin, corner_radius, cell]
	if key != _outside_key:
		_outside = _shape_outside()
		_outside_key = key
	for i: int in _outside:
		_astar.set_point_solid(Vector2i(i % _cells_x, i / _cells_x), true)
	var root := obstacle_root if obstacle_root != null else get_parent()
	for rect: Rect2 in collect_obstacles(root):
		_fill_rect(rect.grow(agent_radius))
	_dirty = false


## Cells whose centre lies outside the rounded island top (minus `margin`).
func _shape_outside() -> PackedInt32Array:
	var out := PackedInt32Array()
	var lo := box_min + Vector2(margin, margin)
	var hi := box_max - Vector2(margin, margin)
	var r := maxf(corner_radius - margin, 0.0)
	for y in _cells_y:
		for x in _cells_x:
			var centre := _astar.get_point_position(Vector2i(x, y))
			if WorldBounds.clamp_rect(centre, lo, hi, r) != centre:
				out.append(x + y * _cells_x)
	return out


## Horizontal footprints (x, z) of every StaticBody3D collision shape under `root`.
func collect_obstacles(root: Node) -> Array[Rect2]:
	var rects: Array[Rect2] = []
	if root == null:
		return rects
	for node: Node in root.find_children("*", "CollisionShape3D", true, false):
		var shape := node as CollisionShape3D
		var body := shape.get_parent() as StaticBody3D
		if shape.shape == null or shape.disabled or body == null or (body.collision_layer & 1) == 0:
			continue  # pick-only bodies (decor layer 2) never block a route
		var box := world_aabb(shape)
		if box.end.y < min_top_y or box.size.x > max_span or box.size.z > max_span:
			continue
		rects.append(Rect2(box.position.x, box.position.z, box.size.x, box.size.z))
	return rects


## World-space AABB of a collision shape: primitives from their parameters, anything else via its debug mesh.
static func world_aabb(shape: CollisionShape3D) -> AABB:
	return shape.global_transform * local_aabb(shape.shape)


static func local_aabb(shape: Shape3D) -> AABB:
	if shape is BoxShape3D:
		var size := (shape as BoxShape3D).size
		return AABB(-size * 0.5, size)
	if shape is SphereShape3D:
		var r := (shape as SphereShape3D).radius
		return AABB(Vector3(-r, -r, -r), Vector3(r, r, r) * 2.0)
	if shape is CylinderShape3D:
		var cyl := shape as CylinderShape3D
		return AABB(Vector3(-cyl.radius, -cyl.height * 0.5, -cyl.radius), Vector3(cyl.radius * 2.0, cyl.height, cyl.radius * 2.0))
	if shape is CapsuleShape3D:
		var cap := shape as CapsuleShape3D
		return AABB(Vector3(-cap.radius, -cap.height * 0.5, -cap.radius), Vector3(cap.radius * 2.0, cap.height, cap.radius * 2.0))
	return shape.get_debug_mesh().get_aabb()


func to_cell(p: Vector2) -> Vector2i:
	var c := ((p - _astar.offset) / cell).round()
	return Vector2i(clampi(int(c.x), 0, _cells_x - 1), clampi(int(c.y), 0, _cells_y - 1))


func is_free(p: Vector2) -> bool:
	if _dirty:
		rebuild()
	return not _solid(to_cell(p))


## Closest non-solid cell by growing rings; (-1, -1) when the grid is full.
func nearest_free(c: Vector2i) -> Vector2i:
	if not _solid(c):
		return c
	for ring in range(1, _cells):
		var best := Vector2i(-1, -1)
		var best_d := INF
		for dy in range(-ring, ring + 1):
			for dx in range(-ring, ring + 1):
				if absi(dx) != ring and absi(dy) != ring:
					continue
				var n := c + Vector2i(dx, dy)
				var d := float(dx * dx + dy * dy)
				if _astar.region.has_point(n) and not _solid(n) and d < best_d:
					best = n
					best_d = d
		if best.x >= 0:
			return best
	return Vector2i(-1, -1)


func _solid(c: Vector2i) -> bool:
	return not _astar.region.has_point(c) or _astar.is_point_solid(c)


func _fill_rect(rect: Rect2) -> void:
	var a := to_cell(rect.position)
	var b := to_cell(rect.end)
	for y in range(a.y, b.y + 1):
		for x in range(a.x, b.x + 1):
			_astar.set_point_solid(Vector2i(x, y), true)


## Drops intermediate cells while the straight line to a later cell stays free.
func _string_pull(cells: PackedVector2Array) -> PackedVector2Array:
	var out := PackedVector2Array([cells[0]])
	var i := 0
	while i < cells.size() - 1:
		var j := cells.size() - 1
		while j > i + 1 and not _line_free(cells[i], cells[j]):
			j -= 1
		out.append(cells[j])
		i = j
	return out


func _line_free(a: Vector2, b: Vector2) -> bool:
	var steps := maxi(int(ceil(a.distance_to(b) / (cell * 0.5))), 1)
	for s in range(steps + 1):
		if _solid(to_cell(a.lerp(b, float(s) / steps))):
			return false
	return true
