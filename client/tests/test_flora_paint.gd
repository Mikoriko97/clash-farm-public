extends SceneTree
## Headless test: the painted zone mask multiplies the density model per layer —
## a neutral mask (0.5) changes nothing, 0 in the canopy channel removes trees but
## keeps ground cover, 1 doubles; the shipped mask loads and is neutral or valid.
## Run: godot --headless --path client --script res://tests/test_flora_paint.gd

const Field := preload("res://tools/flora_field.gd")
const Baker := preload("res://tools/flora_baker.gd")

var _failures := 0
var _scene: Node3D
var _frames := 0


func _check(ok: bool, what: String) -> void:
	if not ok:
		_failures += 1
		push_error("FAIL: " + what)


func _init() -> void:
	_scene = (load("res://scenes/mainland/Mainland.tscn") as PackedScene).instantiate()
	root.add_child(_scene)


func _process(_delta: float) -> bool:
	_frames += 1
	if _frames < 3:
		return false
	_run()
	print("RESULT: %s (%d failures)" % ["PASS" if _failures == 0 else "FAIL", _failures])
	quit(1 if _failures > 0 else 0)
	return true


func _run() -> void:
	var terrain: MainlandTerrain = _scene.get_node("Terrain")
	var tree: FloraKind = null
	var ground: FloraKind = null
	for kind: FloraKind in terrain.flora:
		if tree == null and kind.layer == "canopy" and kind.zones.contains("F"):
			tree = kind
		if ground == null and kind.layer == "ground" and kind.zones.contains("F"):
			ground = kind
	_check(tree != null and ground != null, "forest has a canopy and a ground kind")
	var rows: Array = JSON.parse_string(FileAccess.get_file_as_string(ProjectSettings.globalize_path("res://") + "../shared/mainland_zones.json")).get("rows", [])
	var field: RefCounted = Field.new()
	field.setup(terrain, rows, [], Scatter.new(), 7)
	# A forest spot where the tree grows without any mask.
	var spot := Vector2.ZERO
	var base := 0.0
	for z: int in range(-400, -200, 4):
		for x: int in range(-400, -100, 4):
			var p := Vector2(x, z)
			var d: float = field.density(p, tree)
			if d > base:
				base = d
				spot = p
	_check(base > 0.0, "found a forest spot with tree density %.2f" % base)
	var ground_base: float = field.density(spot, ground)
	var mask := Image.create(Field.PAINT_PX, Field.PAINT_PX, false, Image.FORMAT_RGBA8)
	var n: float = Field.NEUTRAL
	mask.fill(Color(n, n, n, n))
	field.paint = mask
	_check(is_equal_approx(field.density(spot, tree), base), "neutral mask leaves density as is")
	mask.fill(Color(0.0, n, n, n))
	_check(field.density(spot, tree) == 0.0, "zero canopy channel removes trees")
	_check(is_equal_approx(field.density(spot, ground), ground_base), "zero canopy channel keeps ground cover")
	mask.fill(Color(1.0, n, n, n))
	_check(is_equal_approx(field.density(spot, tree), base * 2.0), "full canopy channel doubles")
	var shipped: Image = Baker.load_paint()
	_check(shipped != null and shipped.get_width() == Field.PAINT_PX, "shipped mask loads at %d px" % Field.PAINT_PX)
