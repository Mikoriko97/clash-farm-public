class_name IslandWild
extends Node3D
## Дика рослинність острова (docs/ARCHITECTURE.md §Godot): трави, квіти,
## дрібні камені, поодинокі кущі й гриби під деревами — розкидані `Scatter`
## (синій шум з радіусами) по всій поточній площі острова, а не по колу
## стартового розміру. Перераховується, коли острів росте (група
## `island_extent`, `set_extents`) і після кожного `world_state` (грядки,
## декор): нічого не росте там, де стоїть колайдер хати, грядки, декору чи
## ресурсу (як у NavGrid), дерева-ресурси — крони для грибів. Кущі з
## `collider_radius_m` тверді, і NavGrid після перебудови обходить їх. Кущі Simple Trees
## кидають тінь гладкою кроною `shadow_crown` і мають відтінок екземпляра, як на материку.
## Види — `FloraKind` у масиві `kinds` (правляться в редакторі). Рішення власника
## 2026-09-16: базовий острів (стартові 48 м) правиться руками, куплені смуги —
## генеративно за правилами: `FloraKind.zones` «B» — лише базова ділянка, «X» — лише
## смуги розширення, порожньо — скрізь; `near_water_m` — лише в смузі шириною
## стільки метрів від краю форми (берегова рослинність іде за краєм, куди б він не
## посунувся). Форма — той самий заокруглений прямокутник, що й земля.

const PieceMesh := preload("res://scripts/piece_mesh.gd")
const MMBuffer := preload("res://scripts/multimesh_buffer.gd")
## A cleared spot keeps every wild plant this far from its centre (m): wider than the biggest
## bush footprint (1.35 × 1.2), so no neighbour candidate slides into the freed space.
const CLEAR_RADIUS_M := 1.7
const MAX_CLEARED := 500

@export var kinds: Array[FloraKind] = []
@export var seed: int = 11
## Під яким вузлом шукати колайдери-перешкоди (типово — вся сцена острова).
@export var obstacle_root: Node
## Запас навколо перешкод (м).
@export var clear_margin_m: float = 0.6
## Відступ від краю острова (м).
@export var margin_m: float = 0.8
## Матеріал землі острова: звідти заокруглення кутів для смуги «біля краю».
@export var ground_material: ShaderMaterial

var _min := Vector2(-24.0, -24.0)
var _max := Vector2(24.0, 24.0)
var _noise := FastNoiseLite.new()
var _signature := 0
var _pending: Array[int] = []
var _scatter: Scatter
var _rects: Array[Rect2] = []
var _piece_cache: Dictionary = {}
var _dirty := true
## Where the player cleared bushes (m, from `world_state.cleared`, server-stored 0023).
var _cleared: Array[Vector2] = []
## Solid kind file name → WildBush per collider shape (same order as the MultiMesh instances).
var _bushes: Dictionary = {}
## Останній розкид за назвою виду: [x, z, yaw, scale, …] (тести читають дані, не рендер —
## headless-рушій не зберігає трансформи MultiMesh).
var _placed: Dictionary = {}


func _ready() -> void:
	add_to_group(IslandShape.GROUP)
	_noise.noise_type = FastNoiseLite.TYPE_SIMPLEX_SMOOTH
	_noise.fractal_type = FastNoiseLite.FRACTAL_FBM
	_noise.fractal_octaves = 2
	_noise.frequency = 1.0
	_noise.seed = seed
	var bridge: Node = get_node_or_null(^"/root/Bridge")
	if bridge != null:
		bridge.message_received.connect(_on_bridge_message)


func _process(_delta: float) -> void:
	if _dirty:
		_dirty = false
		rebuild()
	elif not _pending.is_empty():
		_step()


func set_extents(new_min: Vector2, new_max: Vector2) -> void:
	_min = new_min
	_max = new_max
	_dirty = true


func _on_bridge_message(type: String, payload: Dictionary) -> void:
	if type == "world_state":
		if payload.has("cleared"):
			_set_cleared(payload["cleared"])
		_dirty = true


## `[[x_dm, z_dm], …]` from the server; malformed entries are skipped (rules/godot.md).
func _set_cleared(raw: Variant) -> void:
	var out: Array[Vector2] = []
	if typeof(raw) == TYPE_ARRAY:
		for entry: Variant in raw:
			if out.size() >= MAX_CLEARED:
				break
			if typeof(entry) != TYPE_ARRAY or (entry as Array).size() != 2:
				continue
			var x: Variant = (entry as Array)[0]
			var z: Variant = (entry as Array)[1]
			if (typeof(x) == TYPE_FLOAT or typeof(x) == TYPE_INT) and (typeof(z) == TYPE_FLOAT or typeof(z) == TYPE_INT):
				out.append(Vector2(float(x) / 10.0, float(z) / 10.0))
	_cleared = out


