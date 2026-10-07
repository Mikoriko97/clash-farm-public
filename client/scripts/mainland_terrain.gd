@tool
class_name MainlandTerrain
extends "res://scripts/mainland_heightfield.gd"
## Streams the mainland ground around the character (docs/MAINLAND.md §9):
## 64 m chunks from a baked height map, LOD 0 (2 m grid + collider) within
## `lod0_radius`, LOD 1 (8 m grid) out to `load_radius`, freed past
## `unload_radius`; one chunk built per frame. Reports the character's chunk
## to React (`player_chunk`) so the shell can fetch that area's resource nodes.

signal chunk_unloaded(cx: int, cz: int)
## The character moved into another chunk (also sent to React as `player_chunk`).
signal centre_changed(centre: Vector2i)

## Preloaded (not just named) so the export includes the script: exported builds
## only ship what a scene or a preload references.
const CHUNK_SCENE_SCRIPT := preload("res://scripts/terrain_chunk.gd")
const PieceMesh := preload("res://scripts/piece_mesh.gd")

const CHUNK := 64
const CHUNKS := 16
## Порогі поверхонь для кроків, як у `mainland_ground.gdshader`: нормаль.y < 0,72 — урвище (скеля), сніг набирає силу на `snow_line` + 1 м
## (замовчування — `snow_line` шейдера), нижче 0,5 м над морем — мокрий пісок узбережжя.
const CLIFF_UP := 0.72
const SNOW_LINE_DEFAULT := 42.0
const SNOW_MARGIN := 1.0
const SHORE_Y := 0.5

@export var ground_material: Material
## Вода (materials/water_shore_mainland.tres): отримує карту висот для піни й мілин.
@export var sea_material: ShaderMaterial
@export var player: Node3D
@export_range(1, 6) var load_radius: int = 3
@export_range(0, 3) var lod0_radius: int = 1
@export_range(2, 8) var unload_radius: int = 5
@export var lod0_step: int = 2
@export var lod1_step: int = 4
@export var chunks_per_frame: int = 1
## Види флори (`scenes/mainland/flora/*.tres`) у порядку шарів: спершу крони, потім підлісок і трави.
## Їх розстановку запікає `tools/bake_flora.gd` у `flora_bake_path`; тут лише будуються MultiMesh.
@export var flora: Array[FloraKind] = []
@export var flora_bake_path: String = "res://assets/mainland/flora.res"
## Скільки видів флори будувати за кадр (один вид = один MultiMesh чанка), щоб не було ривків.
@export var flora_kinds_per_frame: int = 4
## Лише в редакторі: показати всю карту (усі чанки LOD 1) з запеченою флорою, щоб
## бачити результат пензля зон і запікання без запуску гри. У збірку не потрапляє.
@export var editor_preview: bool = false:
	set(v):
		editor_preview = v
		if Engine.is_editor_hint() and is_inside_tree():
			_set_preview(v)

var flora_bake: FloraBake
var _piece_cache: Dictionary = {}
## [chunk, kind index] pairs still to scatter.
var _flora_queue: Array = []

var _chunks: Dictionary = {}
var _queue: Array[Vector3i] = []
var _centre := Vector2i(-1000, -1000)


func _ready() -> void:
	load_maps()
	if not _heights.is_empty() and sea_material != null and not Engine.is_editor_hint():
		sea_material.set_shader_parameter("height_tex", height_texture())
	if ResourceLoader.exists(flora_bake_path):
		flora_bake = load(flora_bake_path) as FloraBake
	if flora_bake == null and not flora.is_empty():
		push_warning("MainlandTerrain: no flora bake at %s (run tools/bake_flora.gd)" % flora_bake_path)
	if Engine.is_editor_hint() and editor_preview:
		_set_preview.call_deferred(true)
	if not Engine.is_editor_hint():
		add_to_group(Footsteps.PROVIDER_GROUP)
		add_to_group("quality")
		var quality: Node = get_node_or_null(^"/root/Quality")
		if quality != null:
			apply_quality(quality.call("current"))


