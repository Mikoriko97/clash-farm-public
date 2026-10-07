class_name ContactShadow
extends RefCounted
## Контактні тіні (`shaders/contact_ao.gdshader`): спільна площина 1 × 1 м з
## матеріалом множення. `multimesh()` кладе пляму під кожен екземпляр флори,
## `single()` — під один проп (вузол ресурсу, дерево острова). Діаметр плями
## ≈ 2.2 радіуси сліду; лежить за нормаллю ґрунту, трохи над ним.

const MATERIAL := preload("res://materials/contact_ao.tres")
const LIFT := 0.05
const SPREAD := 2.2

static var _plane: PlaneMesh


static func plane() -> PlaneMesh:
	if _plane == null:
		_plane = PlaneMesh.new()
		_plane.size = Vector2.ONE
		_plane.material = MATERIAL
	return _plane


## MultiMesh плям: `spots` — [x, y, z, diameter, …] у локальних координатах `parent`,
## `normals` — нормаль ґрунту на кожну пляму (порожньо — усі догори).
static func multimesh(parent: Node3D, name: String, spots: PackedFloat32Array, normals: PackedVector3Array) -> MultiMeshInstance3D:
	var n := spots.size() / 4
	var mm := MultiMesh.new()
	mm.transform_format = MultiMesh.TRANSFORM_3D
	mm.mesh = plane()
	mm.instance_count = n
	for i: int in n:
		var d := spots[i * 4 + 3]
		var basis := Basis.IDENTITY
		if i < normals.size() and normals[i].y < 0.999:
			basis = Basis(Quaternion(Vector3.UP, normals[i]))
		mm.set_instance_transform(i, Transform3D(basis.scaled(Vector3(d, 1.0, d)), Vector3(spots[i * 4], spots[i * 4 + 1] + LIFT, spots[i * 4 + 2])))
	var inst := MultiMeshInstance3D.new()
	inst.name = name
	inst.multimesh = mm
	inst.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	parent.add_child(inst)
	return inst


## Одна пляма під пропом: діаметр у метрах, у локальних координатах `parent`.
static func single(parent: Node3D, diameter: float) -> MeshInstance3D:
	var old := parent.get_node_or_null("Contact")
	if old != null:
		old.queue_free()
	var mi := MeshInstance3D.new()
	mi.name = "Contact"
	mi.mesh = plane()
	mi.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	mi.transform = Transform3D(Basis.IDENTITY.scaled(Vector3(diameter, 1.0, diameter)), Vector3(0.0, LIFT, 0.0))
	parent.add_child(mi)
	return mi
