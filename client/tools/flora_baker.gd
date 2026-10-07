extends RefCounted
## Ядро запікання флори й вузлів материка (docs/MAINLAND.md §9). Його викликають
## і `tools/bake_flora.gd` з командного рядка, і кнопка «Запекти» пензля зон
## (`addons/flora_paint`) прямо в редакторі. Бере види з масиву `flora` вузла
## Terrain (у порядку шарів), густину рахує `tools/flora_field.gd` (зони, біоми,
## кластери, вода, схили, «під кроною», виключення, намальована маска
## `assets/mainland/flora_paint.png`), розкидає `Scatter` (синій шум з радіусами),
## види з `node_kind` віддає серверу як вузли — стільки на чанк, скільки каже
## рецепт зони (RECIPE). Пише `assets/mainland/flora.res`, `../shared/mainland_nodes.json`
## і, за бажанням, QA-карту png. Підключати як
## `const Baker := preload("res://tools/flora_baker.gd")`.

const Field := preload("res://tools/flora_field.gd")
const ORIGIN := -512.0
const CHUNK := 64.0
const CHUNKS := 16
const PAINT_PATH := "res://assets/mainland/flora_paint.png"
const BAKE_PATH := "res://assets/mainland/flora.res"
## Скільки вузлів кожного виду на чанк за літерою зони (як було в cf_world).
const RECIPE := {
	"F": {"tree_pine": 6, "tree_oak": 2, "mushroom": 1, "herb": 1},
	"D": {"tree_oak": 6, "tree_pine": 2, "mushroom_rare": 1},
	"R": {"rock": 4, "ore_copper": 2, "ore_iron": 1},
	"M": {"herb": 4, "tree_oak": 1, "rock": 1},
	"S": {"reed": 4, "mushroom": 1},
}
## Запасні місця на вузол (міграція 0024): після відновлення вузол стоїть на наступному з
## `[дім, запасні…]`. Шукаються пост-проходом після ВСІХ шарів (нічого іншого не зсуваються),
## у тому самому чанку й зоні, де вид росте, на [MIN, MAX] метрів від дому й одне від одного.
const SPARES := 2
const SPARE_MIN_M := 6.0
const SPARE_MAX_M := 30.0
const SPARE_TRIES := 96
const SPARE_CLEAR_M := 0.9
const QA_COLOURS := {"tree": Color(0.05, 0.35, 0.05), "bush": Color(0.3, 0.7, 0.2), "flower": Color(0.9, 0.5, 0.8), "grass": Color(0.6, 0.85, 0.3), "stone": Color(0.5, 0.5, 0.55), "reed": Color(0.2, 0.8, 0.8), "mushroom": Color(0.8, 0.6, 0.2), "log": Color(0.5, 0.3, 0.1), "node": Color(1.0, 0.1, 0.1)}

var seed := 7
var qa_path := ""
var terrain: MainlandTerrain
## Тримач герой-груп (`Mainland/Groups`, FloraGroups): їх кола зайняті, з `anchor` — якорі підліску.
var groups: Node
var _kinds: Array[FloraKind] = []
var _scatter := Scatter.new()
var _field: RefCounted
var _quota: Dictionary = {}
var _nodes: Dictionary = {}
var _bake: FloraBake
var _qa_image: Image


## Повертає {instances, nodes, chunks, seconds}; `terrain._ready` має вже відпрацювати.
func run() -> Dictionary:
	var t0 := Time.get_ticks_msec()
	_kinds = terrain.flora
	_quota.clear()
	_nodes.clear()
	_bake = FloraBake.new()
	_bake.seed = seed
	_field = Field.new()
	_field.setup(terrain, _load_rows(), _exclusions(), _scatter, seed)
	_field.paint = load_paint()
	_reserve_groups()
	var rect := Rect2(ORIGIN, ORIGIN, CHUNK * CHUNKS, CHUNK * CHUNKS)
	if not qa_path.is_empty():
		_qa_image = _qa_background()
	for i: int in _kinds.size():
		var kind := _kinds[i]
		_bake.kinds.append(kind.resource_path if kind != null else "")
		if kind == null or kind.scene == null:
			continue
		var density := Callable(_field, "density").bind(kind)
		var quota := Callable(self, "_quota_ok").bind(kind.node_kind) if not kind.node_kind.is_empty() else Callable()
		var pts := _scatter.sample(kind, i, seed, rect, density, quota)
		_store(i, kind, pts)
		print("  %-22s %6d  (%s)" % [kind.resource_path.get_file().get_basename(), pts.size() / 4, "node " + kind.node_kind if not kind.node_kind.is_empty() else kind.biome])
	print("  spare node spots: %d" % _reserve_spares())
	_write_nodes()
	var err := ResourceSaver.save(_bake, BAKE_PATH)
	if err != OK:
		push_error("flora_baker: save failed %d" % err)
	if _qa_image != null:
		_qa_image.save_png(qa_path)
	var total := 0
	for v: Variant in _bake.counts.values():
		total += int(v)
	var out := {"instances": total, "nodes": _node_total(), "chunks": _bake.chunks.size(), "seconds": float(Time.get_ticks_msec() - t0) / 1000.0}
	print("flora_baker: %d flora instances, %d nodes, %d chunks, %.1f s" % [out["instances"], out["nodes"], out["chunks"], out["seconds"]])
	return out


