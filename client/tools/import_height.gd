extends SceneTree
## Кладе спечену в Blender (або згенеровану gen_mainland.py) карту в гру (ADR-021):
##   client/blender/height_1024.f32 (float32 LE, 1024 × 1024) → assets/mainland/height.res
##   client/blender/biome_1024.rgba (RGBA8, якщо є)          → assets/mainland/biome.res
## Після цього — тести test_mainland і рендери snapshot.gd.
## godot --headless --path client --script res://tools/import_height.gd

const SIZE := 1024


func _init() -> void:
	var path := ProjectSettings.globalize_path("res://blender/height_1024.f32")
	var bytes := FileAccess.get_file_as_bytes(path)
	if bytes.size() != SIZE * SIZE * 4:
		push_error("import_height: %s has %d bytes, expected %d" % [path, bytes.size(), SIZE * SIZE * 4])
		quit(1)
		return
	var image := Image.create_from_data(SIZE, SIZE, false, Image.FORMAT_RF, bytes)
	# zstd у самому ресурсі: карти лежать у web-паку (висоти −22 %, біоми −80 %, +кілька мс при завантаженні).
	var err := ResourceSaver.save(image, "res://assets/mainland/height.res", ResourceSaver.FLAG_COMPRESS)
	if err != OK:
		push_error("import_height: save failed (%d)" % err)
		quit(1)
		return
	var biome_path := ProjectSettings.globalize_path("res://blender/biome_1024.rgba")
	if FileAccess.file_exists(biome_path):
		var rgba := FileAccess.get_file_as_bytes(biome_path)
		if rgba.size() == SIZE * SIZE * 4:
			var biome := Image.create_from_data(SIZE, SIZE, false, Image.FORMAT_RGBA8, rgba)
			if ResourceSaver.save(biome, "res://assets/mainland/biome.res", ResourceSaver.FLAG_COMPRESS) != OK:
				push_error("import_height: biome save failed")
			else:
				print("import_height: biome.res updated")
	var floats := bytes.to_float32_array()
	var lo := INF
	var hi := -INF
	for v: float in floats:
		lo = minf(lo, v)
		hi = maxf(hi, v)
	print("import_height: ok, range %.2f .. %.2f m, centre=%.2f" % [lo, hi, floats[(SIZE / 2 + 60) * SIZE + SIZE / 2]])
	quit(0)
