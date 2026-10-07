extends SceneTree
## Атлас іконок предметів для web (UX етап 3, 2026-09-24). Бере PNG з `render_icons.gd`,
## обрізає кожну по альфі й вписує в клітинку CELL² з полем, складає пакетики насіння
## (`seed_<культура>` = пакетик + урожай `crop_<культура>`) і пише:
##   web/src/hud/icons/items.webp — атлас COLS × рядки;
##   web/src/hud/icons/items.css  — `.ii-<id>` з позицією клітинки (відсотки, будь-який розмір);
##   web/src/hud/icons/items.ts   — перелік id, для яких іконка є.
##   godot --headless --path client --script res://tools/pack_icons.gd -- --raw=<dir>
## Згенеровані файли руками не правити — перезапускати інструмент.

const CELL := 96
const PAD := 5
const COLS := 8
## Пакетик насіння: пакет позаду (частка клітинки, зсув угору-ліворуч) і урожай спереду
## (частка, зсув униз-праворуч) — культуру видно навіть на 30 px.
const PACKET_SCALE := 0.84
const PACKET_SHIFT := -4
const PRODUCE_SCALE := 0.6
const PRODUCE_SHIFT := 16
## Службові моделі: не предмети, а основа для інших іконок.
const BASES := ["seed_packet"]
## Кілька id з однією іконкою (дозволи на розширення острова — сувій).
const ALIASES := {
	"permit_scroll": ["island_expand_1", "island_expand_2", "island_expand_3", "island_expand_4"],
}
const OUT_DIR := "res://../web/src/hud/icons"


func _init() -> void:
	var raw := _arg("raw")
	if raw.is_empty():
		push_error("pack_icons: --raw=<dir> is required")
		quit(1)
		return
	var cells := _cells(raw)
	if cells.is_empty():
		push_error("pack_icons: no PNG in %s" % raw)
		quit(1)
		return
	_write(cells)
	quit(0)


func _arg(key: String) -> String:
	for arg: String in OS.get_cmdline_user_args():
		if arg.begins_with("--%s=" % key):
			return arg.split("=", true, 1)[1]
	return ""


## id → CELL² image, sorted by id so the atlas is stable between runs.
func _cells(raw: String) -> Dictionary:
	var raws := {}
	for file: String in DirAccess.get_files_at(raw):
		if file.ends_with(".png"):
			raws[file.get_basename()] = Image.load_from_file(raw.path_join(file))
	var cells := {}
	for id: String in raws:
		if not BASES.has(id):
			cells[id] = _fit(raws[id], CELL - PAD * 2)
	if raws.has("seed_packet"):
		for id: String in raws:
			if id.begins_with("crop_"):
				cells["seed_" + id.trim_prefix("crop_")] = _seed(raws["seed_packet"], raws[id])
	var sorted := {}
	var keys := cells.keys()
	keys.sort()
	for id: String in keys:
		sorted[id] = cells[id]
	return sorted


## Crops the image to its visible pixels and centres it, scaled to `inner`, in a CELL² cell.
func _fit(src: Image, inner: int) -> Image:
	var img := src.get_region(src.get_used_rect())
	img.fix_alpha_edges()
	var k := minf(float(inner) / img.get_width(), float(inner) / img.get_height())
	img.resize(maxi(1, roundi(img.get_width() * k)), maxi(1, roundi(img.get_height() * k)), Image.INTERPOLATE_LANCZOS)
	var cell := Image.create_empty(CELL, CELL, false, Image.FORMAT_RGBA8)
	var at := Vector2i((CELL - img.get_width()) / 2, (CELL - img.get_height()) / 2)
	cell.blend_rect(img, Rect2i(Vector2i.ZERO, img.get_size()), at)
	return cell


## Paper packet at the back, the produce big in front of it with a soft drop shadow.
func _seed(packet: Image, produce: Image) -> Image:
	var cell := Image.create_empty(CELL, CELL, false, Image.FORMAT_RGBA8)
	var whole := Rect2i(0, 0, CELL, CELL)
	cell.blend_rect(_fit(packet, roundi(CELL * PACKET_SCALE)), whole, Vector2i.ONE * PACKET_SHIFT)
	var front := _fit(produce, roundi(CELL * PRODUCE_SCALE))
	cell.blend_rect(_shadow(front), whole, Vector2i.ONE * PRODUCE_SHIFT + Vector2i(1, 2))
	cell.blend_rect(front, whole, Vector2i.ONE * PRODUCE_SHIFT)
	return cell


func _shadow(src: Image) -> Image:
	var out := Image.create_empty(src.get_width(), src.get_height(), false, Image.FORMAT_RGBA8)
	for y in src.get_height():
		for x in src.get_width():
			out.set_pixel(x, y, Color(0.05, 0.08, 0.05, src.get_pixel(x, y).a * 0.45))
	return out


func _write(cells: Dictionary) -> void:
	var ids: Array = cells.keys()
	var rows := ceili(ids.size() / float(COLS))
	var atlas := Image.create_empty(COLS * CELL, rows * CELL, false, Image.FORMAT_RGBA8)
	var css := PackedStringArray([_header("/*", " */"),
		".item-icon{background-image:url(\"./items.webp\");background-size:%d%% %d%%}" % [COLS * 100, rows * 100]])
	var names := PackedStringArray()
	for i in ids.size():
		var id: String = ids[i]
		var col := i % COLS
		var row := i / COLS
		atlas.blit_rect(cells[id], Rect2i(0, 0, CELL, CELL), Vector2i(col * CELL, row * CELL))
		var pos := "%s %s" % [_pct(col, COLS), _pct(row, rows)]
		var selectors := PackedStringArray([".ii-" + id])
		names.append(id)
		for alias: String in ALIASES.get(id, []):
			selectors.append(".ii-" + alias)
			names.append(alias)
		css.append("%s{background-position:%s}" % [",".join(selectors), pos])
	names.sort()
	var dir := ProjectSettings.globalize_path(OUT_DIR)
	DirAccess.make_dir_recursive_absolute(dir)
	var err := atlas.save_webp(dir.path_join("items.webp"), true, 0.9)
	if err != OK:
		push_error("pack_icons: cannot save atlas (%d)" % err)
	_save_text(dir.path_join("items.css"), "\n".join(css) + "\n")
	_save_text(dir.path_join("items.ts"), _ts(names))
	print("pack_icons: %d icons, atlas %dx%d -> %s" % [names.size(), atlas.get_width(), atlas.get_height(), dir])


func _pct(i: int, n: int) -> String:
	return "0%" if n <= 1 else "%s%%" % String.num(i * 100.0 / (n - 1), 4)


func _header(open: String, close: String) -> String:
	return "%s Згенеровано client/tools/pack_icons.gd — не правити руками.%s" % [open, close]


func _ts(names: PackedStringArray) -> String:
	var lines := PackedStringArray([_header("/**", " */"), "export const ITEM_ICON_IDS: ReadonlySet<string> = new Set(["])
	for id: String in names:
		lines.append("  \"%s\"," % id)
	lines.append("]);")
	return "\n".join(lines) + "\n"


func _save_text(path: String, text: String) -> void:
	var f := FileAccess.open(path, FileAccess.WRITE)
	if f == null:
		push_error("pack_icons: cannot write %s" % path)
		return
	f.store_string(text)
