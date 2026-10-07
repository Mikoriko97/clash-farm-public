class_name ChunkFlora
extends RefCounted
## Будує MultiMesh одного виду `FloraKind` на одному чанку із запеченого
## масиву [x, z, yaw, scale, …] (`FloraBake`, docs/MAINLAND.md §9): висота —
## найнижча точка під слідом, камені й колоди нахиляються за схилом, дрібне
## має дальність видимості й не кидає тіні. Види з `collider_radius_m` на
## ближніх чанках (LOD 0) отримують одне StaticBody3D з циліндром на кожен
## екземпляр — стовбури, кущі й валуни тверді (фідбек власника 2026-09-11:
## персонаж проходив крізь дерева). Дерева Simple Trees на ближніх чанках
## отримують другий MultiMesh — невидиму гладку крону `shadow_crown`, яка лише
## кидає тінь (листя тінь від сонця не кидає, дірки в картках мерехтіли), а
## листя — відтінок екземпляра (`instance_tint`). Викликається з
## `MainlandTerrain` по одному виду за виклик, щоб не було ривків.

const MMBuffer := preload("res://scripts/multimesh_buffer.gd")

## Ставить `kind` за `data`; повертає кількість екземплярів.
static func build_kind(chunk: Node3D, terrain: Node3D, kind: FloraKind, data: PackedFloat32Array) -> int:
	if kind == null or kind.scene == null or data.size() < 4:
		return 0
	var near := int(chunk.get("lod")) == 0
	# Near chunks draw the middle level of detail, far chunks the lightest one
	# (Simple Trees; single-mesh pieces ignore the hint).
	var found: Dictionary = terrain.piece_mesh(kind.scene, "mid" if near else "far")
	var mesh: Mesh = found.get("mesh")
	if mesh == null:
		return 0
	var xform: Transform3D = found.get("xform", Transform3D.IDENTITY)
	var crown: Dictionary = terrain.piece_mesh(kind.scene, "shadow_crown") if near else {}
	var with_crown := bool(crown.get("matched", false)) and kind.shadows
	var tinted := uses_tint(kind)
	var n := data.size() / 4
	var buf := MMBuffer.new(n, tinted)
	var shadow_buf: MMBuffer = MMBuffer.new(n) if with_crown else null
	var origin: Vector3 = chunk.position
	var solid := kind.collider_radius_m > 0.0 and near
	var grounded := kind.radius_m >= 0.9 and near
	var spots := PackedFloat32Array()
	var normals := PackedVector3Array()
	var body: StaticBody3D = null
	if solid:
		body = StaticBody3D.new()
		body.name = "FloraBody_" + kind.resource_path.get_file().get_basename()
	for i: int in n:
		var x := data[i * 4]
		var z := data[i * 4 + 1]
		var s := data[i * 4 + 3]
		var y: float = terrain.ground_min(x, z, minf(kind.radius_m * s, 1.0)) - kind.sink_m
		var basis := Basis(Vector3.UP, data[i * 4 + 2])
		if kind.tilt:
			var nrm: Vector3 = terrain.normal_at(x, z)
			if nrm.y < 0.999:
				basis = Basis(Quaternion(Vector3.UP, nrm)) * basis
		var local := Vector3(x, y, z) - origin
		var placed := Transform3D(basis.scaled(Vector3(s, s, s)), local)
		buf.add(placed * xform, instance_tint(kind, x, z) if tinted else Color.WHITE)
		if shadow_buf != null:
			shadow_buf.add(placed * (crown["xform"] as Transform3D))
		if solid:
			body.add_child(cylinder(kind, s, local))
		if grounded:
			spots.append_array(PackedFloat32Array([local.x, local.y + kind.sink_m, local.z, (kind.collider_radius_m if kind.collider_radius_m > 0.0 else kind.radius_m) * s * ContactShadow.SPREAD + 0.6]))
			normals.append(terrain.normal_at(x, z))
	var inst := MultiMeshInstance3D.new()
	inst.name = "Flora_" + kind.resource_path.get_file().get_basename()
	inst.multimesh = buf.build(mesh)
	if kind.visible_m > 0.0:
		inst.visibility_range_end = kind.visible_m
		inst.visibility_range_end_margin = 4.0
	# The crown proxy is the only shadow caster of a tree: the visible mesh casting too
	# doubled the shadow-pass triangles (leaves collapse there anyway).
	if not kind.shadows or with_crown:
		inst.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	chunk.add_child(inst)
	if shadow_buf != null:
		var shadow := MultiMeshInstance3D.new()
		shadow.name = "FloraShadow_" + kind.resource_path.get_file().get_basename()
		shadow.multimesh = shadow_buf.build(crown["mesh"])
		shadow.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_SHADOWS_ONLY
		chunk.add_child(shadow)
	if solid:
		chunk.add_child(body)
	if grounded and not spots.is_empty():
		ContactShadow.multimesh(chunk, "Contact_" + kind.resource_path.get_file().get_basename(), spots, normals)
	return n



static func uses_tint(kind: FloraKind) -> bool:
	return kind.tint_min != Color.WHITE or kind.tint_max != Color.WHITE or kind.autumn_chance > 0.0


## Deterministic leaf tint of the instance at (x, z): patches of similar colour
## across the land, a little per-tree scatter, now and then an autumn tree.
static func instance_tint(kind: FloraKind, x: float, z: float) -> Color:
	var h := hash(Vector2i(roundi(x * 10.0), roundi(z * 10.0)))
	var r1 := float(h & 0xFFFF) / 65535.0
	var r2 := float((h >> 16) & 0xFFFF) / 65535.0
	if r2 < kind.autumn_chance:
		return kind.autumn_tint
	var patch := 0.5 + 0.4 * sin(x * 0.031 + z * 0.017) * cos(z * 0.027 - x * 0.011)
	return kind.tint_min.lerp(kind.tint_max, clampf(patch + (r1 - 0.5) * 0.6, 0.0, 1.0))


## Вертикальний циліндр-колайдер екземпляра (стовбур не нахиляється зі схилом).
static func cylinder(kind: FloraKind, s: float, base: Vector3) -> CollisionShape3D:
	var shape := CylinderShape3D.new()
	shape.radius = kind.collider_radius_m * s
	shape.height = kind.collider_height_m * s
	var col := CollisionShape3D.new()
	col.shape = shape
	col.position = base + Vector3(0.0, shape.height * 0.5, 0.0)
	return col
