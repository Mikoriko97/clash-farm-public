class_name WorldBounds
extends Node
## Keeps the character on the island top: the walkable area is the rounded square
## the ground shader draws (materials/island_ground.tres half_size / corner_radius),
## not the collision box under it. Anything below the ground is put back at the
## spawn point. Visual/UX only — the server never learns positions.

@export var player: CharacterBody3D
## Base island; IslandShape replaces the rectangle when the server reports expansions.
@export var half_size: float = 24.0
@export var corner_radius: float = 7.0

var box_min: Vector2 = Vector2(-24.0, -24.0)
var box_max: Vector2 = Vector2(24.0, 24.0)
## How far inside the edge the character stops (m).
@export var margin: float = 0.8
@export var floor_y: float = -1.5
@export var spawn: Vector3 = Vector3(-2.0, 0.05, 0.0)


func _ready() -> void:
	add_to_group(IslandShape.GROUP)
	box_min = Vector2(-half_size, -half_size)
	box_max = Vector2(half_size, half_size)


func set_extents(new_min: Vector2, new_max: Vector2) -> void:
	box_min = new_min
	box_max = new_max


func _physics_process(_delta: float) -> void:
	if player == null:
		return
	var p := player.global_position
	if p.y < floor_y:
		player.global_position = spawn
		player.velocity = Vector3.ZERO
		player.reset_physics_interpolation()
		return
	var inside := clamp_rect(Vector2(p.x, p.z), box_min + Vector2(margin, margin), box_max - Vector2(margin, margin), maxf(corner_radius - margin, 0.0))
	if inside != Vector2(p.x, p.z):
		player.global_position = Vector3(inside.x, p.y, inside.y)


## Nearest point of the rounded square (half extent `h`, corner radius `r`) to `p`.
static func clamp_point(p: Vector2, h: float, r: float) -> Vector2:
	return clamp_rect(p, Vector2(-h, -h), Vector2(h, h), r)


## Nearest point of the rounded rectangle `lo..hi` (corner radius `r`) to `p`.
static func clamp_rect(p: Vector2, lo: Vector2, hi: Vector2, r: float) -> Vector2:
	var rr := minf(r, minf(hi.x - lo.x, hi.y - lo.y) * 0.5)
	var c := Vector2(clampf(p.x, lo.x + rr, hi.x - rr), clampf(p.y, lo.y + rr, hi.y - rr))
	var d := p - c
	if d.length() <= rr:
		return p
	return c + d.normalized() * rr
