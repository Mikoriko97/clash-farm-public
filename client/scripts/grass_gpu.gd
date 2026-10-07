class_name GrassGpu
extends MultiMeshInstance3D
## Трава без роботи CPU на пучок (етап 2, docs/PLAN.md): MultiMesh з N одиничними
## трансформами, а місце, висоту, дозвіл рости й затухання кожен пучок рахує сам
## у `shaders/grass_gpu.gdshader`. Тут лише: сітка (`side`, `cell_m` з густоти й
## радіуса), карти для шейдера (висоти й біоми материка або маска зайнятості
## острова), якір (персонаж на материку, центр острова на острові), кола-винятки,
## рівень якості (`apply_quality` множить густоту). Плитки й перебудови по
## 50–135 мс у кадрі зникли разом із ними.

const MMBuffer := preload("res://scripts/multimesh_buffer.gd")
const GROUP := "grass_field"
## Маска острова: 1 px = 1 м, покриває ±MASK_HALF_M.
const MASK_HALF_M := 96

## Материк: карти висот і біомів звідси; порожньо — острів (плоско, маска зайнятості).
@export var terrain: MainlandTerrain
## За ким іде сітка (персонаж). Порожньо — якір у центрі острова.
@export var follow: Node3D
@export var plot_grid: PlotGrid
## Матеріал землі острова (`materials/island_ground.tres`): звідти форма краю
## (заокруглення, ширина й глибина спуску, шум рваного краю), щоб трава повторювала землю.
@export var ground_material: ShaderMaterial
## Смуги без трави на острові (стежки): x/y — кут, розмір у метрах.
@export var exclude: Array[Rect2] = []
@export var plot_clear: float = 1.75
@export var island_margin_m: float = 0.4
## Радіус трави навколо якоря (материк). На острові — половина більшого боку.
@export var radius_m: float = 40.0
## Пучків на квадратний метр на «високій» якості.
@export var density: float = 24.0
@export var scale_min: float = 0.8
@export var scale_max: float = 1.3
## Кола без трави на материку: Vector3(x, z, радіус), край мʼякий на `edge_m`.
@export var exclusions: Array[Vector3] = []
@export var edge_m: float = 6.0
@export var blade_mesh: Mesh
## Базовий матеріал (`materials/grass/grass_gpu.tres`); вузол працює з копією.
@export var blade_material: ShaderMaterial

var box_min: Vector2 = Vector2(-24.0, -24.0)
var box_max: Vector2 = Vector2(24.0, 24.0)
var _material: ShaderMaterial
var _side := 0
var _density_scale := 1.0
var _mask: Image


func _ready() -> void:
	add_to_group(GROUP)
	add_to_group(IslandShape.GROUP)
	add_to_group("quality")
	cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	if blade_material != null:
		_material = blade_material.duplicate() as ShaderMaterial
		material_override = _material
	if multimesh == null:
		multimesh = MultiMesh.new()
		multimesh.transform_format = MultiMesh.TRANSFORM_3D
	multimesh.mesh = blade_mesh
	# Автолоад Quality шукається вузлом, не глобальним іменем: headless-інструменти автолоадів не бачать.
	var quality: Node = get_node_or_null(^"/root/Quality")
	if quality != null:
		_density_scale = float(quality.call("grass_scale"))
		if terrain != null:
			radius_m = float(((quality.call("current") as Dictionary).get("view", {}) as Dictionary).get("grass_m", radius_m))
	_set_maps()
	rebuild()


func instance_count() -> int:
	return multimesh.instance_count if multimesh != null else 0


## Острів виріс (IslandShape): нова сітка й маска.
func set_extents(new_min: Vector2, new_max: Vector2) -> void:
	box_min = new_min
	box_max = new_max
	if is_inside_tree() and terrain == null:
		rebuild()


## Грядки змінились (PlotGrid кличе групу): лише маска, сітка та сама.
func scatter() -> void:
	if terrain == null and is_inside_tree():
		_build_mask()


func apply_quality(q: Dictionary) -> void:
	var scale := float(q.get("grass", 1.0))
	var radius := float((q.get("view", {}) as Dictionary).get("grass_m", radius_m)) if terrain != null else radius_m
	if not is_equal_approx(scale, _density_scale) or not is_equal_approx(radius, radius_m):
		_density_scale = scale
		radius_m = radius
		rebuild()


func _process(_delta: float) -> void:
	if follow != null and _material != null:
		var p := follow.global_position
		_material.set_shader_parameter("anchor", Vector2(p.x, p.z))


