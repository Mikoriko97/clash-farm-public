@tool
extends Node3D
## Карта висот і біомів материка (1 px = 1 м, 1024 × 1024, центр карти — центр світу):
## висота, нормаль, найнижча точка під основою, ваги біомів у будь-якій точці й ті самі
## карти текстурами для GPU (вода, трава). Основа `MainlandTerrain`, який стрімить чанки
## поверх неї; усе розміщення (вузли, флора, таблички) питає висоту в терену.

const SIZE := 1024
const ORIGIN := -512.0

## Written by tools/import_height.gd: FORMAT_RF metres and RGBA8 biome weights.
@export var height_image: Image
@export var biome_image: Image

var _heights := PackedFloat32Array()
var _biome := PackedByteArray()
var _height_tex: ImageTexture
var _biome_tex: ImageTexture


## Unpacks both maps for per-pixel reads; called from `_ready` of the terrain.
func load_maps() -> void:
	if height_image != null:
		_heights = height_image.get_data().to_float32_array()
	if biome_image != null:
		_biome = biome_image.get_data()


## Height at integer pixel (px, pz), clamped to the map.
func height_px(px: int, pz: int) -> float:
	if _heights.is_empty():
		return 0.0
	return _heights[clampi(pz, 0, SIZE - 1) * SIZE + clampi(px, 0, SIZE - 1)]


## Bilinear height at world (x, z).
func height_at(x: float, z: float) -> float:
	var fx := x - ORIGIN
	var fz := z - ORIGIN
	var px := floori(fx)
	var pz := floori(fz)
	var tx := fx - px
	var tz := fz - pz
	var top := lerpf(height_px(px, pz), height_px(px + 1, pz), tx)
	var bottom := lerpf(height_px(px, pz + 1), height_px(px + 1, pz + 1), tx)
	return lerpf(top, bottom, tz)


## Lowest height within `r` metres of (x, z): what a prop must sit on so nothing floats downhill.
func ground_min(x: float, z: float, r: float) -> float:
	var y := height_at(x, z)
	if r <= 0.0:
		return y
	for d: Vector2 in [Vector2(r, 0.0), Vector2(-r, 0.0), Vector2(0.0, r), Vector2(0.0, -r)]:
		y = minf(y, height_at(x + d.x, z + d.y))
	return y


## Ground normal at world (x, z) from a 2 m stencil.
func normal_at(x: float, z: float) -> Vector3:
	var dx := height_at(x + 1.0, z) - height_at(x - 1.0, z)
	var dz := height_at(x, z + 1.0) - height_at(x, z - 1.0)
	return Vector3(-dx, 2.0, -dz).normalized()


func normal_px(px: int, pz: int, step: int) -> Vector3:
	var dx := height_px(px + step, pz) - height_px(px - step, pz)
	var dz := height_px(px, pz + step) - height_px(px, pz - step)
	return Vector3(-dx, 2.0 * step, -dz).normalized()


## Biome weights (R meadow, G forest, B rock, A sand) at integer pixel.
func biome_px(px: int, pz: int) -> Color:
	if _biome.is_empty():
		return Color(1.0, 0.0, 0.0, 0.0)
	var i := (clampi(pz, 0, SIZE - 1) * SIZE + clampi(px, 0, SIZE - 1)) * 4
	return Color8(_biome[i], _biome[i + 1], _biome[i + 2], _biome[i + 3])


func biome_at(x: float, z: float) -> Color:
	return biome_px(floori(x - ORIGIN), floori(z - ORIGIN))


## Карта висот (FORMAT_RF, метри) як текстура: вода й трава на GPU читають її самі.
func height_texture() -> ImageTexture:
	if _height_tex == null and height_image != null:
		_height_tex = ImageTexture.create_from_image(height_image)
	return _height_tex


## Ваги біомів (RGBA8) як текстура для трави на GPU.
func biome_texture() -> ImageTexture:
	if _biome_tex == null and biome_image != null:
		_biome_tex = ImageTexture.create_from_image(biome_image)
	return _biome_tex
