class_name IslandShape
extends Node
## The island top as the server describes it (ADR-015): a 48 m base plus 8 m
## strips bought per side. `world_state.expand { n, e, s, w }` turns into an
## axis-aligned rectangle; the ground mesh, its collision, the ground shader and
## every node in group "island_extent" (WorldBounds, NavGrid, Placer, GrassGpu)
## take the new extents. Nothing here is gameplay: the server owns the counters.

const GROUP := "island_extent"
const BASE_HALF_M := 24.0
const STRIP_M := 8.0
const MAX_PER_SIDE := 4
## How far past the flat top the mesh keeps going (shader edge + slack).
const MESH_SKIRT_M := 14.0

@export var ground_mesh: MeshInstance3D
@export var ground_collision: CollisionShape3D
@export var ground_material: ShaderMaterial
## Вода (materials/water_shore_island.tres): піна на березі рахується з тієї ж форми острова.
@export var water_material: ShaderMaterial

## Current top: x from box_min.x to box_max.x, z from box_min.y to box_max.y (north = -z).
var box_min: Vector2 = Vector2(-BASE_HALF_M, -BASE_HALF_M)
var box_max: Vector2 = Vector2(BASE_HALF_M, BASE_HALF_M)


func _ready() -> void:
	var bridge: Node = get_node_or_null(^"/root/Bridge")
	if bridge != null:
		bridge.message_received.connect(_on_bridge_message)


func _on_bridge_message(type: String, payload: Dictionary) -> void:
	if type == "world_state" and payload.has("expand"):
		apply_expand(payload["expand"])


## `{ n, e, s, w }` strip counts, all four required; anything malformed is ignored
## (rules/godot.md) so a partial payload can never shrink the island.
func apply_expand(raw: Variant) -> void:
	if typeof(raw) != TYPE_DICTIONARY:
		push_warning("IslandShape: expand is not a dictionary")
		return
	var counts := {}
	for side: String in ["n", "e", "s", "w"]:
		if not (raw as Dictionary).has(side):
			push_warning("IslandShape: expand.%s missing" % side)
			return
		var v: Variant = (raw as Dictionary).get(side)
		if typeof(v) != TYPE_FLOAT and typeof(v) != TYPE_INT:
			push_warning("IslandShape: bad expand.%s" % side)
			return
		counts[side] = clampi(int(v), 0, MAX_PER_SIDE)
	set_extents(
		Vector2(-(BASE_HALF_M + STRIP_M * counts["w"]), -(BASE_HALF_M + STRIP_M * counts["n"])),
		Vector2(BASE_HALF_M + STRIP_M * counts["e"], BASE_HALF_M + STRIP_M * counts["s"]))


func set_extents(new_min: Vector2, new_max: Vector2) -> void:
	box_min = new_min
	box_max = new_max
	var centre := (box_min + box_max) * 0.5
	var half := (box_max - box_min) * 0.5
	if ground_mesh != null:
		ground_mesh.position = Vector3(centre.x, ground_mesh.position.y, centre.y)
		var plane := ground_mesh.mesh as PlaneMesh
		if plane != null:
			plane.size = (half + Vector2(MESH_SKIRT_M, MESH_SKIRT_M)) * 2.0
	if ground_collision != null:
		ground_collision.position = Vector3(centre.x, ground_collision.position.y, centre.y)
		var box := ground_collision.shape as BoxShape3D
		if box != null:
			box.size = Vector3(half.x * 2.0, box.size.y, half.y * 2.0)
	if ground_material != null:
		ground_material.set_shader_parameter("half_extent", half)
	if water_material != null:
		water_material.set_shader_parameter("half_extent", half)
		water_material.set_shader_parameter("island_centre", centre)
	if is_inside_tree():
		get_tree().call_group(GROUP, "set_extents", box_min, box_max)
		get_tree().call_group(NavGrid.GROUP, "mark_dirty")