func _process(_delta: float) -> void:
	if Engine.is_editor_hint() or player == null or _heights.is_empty():
		return
	var centre := chunk_of(player.global_position)
	if centre != _centre:
		_centre = centre
		_replan()
		centre_changed.emit(centre)
		var bridge: Node = get_node_or_null(^"/root/Bridge")
		if bridge != null:
			bridge.send("player_chunk", {"cx": centre.x, "cz": centre.y})
	# A chunk mesh (~2 ms native) and a flora batch never share a frame: in wasm the
	# pair crossed a 75 Hz frame, one alone fits.
	var built := 0
	for _i: int in chunks_per_frame:
		if _queue.is_empty():
			break
		var job: Vector3i = _queue.pop_front()
		_build(Vector2i(job.x, job.y), job.z)
		built += 1
	if built == 0:
		_scatter_flora()


## Перегляд у редакторі: усі 16×16 чанків LOD 1 з усією запеченою флорою (без
## колайдерів); вузли не мають власника, тож сцена їх не зберігає.
func _set_preview(on: bool) -> void:
	for key: Variant in _chunks.keys():
		_drop(key, false)
	_chunks.clear()
	_flora_queue.clear()
	if not on or _heights.is_empty():
		return
	flora_bake = load(flora_bake_path) as FloraBake if ResourceLoader.exists(flora_bake_path) else null
	for cz: int in CHUNKS:
		for cx: int in CHUNKS:
			var c := Vector2i(cx, cz)
			var node: StaticBody3D = CHUNK_SCENE_SCRIPT.new()
			add_child(node)
			node.build(self, cx, cz, lod1_step, false)
			_chunks[c] = node
			if flora_bake == null:
				continue
			for i: int in flora.size():
				var data := flora_bake.chunk_data(c, i)
				if data.size() >= 4:
					ChunkFlora.build_kind(node, self, flora[i], data)


## Перезавантажити запечену флору (після кнопки «Запекти» в редакторі).
func reload_preview() -> void:
	_piece_cache.clear()
	if Engine.is_editor_hint() and editor_preview:
		_set_preview(true)


static func chunk_origin(c: int) -> float:
	return ORIGIN + float(c) * CHUNK


static func chunk_of(pos: Vector3) -> Vector2i:
	return Vector2i(floori((pos.x - ORIGIN) / CHUNK), floori((pos.z - ORIGIN) / CHUNK))


static func in_range(c: Vector2i) -> bool:
	return c.x >= 0 and c.y >= 0 and c.x < CHUNKS and c.y < CHUNKS


## Рівень якості: радіуси стрімінгу (`view`), перепланування з наступного кадру.
func apply_quality(q: Dictionary) -> void:
	var view: Dictionary = q.get("view", {})
	if view.is_empty():
		return
	load_radius = clampi(int(view.get("load", load_radius)), 1, 6)
	lod0_radius = clampi(int(view.get("lod0", lod0_radius)), 0, 3)
	unload_radius = clampi(int(view.get("unload", unload_radius)), load_radius + 1, 8)
	_centre = Vector2i(-1000, -1000)


func loaded_count() -> int:
	return _chunks.size()


func chunk(c: Vector2i) -> StaticBody3D:
	return _chunks.get(c)


func pending_count() -> int:
	return _queue.size()


## Queue every wanted chunk whose LOD is wrong or missing; free the far ones.
func _replan() -> void:
	_queue.clear()
	var wanted: Dictionary = {}
	for dz: int in range(-load_radius, load_radius + 1):
		for dx: int in range(-load_radius, load_radius + 1):
			var c := _centre + Vector2i(dx, dz)
			if not in_range(c):
				continue
			var lod := 0 if maxi(absi(dx), absi(dz)) <= lod0_radius else 1
			wanted[c] = lod
			var existing: StaticBody3D = _chunks.get(c)
			if existing == null or int(existing.get("lod")) != lod:
				_queue.append(Vector3i(c.x, c.y, lod))
	_queue.sort_custom(func(a: Vector3i, b: Vector3i) -> bool:
		return _centre.distance_squared_to(Vector2i(a.x, a.y)) < _centre.distance_squared_to(Vector2i(b.x, b.y)))
	for key: Vector2i in _chunks.keys():
		if wanted.has(key):
			continue
		var d := key - _centre
		if maxi(absi(d.x), absi(d.y)) > unload_radius:
			_drop(key)


