class_name MeshCollider
extends StaticBody3D
## Exact collision for a prop: at ready, every MeshInstance3D under `source`
## (the pack piece next to this body) becomes a ConcavePolygonShape3D child of
## this body, and the hand-placed box/cylinder shapes are dropped unless
## `keep_simple` is on. Low-poly pieces make this cheap; the character then
## stops at the real silhouette (owner feedback 2026-09-07) and seats, rims and
## wheels are where they look. Trees keep their trunk cylinder on purpose: a
## canopy trimesh would block the whole crown for pathfinding.

## Node whose meshes define the collider; defaults to the sibling "Piece".
@export var source: Node3D
## Keep the authored simple shapes next to the trimesh (e.g. a trunk cylinder).
@export var keep_simple: bool = false


func _ready() -> void:
	var root: Node3D = source
	if root == null:
		root = get_parent().get_node_or_null(^"Piece") as Node3D
	if root == null:
		push_warning("MeshCollider: no source piece under %s" % get_parent().name)
		return
	var built := build_from(root)
	if built > 0 and not keep_simple:
		for child: Node in get_children():
			if child is CollisionShape3D and not child.is_in_group("trimesh"):
				# Out of the tree at once so obstacle scans in the same frame do not see it.
				remove_child(child)
				child.queue_free()


## Adds one trimesh shape per mesh; returns how many were built.
func build_from(root: Node3D) -> int:
	var built := 0
	for node: Node in root.find_children("*", "MeshInstance3D", true, false):
		var mesh_node := node as MeshInstance3D
		if mesh_node.mesh == null:
			continue
		var shape := mesh_node.mesh.create_trimesh_shape()
		if shape == null:
			continue
		var collision := CollisionShape3D.new()
		collision.name = "Trimesh%d" % built
		collision.shape = shape
		collision.add_to_group("trimesh")
		add_child(collision)
		collision.global_transform = mesh_node.global_transform
		built += 1
	return built
