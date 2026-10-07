extends SceneTree
## Вивантажує запечену карту материка у сирі файли для Blender (ADR-021):
##   client/blender/height_1024.f32  — 1024 × 1024 float32 LE, метри, рядок = z
##   client/blender/biome_1024.rgba  — 1024 × 1024 RGBA8 (луки, ліс, скелі, пісок)
## Зворотний шлях — tools/import_height.gd після bake_mainland.py.
## godot --headless --path client --script res://tools/dump_height.gd


func _init() -> void:
	var height: Image = load("res://assets/mainland/height.res")
	var biome: Image = load("res://assets/mainland/biome.res")
	if height == null or biome == null:
		push_error("dump_height: assets/mainland/*.res missing")
		quit(1)
		return
	var out := ProjectSettings.globalize_path("res://blender")
	DirAccess.make_dir_recursive_absolute(out)
	var ok := _write(out.path_join("height_1024.f32"), height.get_data()) and _write(out.path_join("biome_1024.rgba"), biome.get_data())
	print("dump_height: %s %dx%d format=%d -> %s" % ["ok" if ok else "FAILED", height.get_width(), height.get_height(), height.get_format(), out])
	quit(0 if ok else 1)


static func _write(path: String, bytes: PackedByteArray) -> bool:
	var file := FileAccess.open(path, FileAccess.WRITE)
	if file == null:
		push_error("dump_height: cannot write %s" % path)
		return false
	file.store_buffer(bytes)
	file.close()
	return true