func rebuild() -> void:
	FrameMark.mark("grass gpu rebuild")
	var radius := radius_m
	if terrain == null:
		radius = maxf(box_max.x - box_min.x, box_max.y - box_min.y) * 0.5
		var centre := (box_min + box_max) * 0.5
		_material.set_shader_parameter("anchor", centre)
		_material.set_shader_parameter("island_centre", centre)
		_material.set_shader_parameter("half_extent", (box_max - box_min) * 0.5)
		_build_mask()
	var side := maxi(1, int(ceil(radius * 2.0 * sqrt(density * _density_scale))))
	_material.set_shader_parameter("side", side)
	_material.set_shader_parameter("cell_m", radius * 2.0 / float(side))
	if side != _side:
		_side = side
		_fill_identity(side * side)
	var half := radius + 2.0
	var c := Vector3(0.0, 0.0, 0.0) if terrain != null else Vector3((box_min.x + box_max.x) * 0.5, 0.0, (box_min.y + box_max.y) * 0.5)
	# Instances are all at the origin: the real extent is what the shader places.
	custom_aabb = AABB(c + Vector3(-half - 600.0 * float(terrain != null), -20.0, -half - 600.0 * float(terrain != null)),
		Vector3(2.0 * half + 1200.0 * float(terrain != null), 120.0, 2.0 * half + 1200.0 * float(terrain != null)))


## N одиничних трансформів одним буфером: шаблон з 12 float подвоюється, не цикл по N.
func _fill_identity(n: int) -> void:
	var pattern := PackedFloat32Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0])
	var data := pattern.duplicate()
	while data.size() < n * 12:
		data.append_array(data)
	data.resize(n * 12)
	multimesh.instance_count = n
	multimesh.buffer = data


func _set_maps() -> void:
	if _material == null:
		return
	if terrain != null:
		_material.set_shader_parameter("use_height", true)
		_material.set_shader_parameter("mask_mode", 0)
		_material.set_shader_parameter("height_tex", terrain.height_texture())
		_material.set_shader_parameter("mask_tex", terrain.biome_texture())
		_material.set_shader_parameter("map_origin", Vector2(MainlandTerrain.ORIGIN, MainlandTerrain.ORIGIN))
		_material.set_shader_parameter("map_size", float(MainlandTerrain.SIZE))
		var circles: Array[Vector4] = []
		for e: Vector3 in exclusions:
			circles.append(Vector4(e.x, e.y, e.z, edge_m))
		while circles.size() < 4:
			circles.append(Vector4.ZERO)
		_material.set_shader_parameter("exclusions", circles)
		_material.set_shader_parameter("exclusion_count", mini(exclusions.size(), 4))
	else:
		_material.set_shader_parameter("use_height", false)
		_material.set_shader_parameter("mask_mode", 1)
		_material.set_shader_parameter("map_origin", Vector2(-MASK_HALF_M, -MASK_HALF_M))
		_material.set_shader_parameter("map_size", float(MASK_HALF_M * 2))
		_material.set_shader_parameter("exclusion_count", 0)
		# Якір у центрі острова: без затухання, інакше краї гаснуть круглою плямою.
		_material.set_shader_parameter("fade_start", 1.0)
		if ground_material != null:
			for key: String in ["corner_radius", "edge_width", "edge_drop", "noise_scale"]:
				_material.set_shader_parameter(key, ground_material.get_shader_parameter(key))
			_material.set_shader_parameter("edge_noise", ground_material.get_shader_parameter("noise_tex"))
	_material.set_shader_parameter("scale_min", scale_min)
	_material.set_shader_parameter("scale_max", scale_max)


## Маска зайнятості острова: біле — вільно. Заповнюється прямокутниками, не попіксельно.
func _build_mask() -> void:
	var size := MASK_HALF_M * 2
	if _mask == null:
		_mask = Image.create(size, size, false, Image.FORMAT_L8)
	_mask.fill(Color.BLACK)
	# The island's rounded shape lives in the shader (same SDF as the ground); the mask
	# only carries what the shader cannot know: plots and paths.
	_mask.fill_rect(_rect_px(Rect2(box_min - Vector2.ONE * island_margin_m, box_max - box_min + Vector2.ONE * island_margin_m * 2.0)), Color.WHITE)
	for r: Rect2 in exclude:
		_mask.fill_rect(_rect_px(r), Color.BLACK)
	if plot_grid != null:
		for row: int in range(-PlotGrid.CELL_MAX, PlotGrid.CELL_MAX + 1):
			for col: int in range(-PlotGrid.CELL_MAX, PlotGrid.CELL_MAX + 1):
				if plot_grid.cell_taken(col, row):
					var c := Vector2(col * PlotGrid.CELL_M, row * PlotGrid.CELL_M)
					_mask.fill_rect(_rect_px(Rect2(c - Vector2.ONE * plot_clear, Vector2.ONE * plot_clear * 2.0)), Color.BLACK)
	_material.set_shader_parameter("mask_tex", ImageTexture.create_from_image(_mask))


## Клітинка (x, z) світу → піксель маски; шейдер бере texel floor(x + MASK_HALF_M).
static func _rect_px(r: Rect2) -> Rect2i:
	var x0 := floori(r.position.x + MASK_HALF_M)
	var z0 := floori(r.position.y + MASK_HALF_M)
	var x1 := ceili(r.end.x + MASK_HALF_M)
	var z1 := ceili(r.end.y + MASK_HALF_M)
	return Rect2i(x0, z0, maxi(x1 - x0, 0), maxi(z1 - z0, 0))