## A click hit one of the `WildBody_*` colliders (`shape_idx` = the cylinder in that body): walk up
## to that bush. False when the shape is no bush (the caller falls back to a plain walk).
func approach_bush(interaction: Interaction, body: Object, shape_idx: int) -> bool:
	var list: Array = _bushes.get(String((body as Node).name), []) if body is Node else []
	if shape_idx < 0 or shape_idx >= list.size():
		return false
	return interaction.walk_to_node(list[shape_idx] as Node3D)


## Скільки видів ще чекають своєї черги (по одному за кадр).
func pending() -> int:
	return _pending.size() + (1 if _dirty else 0)


## Розкид виду за назвою ресурсу (`w_bush`, `x_shore_grass`): [x, z, yaw, scale, …].
func placed(kind_name: String) -> PackedFloat32Array:
	return _placed.get(kind_name, PackedFloat32Array())


func placed_all() -> Dictionary:
	return _placed


func instance_count() -> int:
	var n := 0
	for child: Node in get_children():
		if child is MultiMeshInstance3D and not child.is_queued_for_deletion():
			n += (child as MultiMeshInstance3D).multimesh.instance_count
	return n


func rebuild() -> void:
	var scatter := Scatter.new()
	var rects := obstacle_rects(scatter)
	# Cleared spots are blockers with the biggest bush's footprint: what stood there stays gone
	# and its neighbours do not creep into the space (dart throwing rejects overlaps).
	for spot: Vector2 in _cleared:
		scatter.add(spot, CLEAR_RADIUS_M, false)
	# Every world_state marks us dirty; only a changed footprint set or island shape
	# is worth a rebuild (~150 ms native when done in one frame, hence one kind per frame).
	var signature := hash([rects, _min, _max, seed, _cleared])
	if signature == _signature and get_child_count() > 0 and _pending.is_empty():
		return
	_signature = signature
	FrameMark.mark("island wild rebuild")
	_placed.clear()
	_bushes.clear()
	for child: Node in get_children():
		child.queue_free()
	_scatter = scatter
	_rects = rects
	_pending.clear()
	for i: int in kinds.size():
		if kinds[i] != null and kinds[i].scene != null:
			_pending.append(i)
	_step()


## Один вид за кадр: розкид (синій шум) і MultiMesh; крони перших видів — якорі
## підліску наступних, тому порядок `kinds` зберігається.
func _step() -> void:
	var i: int = _pending.pop_front()
	var kind := kinds[i]
	FrameMark.mark("island wild %s" % kind.resource_path.get_file())
	var rect := Rect2(_min + Vector2.ONE * margin_m, _max - _min - Vector2.ONE * margin_m * 2.0)
	var pts := _scatter.sample(kind, i, seed, rect, Callable(self, "_density").bind(kind, _rects, _scatter))
	_placed[kind.resource_path.get_file().get_basename()] = pts
	if pts.size() >= 4:
		_add_kind(kind, pts)
	if _pending.is_empty():
		get_tree().call_group(NavGrid.GROUP, "mark_dirty")


func obstacle_rects(scatter: Scatter) -> Array[Rect2]:
	var rects: Array[Rect2] = []
	var root: Node = obstacle_root if obstacle_root != null else get_parent()
	for node: Node in root.find_children("*", "CollisionShape3D", true, false):
		var shape := node as CollisionShape3D
		var body := shape.get_parent() as StaticBody3D
		if shape.shape == null or shape.disabled or body == null or (body.collision_layer & 3) == 0 or is_ancestor_of(body):
			continue
		var box := NavGrid.world_aabb(shape).grow(clear_margin_m)
		# The ground slab and the invisible walls are not obstacles.
		if box.end.y < 0.1 or box.size.x > 40.0 or box.size.z > 40.0:
			continue
		rects.append(Rect2(box.position.x, box.position.z, box.size.x, box.size.z))
		var res := body.get_parent() as ResourceNode
		if res != null:
			scatter.add(Vector2(res.global_position.x, res.global_position.z), 1.2, res.kind == "tree")
	return rects


