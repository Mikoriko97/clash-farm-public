class_name MeshLod
extends RefCounted
## Рівні деталізації (LOD) мешів персонажа (ADR-028). Godot сам будує LOD при імпорті (`generate_lods`), але:
## 1. меш, який ми перебудовуємо (тіло з перемапованими UV), втрачав їх — `add_surface_from_arrays` без `lods` дає лише LOD 0,
##    тож кожен персонаж малювався 10 644 трикутниками за будь-якої відстані;
## 2. для скінних GLB з Blender (жіноче тіло, вбрання) імпортер рахує похибку LOD у просторі кісток арматури, що у Blender
##    сантиметрова (масштаб 0,01): `edge_length` виходить у ~100 разів завеликою, і рівні не вмикаються взагалі
##    (куртка лишалась 3 658 трикутників на 120 м). Тут ключі множаться на `GLB_SCALE`.
## Індекси LOD читаються з `RenderingServer.mesh_get_surface` і віддаються назад у `add_surface_from_arrays`.

## Поправка похибки LOD скінних GLB із Blender (армату в сантиметрах).
const GLB_SCALE := 0.01


## Копія `source` (усі поверхні, матеріали, імена) із LOD, ключі яких помножено на `scale`. `edit(surface, arrays)`
## — необовʼязкова правка масивів поверхні (перемапування UV) до збирання. Окремого тіньового меша імпорт скінних мешів не дає (`shadow_mesh` = null) — тінь малює сам меш зі своїми LOD.
static func rebuilt(source: Mesh, scale: float = 1.0, edit: Callable = Callable()) -> ArrayMesh:
	var mesh := ArrayMesh.new()
	for surface in source.get_surface_count():
		var arrays := source.surface_get_arrays(surface)
		if edit.is_valid():
			edit.call(surface, arrays)
		var flags: int = int(source.surface_get_format(surface)) & int(Mesh.ARRAY_FLAG_USE_8_BONE_WEIGHTS)
		mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays, [], lods_of(source, surface, scale), flags)
		mesh.surface_set_material(surface, source.surface_get_material(surface))
		mesh.surface_set_name(surface, source.surface_get_name(surface))
	mesh.custom_aabb = source.get_aabb()
	return mesh


## Кількість LOD (без нульового) поверхні.
static func count_of(mesh: Mesh, surface: int) -> int:
	return (RenderingServer.mesh_get_surface(mesh.get_rid(), surface).get("lods", []) as Array).size()


## Найменша похибка LOD поверхні в метрах (0 — LOD нема): перший рівень вмикається, коли 1 піксель ≥ цієї довжини.
static func first_error_of(mesh: Mesh, surface: int) -> float:
	var lods: Array = RenderingServer.mesh_get_surface(mesh.get_rid(), surface).get("lods", [])
	var best := 0.0
	for lod: Dictionary in lods:
		var error := float(lod["edge_length"])
		if best == 0.0 or error < best:
			best = error
	return best


## `{ похибка (м): індекси }` LOD поверхні зі множником `scale` на похибку.
static func lods_of(mesh: Mesh, surface: int, scale: float) -> Dictionary:
	var info: Dictionary = RenderingServer.mesh_get_surface(mesh.get_rid(), surface)
	var wide := int(info.get("vertex_count", 0)) > 65535
	var out: Dictionary = {}
	for lod: Dictionary in info.get("lods", []):
		var bytes: PackedByteArray = lod["index_data"]
		out[float(lod["edge_length"]) * scale] = bytes.to_int32_array() if wide else _indices16(bytes)
	return out


static func _indices16(bytes: PackedByteArray) -> PackedInt32Array:
	var out := PackedInt32Array()
	out.resize(bytes.size() / 2)
	for i in out.size():
		out[i] = bytes.decode_u16(i * 2)
	return out
