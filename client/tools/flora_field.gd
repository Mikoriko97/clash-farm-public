extends RefCounted
## Модель густини для запікання флори материка (`tools/bake_flora.gd`,
## docs/MAINLAND.md §9): що ще, крім відстаней, вирішує, чи росте вид у точці.
## Множники (0…1) перемножуються: зона з таблиці, виключення (орієнтири, дороги),
## мʼякий поріг ваги біому (узлісся рідшає поступово), кластерний шум
## (гаї й галявини), висота над водою, крутизна, близькість води, «під кроною».

const SEA := 0.3
const WCELL := 4.0
const WN := 256
const ORIGIN := -512.0
const CHUNK := 64.0

var terrain: Node3D
var rows: Array = []
## Кола виключення: Vector3(x, z, радіус).
var exclusions: Array = []
var noise := FastNoiseLite.new()
var scatter: Scatter
## Маска пензля зон (RGBA8, `PAINT_PX` px на всю карту): 0.5 — як є, 0 — нічого, 1 — удвічі.
var paint: Image
const PAINT_PX := 512
## Байт 128 — рівно ×1 (0.5 у float не дає цілого байта); нижче — до ×0, вище — до ×2.
const NEUTRAL := 128.0 / 255.0
const LAYER_CHANNEL := {"canopy": 0, "shrub": 1, "ground": 2, "detail": 3}
var _water := PackedFloat32Array()


func setup(t: Node3D, zone_rows: Array, circles: Array, sc: Scatter, seed: int) -> void:
	terrain = t
	rows = zone_rows
	exclusions = circles
	scatter = sc
	noise.noise_type = FastNoiseLite.TYPE_SIMPLEX_SMOOTH
	noise.fractal_type = FastNoiseLite.FRACTAL_FBM
	noise.fractal_octaves = 3
	noise.frequency = 1.0
	noise.seed = seed
	_bake_water()


## Відстань до води (м) на сітці 4 м: пошук у ширину від усіх водних клітинок.
func _bake_water() -> void:
	_water.resize(WN * WN)
	_water.fill(1.0e9)
	var queue := PackedInt32Array()
	for j: int in WN:
		for i: int in WN:
			if float(terrain.height_at(ORIGIN + (i + 0.5) * WCELL, ORIGIN + (j + 0.5) * WCELL)) < SEA:
				_water[j * WN + i] = 0.0
				queue.append(j * WN + i)
	var head := 0
	while head < queue.size():
		var idx := queue[head]
		head += 1
		var d := _water[idx] + WCELL
		var i := idx % WN
		var j := idx / WN
		for nb: Vector2i in [Vector2i(i - 1, j), Vector2i(i + 1, j), Vector2i(i, j - 1), Vector2i(i, j + 1)]:
			if nb.x < 0 or nb.y < 0 or nb.x >= WN or nb.y >= WN:
				continue
			var k := nb.y * WN + nb.x
			if _water[k] > d:
				_water[k] = d
				queue.append(k)


func water_dist(x: float, z: float) -> float:
	var i := clampi(int((x - ORIGIN) / WCELL), 0, WN - 1)
	var j := clampi(int((z - ORIGIN) / WCELL), 0, WN - 1)
	return _water[j * WN + i]


func zone_at(x: float, z: float) -> String:
	var cx := int((x - ORIGIN) / CHUNK)
	var cz := int((z - ORIGIN) / CHUNK)
	if cz < 0 or cz >= rows.size():
		return "W"
	var row := str(rows[cz])
	if cx < 0 or cx >= row.length():
		return "W"
	return row[cx]


func excluded(x: float, z: float) -> bool:
	for c: Vector3 in exclusions:
		var dx := x - c.x
		var dz := z - c.y
		if dx * dx + dz * dz < c.z * c.z:
			return true
	return false


static func biome_weight(kind: FloraKind, w: Color) -> float:
	match kind.biome:
		"meadow":
			return w.r
		"forest":
			return w.g
		"rock":
			return w.b
		"sand":
			return w.a
	return 1.0


## Густота виду в точці, 0…1 (для `Scatter.sample`, звʼязується через bind).
## Множник намальованої зони для шару виду в точці (1.0 без маски).
func paint_mul(pos: Vector2, layer: String) -> float:
	if paint == null:
		return 1.0
	var px := clampi(int((pos.x - ORIGIN) / (CHUNK * 16.0) * PAINT_PX), 0, PAINT_PX - 1)
	var pz := clampi(int((pos.y - ORIGIN) / (CHUNK * 16.0) * PAINT_PX), 0, PAINT_PX - 1)
	var c := paint.get_pixel(px, pz)
	var b := roundi(float([c.r, c.g, c.b, c.a][int(LAYER_CHANNEL.get(layer, 2))]) * 255.0)
	return b / 128.0 if b <= 128 else 1.0 + float(b - 128) / 127.0


func density(pos: Vector2, kind: FloraKind) -> float:
	var zone := zone_at(pos.x, pos.y)
	if zone == "W":
		return 0.0
	if kind.zones.is_empty():
		if zone == "C":
			return 0.0
	elif not kind.zones.contains(zone):
		return 0.0
	if excluded(pos.x, pos.y):
		return 0.0
	var w: Color = terrain.biome_at(pos.x, pos.y)
	var f := kind.density * paint_mul(pos, kind.layer)
	if f <= 0.001:
		return 0.0
	if kind.biome != "any":
		f *= smoothstep(kind.biome_min - 0.15, kind.biome_min + 0.15, biome_weight(kind, w))
		# Roads and beaches are sand: nothing but shore plants roots there.
		if kind.biome != "sand" and w.a > 0.5:
			return 0.0
	if f <= 0.001:
		return 0.0
	if kind.cluster > -1.0:
		var v := noise.get_noise_2d(pos.x * kind.cluster_freq, pos.y * kind.cluster_freq)
		f *= smoothstep(kind.cluster - 0.3, kind.cluster + 0.2, v)
		if f <= 0.001:
			return 0.0
	if kind.near_water_m > 0.0 and water_dist(pos.x, pos.y) > kind.near_water_m:
		return 0.0
	if kind.under_canopy_m > 0.0 and not scatter.near_anchor(pos, kind.under_canopy_m):
		f *= 0.06
	if float(terrain.height_at(pos.x, pos.y)) < kind.above_sea_m:
		return 0.0
	var nrm: Vector3 = terrain.normal_at(pos.x, pos.y)
	if nrm.y < cos(deg_to_rad(kind.slope_max_deg)):
		return 0.0
	return f