func _build(c: Vector2i, lod: int) -> void:
	if _chunks.has(c):
		_drop(c, false)
	FrameMark.mark("chunk (%d,%d) lod %d build" % [c.x, c.y, lod])
	var node: StaticBody3D = CHUNK_SCENE_SCRIPT.new()
	add_child(node)
	node.build(self, c.x, c.y, lod0_step if lod == 0 else lod1_step, lod == 0)
	_chunks[c] = node
	if flora_bake != null:
		for i: int in flora.size():
			if (lod == 0 or flora[i].far_chunks) and flora_bake.chunk_data(c, i).size() >= 4:
				_flora_queue.append([node, i])


func _drop(c: Vector2i, announce: bool = true) -> void:
	var node: StaticBody3D = _chunks.get(c)
	if node == null:
		return
	_chunks.erase(c)
	node.queue_free()
	if announce:
		chunk_unloaded.emit(c.x, c.y)


## One baked flora kind on one chunk per call, `flora_kinds_per_frame` calls per
## frame; jobs of chunks that were dropped or rebuilt meanwhile are skipped.
func _scatter_flora() -> void:
	var left := flora_kinds_per_frame
	while left > 0 and not _flora_queue.is_empty():
		var job: Array = _flora_queue.pop_front()
		if not is_instance_valid(job[0]):
			continue
		var chunk: Node3D = job[0]
		var c := Vector2i(int(chunk.get("cx")), int(chunk.get("cz")))
		if _chunks.get(c) != chunk:
			continue
		var data := flora_bake.chunk_data(c, job[1])
		FrameMark.mark("flora %s on (%d,%d): %d inst" % [flora[job[1]].resource_path.get_file(), c.x, c.y, data.size() / 4])
		ChunkFlora.build_kind(chunk, self, flora[job[1]], data)
		left -= 1


## Mesh of a piece scene with its transform, cached per scene (flora instances it thousands of times).
func piece_mesh(scene: PackedScene, hint: String = "") -> Dictionary:
	var key := "%d:%s" % [scene.get_instance_id(), hint]
	if not _piece_cache.has(key):
		_piece_cache[key] = PieceMesh.first(scene, hint)
	return _piece_cache[key]


func flora_pending() -> int:
	return _flora_queue.size()


## Поверхня для кроків (`Footsteps`) так, як її фарбує `mainland_ground.gdshader`: див. `surface_for`.
func surface_at(position: Vector3) -> String:
	var snow_line := SNOW_LINE_DEFAULT
	var shader := ground_material as ShaderMaterial
	if shader != null and shader.get_shader_parameter("snow_line") != null:
		snow_line = float(shader.get_shader_parameter("snow_line"))
	return surface_for(height_at(position.x, position.z), normal_at(position.x, position.z).y, biome_at(position.x, position.z), snow_line)


## Поверхня під ногами за висотою, нормаллю (`up` — її y) і вагами біомів (R луки, G ліс, B скеля, A пісок): урвище — камінь,
## вище лінії снігу — сніг, узбережжя й піщаний біом — пісок, скеля — камінь, решта — трава. Пороги дзеркалять шейдер землі.
static func surface_for(height: float, up: float, biome: Color, snow_line: float) -> String:
	if up < CLIFF_UP:
		return "stone"
	if height >= snow_line + SNOW_MARGIN:
		return "snow"
	if height < SHORE_Y or biome.a > 0.5:
		return "sand"
	if biome.b > 0.5:
		return "stone"
	return "grass"
