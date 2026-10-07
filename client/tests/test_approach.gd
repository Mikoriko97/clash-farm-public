extends SceneTree
## Headless test: the character walks up to the real silhouette of decor, not to
## a fixed distance or a pick cylinder. The pick body is invisible to routes and
## to the character; a thin lantern is approached closer than a bench.
## Run: godot --headless --path client --script res://tests/test_approach.gd

const BUDGET_FRAMES := 600

var _failures := 0
var _frames := 0
var _player: Player
var _lantern: DecorItem
var _bench: DecorItem
var _arrived := false
var _stage := 0


func _check(ok: bool, what: String) -> void:
	if not ok:
		_failures += 1
		push_error("FAIL: " + what)


func _init() -> void:
	var world := Node3D.new()
	root.add_child(world)
	var floor_body := StaticBody3D.new()
	var floor_shape := CollisionShape3D.new()
	var box := BoxShape3D.new()
	box.size = Vector3(120.0, 1.0, 120.0)
	floor_shape.shape = box
	floor_body.add_child(floor_shape)
	floor_body.position = Vector3(0.0, -0.5, 0.0)
	world.add_child(floor_body)
	var scene := load("res://scenes/island/DecorItem.tscn") as PackedScene
	_lantern = scene.instantiate() as DecorItem
	_lantern.idx = 0
	_lantern.position = Vector3(6.0, 0.0, 0.0)
	world.add_child(_lantern)
	_lantern.apply("decor_lantern")
	_bench = scene.instantiate() as DecorItem
	_bench.idx = 1
	_bench.position = Vector3(-6.0, 0.0, 0.0)
	world.add_child(_bench)
	_bench.apply("decor_bench")
	_player = (load("res://scenes/player/Player.tscn") as PackedScene).instantiate()
	world.add_child(_player)
	var interaction := Interaction.new()
	interaction.name = "Interaction"
	interaction.player = _player
	world.add_child(interaction)
	_player.arrived.connect(func() -> void: _arrived = true)
	var nav := NavGrid.new()
	nav.name = "NavGrid"
	world.add_child(nav)
	_world = world
	_interaction = interaction
	_nav = nav


var _world: Node3D
var _interaction: Interaction
var _nav: NavGrid


func _process(_delta: float) -> bool:
	_frames += 1
	if _frames == 2:
		var lantern_reach := _lantern.approach_distance()
		var bench_reach := _bench.approach_distance()
		_check(lantern_reach < 1.0, "lantern is approached closely: %.2f" % lantern_reach)
		_check(bench_reach > lantern_reach, "bench keeps more distance than the lantern: %.2f vs %.2f" % [bench_reach, lantern_reach])
		var rects := _nav.collect_obstacles(_world)
		var widest := 0.0
		for r: Rect2 in rects:
			if r.has_point(Vector2(6.0, 0.0)):
				widest = maxf(widest, maxf(r.size.x, r.size.y))
		_check(widest > 0.0 and widest < 1.0, "route obstacle around the lantern is its post, not the 2 m pick cylinder: %.2f" % widest)
		_check(_interaction.walk_to("decor", 0), "walk_to lantern accepted")
		_stage = 1
	elif _stage == 1 and _arrived:
		var gap := Vector2(_player.global_position.x - 6.0, _player.global_position.z).length()
		_check(gap < 1.1, "character stands next to the lantern post, %.2f m from its origin" % gap)
		return _finish()
	if _frames > BUDGET_FRAMES:
		_check(false, "did not arrive within the budget (stage %d)" % _stage)
		return _finish()
	return false


func _finish() -> bool:
	print("RESULT: %s (%d failures)" % ["PASS" if _failures == 0 else "FAIL", _failures])
	quit(1 if _failures > 0 else 0)
	return true
