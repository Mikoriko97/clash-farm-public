extends SceneTree
## Headless test: the cave scene holds the lantern up (idle_torch), keeps the
## character on its floor, places cave nodes without a terrain, glows crystals
## and shows the way out as a landmark.
## Run: godot --headless --path client --script res://tests/test_cave.gd

var _failures := 0
var _frames := 0
var _scene: Node3D


func _check(ok: bool, what: String) -> void:
	if not ok:
		_failures += 1
		push_error("FAIL: " + what)


func _init() -> void:
	_scene = (load("res://scenes/cave/Cave.tscn") as PackedScene).instantiate()
	root.add_child(_scene)


func _process(_delta: float) -> bool:
	_frames += 1
	if _frames < 40:
		return false
	var player: Player = _scene.get_node("Player")
	var anim: AnimationPlayer = player.get_node("AnimationPlayer")
	_check(anim.current_animation == "idle_torch", "standing still holds the lantern up: %s" % anim.current_animation)
	_check(player.global_position.y > -0.2 and player.global_position.y < 0.6, "player rests on the cave floor: y=%.2f" % player.global_position.y)
	var lantern: LanternFollow = _scene.get_node("Lantern")
	_check(lantern.global_position.distance_to(player.global_position + lantern.offset) < 0.05, "lantern rides with the character")
	var nodes: MainlandNodes = _scene.get_node("Nodes")
	nodes.apply([{"cx": 99, "cz": 1, "nodes": [
		{"id": "99_1_10", "kind": "crystal", "x_dm": 200, "z_dm": -160, "hits_left": 3},
		{"id": "99_1_3", "kind": "ore_copper", "x_dm": -180, "z_dm": -120, "hits_left": 4},
	]}])
	_check(nodes.count() == 2, "cave nodes spawn without a terrain: %d" % nodes.count())
	var crystal := nodes.node("99_1_10")
	_check(crystal != null and crystal.visible and crystal.position.is_equal_approx(Vector3(20.0, 0.0, -16.0)), "crystal sits on the floor and is visible")
	var glowing := 0
	if crystal != null:
		for mesh: Node in crystal.find_children("*", "MeshInstance3D", true, false):
			if mesh.name != "Highlight" and (mesh as MeshInstance3D).material_override != null:
				glowing += 1
	_check(glowing > 0, "crystal meshes carry the glow material")
	var landmarks: MainlandLandmarks = _scene.get_node("Landmarks")
	landmarks.apply([{"id": "cave_exit", "name": "Вихід", "x_dm": 0, "z_dm": 80, "discovered": true}])
	_check(landmarks.marker("cave_exit") != null and landmarks.marker("cave_exit").position.is_equal_approx(Vector3(0.0, 0.0, 8.0)), "the way out is a landmark at the entrance")
	print("RESULT: %s (%d failures)" % ["PASS" if _failures == 0 else "FAIL", _failures])
	quit(1 if _failures > 0 else 0)
	return true
