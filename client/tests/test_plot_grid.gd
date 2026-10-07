extends SceneTree
## Headless test: PlotGrid builds PlotNodes from world_state cells, links side
## neighbours into beds, drops plots missing from a full snapshot, ignores
## garbage, and a plot click reports plot_selected. Placer snaps and checks cells.
## Run: godot --headless --path client --script res://tests/test_plot_grid.gd

var _failures := 0
var _ran := false


## Nodes added while the tree boots are not ready yet: run on the first frame.
func _process(_delta: float) -> bool:
	if _ran:
		quit(1)
		return true
	_ran = true
	var grid: PlotGrid = (load("res://scenes/island/PlotGrid.tscn") as PackedScene).instantiate() as PlotGrid
	root.add_child(grid)
	_check(grid.plot_count() == 0, "no plots before world_state")

	grid.apply_world_state({
		"full": true,
		"now": "2026-09-04T12:00:00Z",
		"plots": [
			{"idx": 0, "col": -1, "row": -3, "state": "growing", "crop_id": "seed_wheat", "ready_at": "2026-09-04T12:30:00Z"},
			{"idx": 1, "col": 0, "row": -3, "state": "ready", "crop_id": "seed_wheat", "ready_at": "2026-09-04T11:00:00Z"},
			{"idx": 2, "col": 1, "row": -3, "state": "empty"},
			{"idx": 3, "col": 4, "row": 4, "state": "teleporting"},
			{"idx": 9, "state": "ready"},
			"garbage",
		],
	})
	_check(grid.plot_count() == 4, "four plots with cells created (%d)" % grid.plot_count())
	_check(grid.plot(9) == null, "entry without a cell is ignored")
	_check(grid.plot(0).position == Vector3(-3.0, 0.0, -9.0), "cell (-1,-3) sits at (-3, 0, -9): %s" % grid.plot(0).position)
	_check(grid.plot(0).state == "growing" and grid.plot(0).crop_id == "seed_wheat", "growing applied")
	_check(grid.plot(1).state == "ready", "ready applied")
	_check(grid.plot(3).state == "empty", "unknown state ignored")
	_check(grid.plot(0).bed_mates.size() == 3 and grid.plot(3).bed_mates.size() == 1, "row of three is one bed, the far plot its own")
	var soil0 := grid.plot(0).get_node("Soil") as Node3D
	var soil1 := grid.plot(1).get_node("Soil") as Node3D
	_check(soil0.scale == soil1.scale and soil0.position == Vector3.ZERO, "every plot fills its cell the same way, no stretching")
	grid.plot(0).set_highlighted(true)
	_check(grid.plot(0).get_node("EdgeW").visible and not grid.plot(0).get_node("EdgeE").visible and grid.plot(2).get_node("EdgeE").visible, "outline only on the bed's outer edges")
	grid.plot(0).set_highlighted(false)
	_check(not grid.plot(2).get_node("EdgeE").visible, "outline clears on the whole bed")
	_check(grid.cell_taken(0, -3) and not grid.cell_taken(0, 0), "cell_taken follows the plots")
	var left := grid.plot(0).remaining_seconds()
	_check(left > 1700.0 and left <= 1800.0, "countdown follows server clock (%.0f s left)" % left)

	# Patch: a new plot next to the row joins the bed; nothing else is dropped.
	grid.apply_world_state({"full": false, "now": "2026-09-04T12:00:00Z", "plots": [{"idx": 4, "col": 2, "row": -3, "state": "empty"}]})
	_check(grid.plot_count() == 5 and grid.plot(4).bed_mates.size() == 4, "patch adds a plot into the bed")
	# Full snapshot without idx 3 removes it.
	grid.apply_world_state({"full": true, "now": "2026-09-04T12:00:00Z", "plots": [
		{"idx": 0, "col": -1, "row": -3, "state": "empty"}, {"idx": 1, "col": 0, "row": -3, "state": "empty"},
		{"idx": 2, "col": 1, "row": -3, "state": "empty"}, {"idx": 4, "col": 2, "row": -3, "state": "empty"}]})
	_check(grid.plot(3) == null and grid.plot_count() == 4, "full snapshot drops the missing plot")

	var got: Array[int] = []
	grid.plot(4).selected.connect(func(i: int) -> void: got.append(i))
	grid.plot(4)._on_click()
	_check(got == [4], "click reports plot idx 4")

	_check_placer(grid)
	print("RESULT: %s (%d failures)" % ["PASS" if _failures == 0 else "FAIL", _failures])
	quit(1 if _failures > 0 else 0)
	return true


func _check_placer(grid: PlotGrid) -> void:
	var placer: Placer = (load("res://scenes/island/Placer.tscn") as PackedScene).instantiate() as Placer
	placer.plot_grid = grid
	root.add_child(placer)
	_check(Placer.cell_of(Vector3(4.4, 0.0, -1.6)) == Vector2i(1, -1), "world point snaps to the nearest cell")
	_check(not placer.is_free(Vector2i(0, -3)), "occupied cell is not free")
	_check(placer.is_free(Vector2i(5, 5)), "free cell inside the island")
	_check(not placer.is_free(Vector2i(8, 0)), "cell over the edge is not free")
	placer.set_active(true)
	placer.hover(Vector3(15.2, 0.0, 14.9))
	_check(placer._ghost.visible and placer._ghost.global_position == Vector3(15.0, 0.0, 15.0), "ghost snaps to the hovered cell")


func _check(ok: bool, what: String) -> void:
	if not ok:
		_failures += 1
	print("%s %s" % ["ok  " if ok else "FAIL", what])