func _density(pos: Vector2, kind: FloraKind, rects: Array[Rect2], scatter: Scatter) -> float:
	for r: Rect2 in rects:
		if r.has_point(pos):
			return 0.0
	var in_base := absf(pos.x) <= IslandShape.BASE_HALF_M and absf(pos.y) <= IslandShape.BASE_HALF_M
	if kind.zones.contains("B") and not in_base:
		return 0.0
	if kind.zones.contains("X") and in_base:
		return 0.0
	if kind.near_water_m > 0.0 and edge_distance(pos) > kind.near_water_m:
		return 0.0
	var f := kind.density
	if kind.cluster > -1.0:
		f *= smoothstep(kind.cluster - 0.3, kind.cluster + 0.2, _noise.get_noise_2d(pos.x * kind.cluster_freq, pos.y * kind.cluster_freq))
	if kind.under_canopy_m > 0.0 and not scatter.near_anchor(pos, kind.under_canopy_m):
		return 0.0
	return f


## Відстань від точки всередині острова до його краю (заокруглений прямокутник, як у
## island_ground.gdshader); за краєм — відʼємна.
func edge_distance(pos: Vector2) -> float:
	var corner := 7.0
	if ground_material != null:
		corner = float(ground_material.get_shader_parameter("corner_radius"))
	var half := (_max - _min) * 0.5
	var p := (pos - (_min + _max) * 0.5).abs() - half + Vector2.ONE * corner
	var outside := Vector2(maxf(p.x, 0.0), maxf(p.y, 0.0)).length()
	var inside := minf(maxf(p.x, p.y), 0.0)
	return -(outside + inside - corner)


func _add_kind(kind: FloraKind, pts: PackedFloat32Array) -> void:
	var found := _piece(kind.scene, "")
	var mesh: Mesh = found.get("mesh")
	if mesh == null:
		return
	var xform: Transform3D = found.get("xform", Transform3D.IDENTITY)
	var crown := _piece(kind.scene, "shadow_crown")
	var with_crown := bool(crown.get("matched", false)) and kind.shadows
	var tinted := ChunkFlora.uses_tint(kind)
	var n := pts.size() / 4
	var buf := MMBuffer.new(n, tinted)
	var shadow_buf: MMBuffer = MMBuffer.new(n) if with_crown else null
	var body: StaticBody3D = null
	if kind.collider_radius_m > 0.0:
		body = StaticBody3D.new()
		body.name = "WildBody_" + kind.resource_path.get_file().get_basename()
	var spots := PackedFloat32Array()
	var proxies: Array[WildBush] = []
	for i: int in n:
		var s := pts[i * 4 + 3]
		var base := Vector3(pts[i * 4], -kind.sink_m, pts[i * 4 + 1])
		var placed := Transform3D(Basis(Vector3.UP, pts[i * 4 + 2]).scaled(Vector3(s, s, s)), base)
		buf.add(placed * xform, ChunkFlora.instance_tint(kind, base.x, base.z) if tinted else Color.WHITE)
		if shadow_buf != null:
			shadow_buf.add(placed * (crown["xform"] as Transform3D))
		if body != null:
			body.add_child(ChunkFlora.cylinder(kind, s, base))
			proxies.append(_add_bush_proxy(base, kind.collider_radius_m * s))
		if kind.radius_m >= 0.7:
			spots.append_array(PackedFloat32Array([base.x, 0.0, base.z, kind.radius_m * s * ContactShadow.SPREAD + 0.4]))
	var inst := MultiMeshInstance3D.new()
	inst.name = "Wild_" + kind.resource_path.get_file().get_basename()
	inst.multimesh = buf.build(mesh)
	if not kind.shadows or with_crown:
		inst.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	add_child(inst)
	if shadow_buf != null:
		var shadow := MultiMeshInstance3D.new()
		shadow.name = "WildShadow_" + kind.resource_path.get_file().get_basename()
		shadow.multimesh = shadow_buf.build(crown["mesh"])
		shadow.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_SHADOWS_ONLY
		add_child(shadow)
	if body != null:
		add_child(body)
		_bushes[body.name] = proxies
	if not spots.is_empty():
		ContactShadow.multimesh(self, "Contact_" + kind.resource_path.get_file().get_basename(), spots, PackedVector3Array())


## A clickable, targetable stand-in for one solid bush (the MultiMesh instance is not a node).
func _add_bush_proxy(base: Vector3, radius: float) -> WildBush:
	var bush := WildBush.new()
	bush.radius = radius
	bush.bush_id = WildBush.id_for(base.x, base.z)
	add_child(bush)
	bush.position = Vector3(base.x, 0.0, base.z)
	return bush


func _piece(scene: PackedScene, hint: String) -> Dictionary:
	var key := "%d:%s" % [scene.get_instance_id(), hint]
	if not _piece_cache.has(key):
		_piece_cache[key] = PieceMesh.first(scene, hint)
	return _piece_cache[key]
