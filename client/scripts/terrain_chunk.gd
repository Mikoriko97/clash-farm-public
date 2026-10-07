@tool
class_name TerrainChunk
extends StaticBody3D
## One 64 m square of mainland ground (docs/MAINLAND.md §9): a grid mesh sampled
## from MainlandTerrain's height map with biome weights in the vertex colour,
## a HeightMapShape3D collider at LOD 0 (grass is GrassCarpet, flora is baked).
## Built once by `build`, freed on unload; baked flora (`ChunkFlora`) is added by
## MainlandTerrain over the following frames, one kind at a time.

## Same as CHUNK; a local copy keeps the two scripts free of a
## class cycle (exported builds refuse to parse mutually typed class_names).
const CHUNK := 64
## Neighbouring chunks at different LODs sample the map at different steps, so
## their shared edge does not line up exactly; a curtain hanging this far (m)
## below every edge hides the crack from any angle.
const SKIRT_DEPTH := 8.0

var cx: int = 0
var cz: int = 0
var lod: int = 0

var _mesh_instance: MeshInstance3D


## `step` metres between vertices (2 near, 8 far); colliders and greenery at LOD 0 only.
func build(terrain: Node3D, chunk_x: int, chunk_z: int, step: int, with_collision: bool) -> void:
	cx = chunk_x
	cz = chunk_z
	lod = 0 if with_collision else 1
	name = "Chunk_%d_%d" % [cx, cz]
	position = Vector3(terrain.chunk_origin(cx), 0.0, terrain.chunk_origin(cz))
	collision_layer = Interaction.WALK_LAYER
	var heights := _build_mesh(terrain, step)
	if with_collision:
		_build_collision(heights, step)


## Grid of (n+1)² vertices; returns the sampled heights row by row for the collider.
func _build_mesh(terrain: Node3D, step: int) -> PackedFloat32Array:
	var n := CHUNK / step
	var px0 := cx * CHUNK
	var pz0 := cz * CHUNK
	var vertices := PackedVector3Array()
	var normals := PackedVector3Array()
	var colors := PackedColorArray()
	var uvs := PackedVector2Array()
	var heights := PackedFloat32Array()
	for iz: int in n + 1:
		for ix: int in n + 1:
			var px := px0 + ix * step
			var pz := pz0 + iz * step
			var h: float = terrain.height_px(px, pz)
			heights.append(h)
			vertices.append(Vector3(ix * step, h, iz * step))
			normals.append(terrain.normal_px(px, pz, step))
			colors.append(terrain.biome_px(px, pz))
			uvs.append(Vector2(px, pz) / float(CHUNK))
	var indices := PackedInt32Array()
	for iz: int in n:
		for ix: int in n:
			var a := iz * (n + 1) + ix
			var b := a + 1
			var c := a + n + 1
			var d := c + 1
			# Godot front faces wind clockwise.
			indices.append_array([a, b, c, b, d, c])
	_add_skirt(vertices, normals, colors, uvs, indices, n, SKIRT_DEPTH)
	var arrays := []
	arrays.resize(Mesh.ARRAY_MAX)
	arrays[Mesh.ARRAY_VERTEX] = vertices
	arrays[Mesh.ARRAY_NORMAL] = normals
	arrays[Mesh.ARRAY_COLOR] = colors
	arrays[Mesh.ARRAY_TEX_UV] = uvs
	arrays[Mesh.ARRAY_INDEX] = indices
	var mesh := ArrayMesh.new()
	mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)
	_mesh_instance = MeshInstance3D.new()
	_mesh_instance.name = "Mesh"
	_mesh_instance.mesh = mesh
	_mesh_instance.material_override = terrain.ground_material
	add_child(_mesh_instance)
	return heights


## Vertical curtain around the chunk, both windings so it shows from either side.
static func _add_skirt(vertices: PackedVector3Array, normals: PackedVector3Array, colors: PackedColorArray, uvs: PackedVector2Array, indices: PackedInt32Array, n: int, depth: float) -> void:
	var loop := PackedInt32Array()
	for ix: int in n + 1:
		loop.append(ix)
	for iz: int in range(1, n + 1):
		loop.append(iz * (n + 1) + n)
	for ix: int in range(n - 1, -1, -1):
		loop.append(n * (n + 1) + ix)
	for iz: int in range(n - 1, 0, -1):
		loop.append(iz * (n + 1))
	var lowered := PackedInt32Array()
	for top: int in loop:
		lowered.append(vertices.size())
		vertices.append(vertices[top] - Vector3(0.0, depth, 0.0))
		normals.append(normals[top])
		colors.append(colors[top])
		uvs.append(uvs[top])
	for i: int in loop.size():
		var j := (i + 1) % loop.size()
		var p := loop[i]
		var q := loop[j]
		var pl := lowered[i]
		var ql := lowered[j]
		indices.append_array([p, q, pl, q, ql, pl, p, pl, q, q, pl, ql])


## HeightMapShape3D is centred on its node and 1 unit per sample: scale it by `step`
## and park it in the middle of the chunk so it lines up with the mesh.
func _build_collision(heights: PackedFloat32Array, step: int) -> void:
	var n := CHUNK / step
	var shape := HeightMapShape3D.new()
	shape.map_width = n + 1
	shape.map_depth = n + 1
	shape.map_data = heights
	var collision := CollisionShape3D.new()
	collision.name = "Collision"
	collision.shape = shape
	collision.position = Vector3(CHUNK * 0.5, 0.0, CHUNK * 0.5)
	collision.scale = Vector3(step, 1.0, step)
	add_child(collision)


## Non-interactive trees for visual density where the forest weight is high;
## resource nodes (the ones you can chop) come from the server separately.