## Герой-групи займають свої кола до розкиду: флора й вузли не лізуть у композицію,
## а підлісок «під кроною» тягнеться до груп з `anchor` (як до дерев перших шарів).
func _reserve_groups() -> void:
	if groups == null:
		return
	var n := 0
	for child: Node in groups.get_children():
		if child is FloraGroup:
			var g := child as FloraGroup
			_scatter.add(Vector2(g.position.x, g.position.z), g.radius_m, g.anchor)
			_qa_dot(g.position.x, g.position.z, Color(1.0, 0.8, 0.2), 3)
			n += 1
	print("  hero groups reserved: %d" % n)


## Маска пензля з диска (не через імпорт: редактор пише її сам); нема — null (усе ×1).
static func load_paint() -> Image:
	var path := ProjectSettings.globalize_path(PAINT_PATH)
	if not FileAccess.file_exists(path):
		return null
	var img := Image.load_from_file(path)
	if img == null:
		return null
	if img.get_format() != Image.FORMAT_RGBA8:
		img.convert(Image.FORMAT_RGBA8)
	return img


func _load_rows() -> Array:
	var raw: Variant = JSON.parse_string(FileAccess.get_file_as_string(ProjectSettings.globalize_path("res://") + "../shared/mainland_zones.json"))
	return raw.get("rows", []) if typeof(raw) == TYPE_DICTIONARY else []


## Кола, де флора не росте: орієнтири з міграції 0014 (радіус + 4 м), причал і площа міста.
func _exclusions() -> Array:
	var out: Array = []
	var sql := FileAccess.get_file_as_string(ProjectSettings.globalize_path("res://") + "../server/migrations/0014_exploration.sql")
	var re := RegEx.new()
	re.compile("\\(\\s*'\\w+',\\s*'[^']*',\\s*(-?\\d+),\\s*(-?\\d+),\\s*(\\d+)")
	for m: RegExMatch in re.search_all(sql):
		out.append(Vector3(float(m.get_string(1)) / 10.0, float(m.get_string(2)) / 10.0, float(m.get_string(3)) / 10.0 + 4.0))
	if out.is_empty():
		push_warning("flora_baker: no landmarks parsed from 0014_exploration.sql")
	# The city square with its buildings (interiors!) — see scenes/city/City.tscn.
	out.append(Vector3(0.0, 64.0, 58.0))
	return out


## Ліміт вузлів на чанк за рецептом зони; відмова не займає місця.
func _quota_ok(pos: Vector2, node_kind: String) -> bool:
	var c := Vector2i(int((pos.x - ORIGIN) / CHUNK), int((pos.y - ORIGIN) / CHUNK))
	var zone: String = _field.zone_at(pos.x, pos.y)
	var need := int((RECIPE.get(zone, {}) as Dictionary).get(node_kind, 0))
	var key := "%d_%d_%s" % [c.x, c.y, node_kind]
	var have := int(_quota.get(key, 0))
	if have >= need:
		return false
	_quota[key] = have + 1
	return true


func _store(index: int, kind: FloraKind, pts: PackedFloat32Array) -> void:
	_bake.counts[index] = pts.size() / 4
	for p: int in pts.size() / 4:
		var x := pts[p * 4]
		var z := pts[p * 4 + 1]
		var c := Vector2i(int((x - ORIGIN) / CHUNK), int((z - ORIGIN) / CHUNK))
		if not kind.node_kind.is_empty():
			var list: Array = _nodes.get(c, [])
			list.append({"kind": kind.node_kind, "x_dm": int(round(x * 10.0)), "z_dm": int(round(z * 10.0))})
			_nodes[c] = list
			_qa_dot(x, z, QA_COLOURS["node"], 2)
			continue
		var per: Dictionary = _bake.chunks.get(c, {})
		var arr: PackedFloat32Array = per.get(index, PackedFloat32Array())
		arr.append_array(pts.slice(p * 4, p * 4 + 4))
		per[index] = arr
		_bake.chunks[c] = per
		_qa_dot(x, z, _qa_colour(kind), 1 if kind.radius_m < 1.0 else 2)


