extends SceneTree
## Генерує стартовий набір герой-груп флори (`scenes/mainland/groups/*.tscn`,
## docs/MAINLAND.md §9) зі шматків паку рослинності власника (`assets/nature/pieces`):
##   godot --headless --path client --script res://tools/make_flora_groups.gd
## Кожна група — сцена `FloraGroup` з мешами шматків (спільний матеріал паку через
## пост-імпорт), циліндрами-колайдерами під твердим (колоди, пні, каміння) і
## радіусом сліду. Далі власник править їх у редакторі як звичайні сцени (зсув,
## поворот, заміна шматка) або складає нові за зразком; генератор — лише старт.

const PieceMesh := preload("res://scripts/piece_mesh.gd")
const DIR := "res://scenes/mainland/groups"
const PIECES := "res://assets/nature/pieces"
## Назва → [радіус сліду, [[шматок, x, z, поворот°, масштаб, твердий], …]].
const GROUPS := {
	"log_fern": [3.6, [["log_big_knotty", 0.0, 0.0, 20.0, 1.0, true], ["bush_fern_average", 1.6, 1.3, 0.0, 1.0, false], ["bush_fern_average", -1.9, 0.9, 130.0, 0.9, false], ["bush_fern_small", 0.4, -1.7, 60.0, 1.0, false], ["mushroom_flat_group_brown", 1.1, -0.6, 0.0, 1.0, false], ["grass_bush_high_02", -2.4, -1.2, 0.0, 1.0, false]]],
	"stump_mushrooms": [2.6, [["stump_big_hollow", 0.0, 0.0, 0.0, 1.0, true], ["amanita_group_red", 1.3, 0.6, 0.0, 1.0, false], ["mushroom_high_group_brown", -1.1, 1.0, 40.0, 1.0, false], ["bush_fern_small", -1.4, -1.2, 200.0, 1.0, false], ["grass_bush_low_02", 1.5, -1.3, 0.0, 1.0, false]]],
	"boulders_moss": [3.8, [["stone_big_01_mossy", 0.0, 0.0, 15.0, 1.0, true], ["stone_average_02_mossy", 2.2, 0.8, 70.0, 1.0, true], ["stone_small_03_mossy", -1.9, 1.4, 0.0, 1.0, false], ["stone_small_01_mossy", 1.0, -2.0, 110.0, 1.0, false], ["bush_fern_average", -1.6, -1.5, 30.0, 0.9, false], ["grass_bush_high_01", 2.6, -1.4, 0.0, 1.0, false]]],
	"dead_tree": [4.2, [["tree_giant_bare", 0.0, 0.0, 0.0, 1.0, true], ["branch_big_bare", 2.4, 1.2, 35.0, 1.0, false], ["stump_average_high", -2.1, 1.6, 0.0, 1.0, true], ["mushroom_wide_group_white", 1.2, -1.8, 0.0, 1.0, false], ["branch_small_bare", -1.0, -2.4, 150.0, 1.0, false]]],
	"berry_patch": [3.2, [["bush_raspberries_berry", 0.0, 0.0, 0.0, 1.0, false], ["bush_raspberries_berry", 1.9, 0.9, 80.0, 0.9, false], ["bush_average_blueberry", -1.7, 1.2, 0.0, 1.0, false], ["flower_high_purple", 0.6, -1.9, 0.0, 1.0, false], ["flower_small_white", -1.4, -1.4, 0.0, 1.0, false], ["grass_bush_low_01", 2.3, -1.1, 0.0, 1.0, false]]],
	"rock_outcrop": [4.4, [["stone_group_big", 0.0, 0.0, 0.0, 1.0, true], ["stone_big_03", 2.6, 1.4, 50.0, 0.9, true], ["stone_small_01", -2.4, 1.8, 0.0, 1.0, false], ["stone_small_02", 1.6, -2.6, 90.0, 1.0, false], ["grass_plain_low_01", -1.8, -2.2, 0.0, 1.0, false]]],
	"reed_bank": [3.4, [["grass_plain_high_01", 0.0, 0.0, 0.0, 1.1, false], ["grass_plain_high_01", 1.5, 0.9, 60.0, 1.0, false], ["grass_plain_high_02", -1.4, 1.1, 0.0, 1.0, false], ["grass_plain_high_01", 0.3, -1.6, 120.0, 0.9, false], ["log_small_hollow", -1.6, -1.2, 30.0, 1.0, true], ["mushroom_flat_group_yellow", 1.6, -1.2, 0.0, 1.0, false]]],
	"fallen_birch": [3.6, [["log_big_regular", 0.0, 0.0, 70.0, 1.0, true], ["branch_average_leaves", 1.8, 1.4, 20.0, 1.0, false], ["bush_small_flowers", -1.9, 1.1, 0.0, 1.0, false], ["flower_bush_yellow", 1.2, -1.7, 0.0, 1.0, false], ["grass_bush_low_02", -1.5, -1.6, 0.0, 1.0, false]]],
}


func _init() -> void:
	DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path(DIR))
	for name: String in GROUPS:
		_make(name, float(GROUPS[name][0]), GROUPS[name][1])
	print("make_flora_groups: %d groups in %s" % [GROUPS.size(), DIR])
	quit()


func _make(name: String, radius: float, parts: Array) -> void:
	var root := FloraGroup.new()
	root.name = name.to_pascal_case()
	root.radius_m = radius
	var body := StaticBody3D.new()
	body.name = "Body"
	body.collision_layer = 1
	body.collision_mask = 0
	root.add_child(body)
	body.owner = root
	for part: Array in parts:
		var found: Dictionary = PieceMesh.first(load("%s/%s.glb" % [PIECES, part[0]]))
		var mesh: Mesh = found.get("mesh")
		if mesh == null:
			push_warning("make_flora_groups: no mesh in " + str(part[0]))
			continue
		var mi := MeshInstance3D.new()
		mi.name = str(part[0])
		mi.mesh = mesh
		var basis := Basis(Vector3.UP, deg_to_rad(float(part[3]))).scaled(Vector3.ONE * float(part[4]))
		mi.transform = Transform3D(basis, Vector3(float(part[1]), 0.0, float(part[2]))) * (found.get("xform", Transform3D.IDENTITY) as Transform3D)
		root.add_child(mi)
		mi.owner = root
		if bool(part[5]):
			var aabb := mesh.get_aabb()
			var shape := CylinderShape3D.new()
			shape.radius = maxf(aabb.size.x, aabb.size.z) * 0.4 * float(part[4])
			shape.height = maxf(aabb.size.y * float(part[4]), 0.4)
			var col := CollisionShape3D.new()
			col.name = str(part[0]) + "_col"
			col.shape = shape
			col.position = Vector3(float(part[1]), shape.height * 0.5, float(part[2]))
			body.add_child(col)
			col.owner = root
	var packed := PackedScene.new()
	packed.pack(root)
	var err := ResourceSaver.save(packed, "%s/%s.tscn" % [DIR, name])
	print("  %-16s r=%.1f parts=%d %s" % [name, radius, parts.size(), "ok" if err == OK else "ERR %d" % err])
	root.free()
