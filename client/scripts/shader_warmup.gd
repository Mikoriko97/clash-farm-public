class_name ShaderWarmup
extends Node3D
## Прогрів шейдерів (docs/MAINLAND.md §9): у web-збірці (Compatibility, WebGL)
## шейдер матеріалу компілюється синхронно в кадрі, коли обʼєкт уперше
## потрапляє в камеру — тому перша зустріч із лісом «зависала» на секунди.
## Тут на старті сцени кожен вид вузла і флори (плюс `extra`) на кілька
## кадрів стає крихітною цяткою перед камерою: і звичайний, і MultiMesh
## варіанти шейдера компілюються за завісою переходу, а не на прогулянці.

@export var nodes: MainlandNodes
@export var terrain: MainlandTerrain
@export var extra: Array[PackedScene] = []
## Скільки кадрів тримати цятки (тінь і основний прохід мають їх побачити).
@export var frames: int = 3
## Масштаб цяток: видно лише сірий пил, шейдер компілюється однаково.
@export var speck_scale: float = 0.01
## Скільки сцен показувати за кадр: усі разом давали один довгий кадр (компіляція
## кожного шейдера синхронна), порціями — кілька коротших.
@export var per_frame: int = 6

var _left := -1
var _done := false
var _pending: Array[PackedScene] = []
var _next := 0


func _process(_delta: float) -> void:
	if _done:
		return
	var cam := get_viewport().get_camera_3d()
	if cam == null:
		return
	if _left < 0:
		if _pending.is_empty():
			_pending = _scenes()
		_spawn(cam)
		if _next < _pending.size():
			return
		_left = frames
		return
	_left -= 1
	if _left <= 0:
		for child: Node in get_children():
			child.queue_free()
		_done = true
		set_process(false)


func warmed() -> int:
	return get_child_count()


## Усі порції показані й цятки прибрані.
func is_done() -> bool:
	return _done


func _scenes() -> Array[PackedScene]:
	var out: Array[PackedScene] = []
	if nodes != null:
		for scene: Variant in nodes.kinds.values():
			if scene is PackedScene and not out.has(scene):
				out.append(scene)
		if nodes.fallback_scene != null and not out.has(nodes.fallback_scene):
			out.append(nodes.fallback_scene)
	if terrain != null:
		for kind: FloraKind in terrain.flora:
			if kind != null and kind.scene != null and not out.has(kind.scene):
				out.append(kind.scene)
	for scene: PackedScene in extra:
		if scene != null and not out.has(scene):
			out.append(scene)
	# Вбрання сусідів (ADR-026): гравець без вбрання ще не показав його матеріалів, і перший сусід у куртці смикав кадр.
	# Тіла сусідів мають ту саму палітру, що й гравець, — їх гріти нема чого.
	var outfits := CharacterBody.outfit_path_of(CharacterBody.MALE)
	if ResourceLoader.exists(outfits):
		var scene := load(outfits) as PackedScene
		if scene != null and not out.has(scene):
			out.append(scene)
	return out


## Наступна порція `per_frame` сцен перед камерою.
func _spawn(cam: Camera3D) -> void:
	FrameMark.mark("shader warmup %d/%d" % [_next, _pending.size()])
	var stop := mini(_next + per_frame, _pending.size())
	for i: int in range(_next, stop):
		var scene: PackedScene = _pending[i]
		var inst := scene.instantiate() as Node3D
		if inst == null:
			continue
		var spot: Vector3 = cam.global_position + cam.global_basis * Vector3(float(i % 8 - 4) * 0.3, -0.8 + float(i / 8) * 0.2, -5.0)
		add_child(inst)
		inst.global_transform = Transform3D(Basis.IDENTITY.scaled(Vector3.ONE * speck_scale), spot)
		# Every level of detail (Simple Trees switch them by distance) in both the
		# plain and the MultiMesh variant, whatever the visibility range says.
		var meshes: Array[Node] = inst.find_children("*", "MeshInstance3D", true, false)
		for k: int in meshes.size():
			var mesh_node := meshes[k] as MeshInstance3D
			mesh_node.visibility_range_begin = 0.0
			mesh_node.visibility_range_end = 0.0
			var mm := MultiMesh.new()
			mm.transform_format = MultiMesh.TRANSFORM_3D
			mm.mesh = mesh_node.mesh
			mm.instance_count = 1
			mm.set_instance_transform(0, Transform3D.IDENTITY)
			var mmi := MultiMeshInstance3D.new()
			mmi.multimesh = mm
			add_child(mmi)
			mmi.global_transform = Transform3D(Basis.IDENTITY.scaled(Vector3.ONE * speck_scale), spot + cam.global_basis.x * (0.15 + 0.1 * float(k)))
	_next = stop