## Резервує запасні місця вузлам (див. SPARES); повертає, скільки знайдено.
func _reserve_spares() -> int:
	var kind_of: Dictionary = {}
	for kind: FloraKind in _kinds:
		if kind != null and not kind.node_kind.is_empty() and not kind_of.has(kind.node_kind):
			kind_of[kind.node_kind] = kind
	var found := 0
	for cz: int in CHUNKS:
		for cx: int in CHUNKS:
			var list: Array = _nodes.get(Vector2i(cx, cz), [])
			for i: int in list.size():
				var n: Dictionary = list[i]
				var kind: FloraKind = kind_of.get(n["kind"])
				if kind != null:
					n["alts"] = _spares_for(Vector2i(cx, cz), i, Vector2(float(n["x_dm"]) / 10.0, float(n["z_dm"]) / 10.0), kind)
					found += (n["alts"] as Array).size()
	return found


func _spares_for(c: Vector2i, i: int, home: Vector2, kind: FloraKind) -> Array:
	var alts: Array = []
	var taken: Array[Vector2] = [home]
	# Стовбур, не крона: у густому лісі кроні й так налягають одна на одну, а запасне місце
	# має бути вільним лише від стовбурів, каменів і рослин на землі.
	var r := minf(kind.radius_m * kind.scale_max, SPARE_CLEAR_M)
	for t: int in SPARE_TRIES:
		if alts.size() >= SPARES:
			break
		var h := Scatter.mix(Scatter.mix(seed * 7919 + c.x * 131 + c.y) ^ (i * 104729) ^ (t * 1299709))
		var p := home + Vector2.from_angle(Scatter.unit(h) * TAU) * lerpf(SPARE_MIN_M, SPARE_MAX_M, Scatter.unit(Scatter.mix(h)))
		if Vector2i(int((p.x - ORIGIN) / CHUNK), int((p.y - ORIGIN) / CHUNK)) != c or _field.density(p, kind) <= 0.0 or _scatter.blocked(p, r):
			continue
		if taken.any(func(q: Vector2) -> bool: return q.distance_to(p) < SPARE_MIN_M):
			continue
		_scatter.add(p, r, false)
		taken.append(p)
		alts.append([int(round(p.x * 10.0)), int(round(p.y * 10.0))])
		_qa_dot(p.x, p.y, Color(1.0, 0.6, 0.1), 1)
	return alts


func _write_nodes() -> void:
	var out: Array = []
	for cz: int in CHUNKS:
		for cx: int in CHUNKS:
			var list: Array = _nodes.get(Vector2i(cx, cz), [])
			for i: int in list.size():
				var n: Dictionary = list[i]
				var row := {"id": "%d_%d_%d" % [cx, cz, i], "kind": n["kind"], "x_dm": n["x_dm"], "z_dm": n["z_dm"]}
				if not (n.get("alts", []) as Array).is_empty():
					row["alts"] = n["alts"]
				out.append(row)
	var path := ProjectSettings.globalize_path("res://") + "../shared/mainland_nodes.json"
	var f := FileAccess.open(path, FileAccess.WRITE)
	f.store_string(JSON.stringify({"seed": seed, "nodes": out}, "", false) + "\n")
	f.close()


func _node_total() -> int:
	var n := 0
	for list: Variant in _nodes.values():
		n += (list as Array).size()
	return n


func _qa_background() -> Image:
	var img := Image.create(1024, 1024, false, Image.FORMAT_RGB8)
	for pz: int in 1024:
		for px: int in 1024:
			var y := terrain.height_px(px, pz)
			var w := terrain.biome_px(px, pz)
			var col := Color(0.2, 0.4, 0.8) if y < 0.3 else Color(0.75, 0.85, 0.55) * w.r + Color(0.45, 0.65, 0.4) * w.g + Color(0.6, 0.55, 0.5) * w.b + Color(0.9, 0.85, 0.65) * w.a
			img.set_pixel(px, pz, col)
	return img


func _qa_dot(x: float, z: float, col: Color, size: int) -> void:
	if _qa_image == null:
		return
	var px := int(x - ORIGIN)
	var pz := int(z - ORIGIN)
	for dz: int in range(-size + 1, size):
		for dx: int in range(-size + 1, size):
			var qx := px + dx
			var qz := pz + dz
			if qx >= 0 and qz >= 0 and qx < 1024 and qz < 1024:
				_qa_image.set_pixel(qx, qz, col)


static func _qa_colour(kind: FloraKind) -> Color:
	var name := kind.resource_path.get_file().get_basename()
	for key: String in QA_COLOURS:
		if name.begins_with(key) or (key == "tree" and (name.begins_with("fir") or name.begins_with("spruce") or name.begins_with("birch") or name.begins_with("oak") or name.begins_with("meadow_tree") or name.begins_with("sapling"))):
			return QA_COLOURS[key]
	if name.begins_with("fern") or name.begins_with("bush"):
		return QA_COLOURS["bush"]
	if name.begins_with("boulder") or name.begins_with("scree") or name.begins_with("rock"):
		return QA_COLOURS["stone"]
	return Color(0.1, 0.1, 0.1)
