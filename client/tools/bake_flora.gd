extends SceneTree
## Запікання флори й вузлів ресурсів материка з командного рядка (docs/MAINLAND.md §9):
##   godot --headless --path client --script res://tools/bake_flora.gd -- [--seed=7] [--qa=C:/tmp/flora.png]
## Саме запікання — `tools/flora_baker.gd` (те саме робить кнопка «Запекти» пензля
## зон у редакторі). Бере види з масиву `flora` вузла Terrain у `Mainland.tscn`,
## маску пензля `assets/mainland/flora_paint.png`, пише `assets/mainland/flora.res`
## і `../shared/mainland_nodes.json`. Запускати після перепікання рельєфу
## (після import_height.gd) і після малювання зон.

const Baker := preload("res://tools/flora_baker.gd")

var _baker: RefCounted
var _scene: Node3D


func _init() -> void:
	_baker = Baker.new()
	for a: String in OS.get_cmdline_user_args():
		if a.begins_with("--seed="):
			_baker.seed = int(a.get_slice("=", 1))
		elif a.begins_with("--qa="):
			_baker.qa_path = a.get_slice("=", 1)
	_scene = (load("res://scenes/mainland/Mainland.tscn") as PackedScene).instantiate()
	root.add_child(_scene)
	_baker.terrain = _scene.get_node("Terrain")
	_baker.groups = _scene.get_node_or_null("Groups")
	# Terrain._ready (height map) runs only once the loop starts — hence deferred.
	call_deferred("_run")


func _run() -> void:
	var out: Dictionary = _baker.run()
	print("bake_flora: %d flora instances, %d nodes, %d chunks, %.1f s" % [out["instances"], out["nodes"], out["chunks"], out["seconds"]])
	quit()
