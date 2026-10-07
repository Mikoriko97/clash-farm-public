extends SceneTree
## Headless test of footsteps and weather sound: contact phases of the walk clips match the feet in the animation, a step
## plays per contact (two per walk cycle), far peers stay silent, the surface comes from the collider (`wood`), the plots
## (`dirt`) and the zone (`stone` in the cave), and the rain loops follow the weather level (rain → light, storm → heavy, none
## underground). Run: godot --headless --path client --script res://tests/test_footsteps.gd

var _failures := 0
var _avatar: Node3D
var _steps: Array = []
var _audio: Node


func _check(ok: bool, what: String) -> void:
	if not ok:
		_failures += 1
		push_error("FAIL: " + what)


func _init() -> void:
	_audio = load("res://scripts/audio.gd").new()
	_audio.name = "Audio"
	root.add_child(_audio)
	_avatar = (load("res://scenes/player/PeerAvatar.tscn") as PackedScene).instantiate() as Node3D
	root.add_child(_avatar)
	_run.call_deferred()


func _run() -> void:
	for i in 3:
		await process_frame
	_check_phases()
	_check_cycle()
	_check_range()
	_check_weights()
	await _check_surfaces()
	await _check_weather()
	print("RESULT: %s (%d failures)" % ["PASS" if _failures == 0 else "FAIL", _failures])
	quit(1 if _failures > 0 else 0)


func _footsteps() -> Footsteps:
	return _avatar.get_node("Footsteps") as Footsteps


func _check_phases() -> void:
	_check(Footsteps.crossed(0.05, 0.15, 0.10) and not Footsteps.crossed(0.05, 0.09, 0.10), "a contact is crossed once, inside the interval")
	_check(Footsteps.crossed(0.95, 0.05, 0.0) and Footsteps.crossed(0.9, 0.2, 0.1) and not Footsteps.crossed(0.9, 0.05, 0.5), "the interval wraps around the end of the cycle")
	_check(not Footsteps.crossed(0.4, 0.4, 0.4), "a standing phase crosses nothing")
	# The contacts sit where the foot bone is lowest in the clip (measured on the real animation).
	var anim := _avatar.get_node("%AnimationPlayer") as AnimationPlayer
	var skeleton := _avatar.find_child("Skeleton3D", true, false) as Skeleton3D
	for clip: StringName in Footsteps.CONTACTS:
		var marks: Array = Footsteps.CONTACTS[clip]
		var length := anim.get_animation(clip).length
		var foot_y: Array[float] = []
		for mark: float in marks:
			anim.play(clip, 0.0)
			anim.seek(length * mark, true)
			skeleton.force_update_all_bone_transforms()
			var bone := skeleton.find_bone("LeftFoot" if marks.find(mark) == 0 else "RightFoot")
			# at the contact the foot is near its lowest point of the whole cycle
			var low := 1e9
			var high := -1e9
			for i in 40:
				anim.seek(length * float(i) / 40.0, true)
				skeleton.force_update_all_bone_transforms()
				var y := skeleton.get_bone_global_pose(bone).origin.y
				low = minf(low, y)
				high = maxf(high, y)
			anim.seek(length * mark, true)
			skeleton.force_update_all_bone_transforms()
			foot_y.append((skeleton.get_bone_global_pose(bone).origin.y - low) / maxf(high - low, 0.001))
		_check(foot_y[0] < 0.35 and foot_y[1] < 0.35, "%s: both contacts are on the ground (foot height %s of its range)" % [clip, foot_y])


func _check_cycle() -> void:
	var steps := _footsteps()
	steps.stepped.connect(func(surface: String, loudness: float) -> void: _steps.append([surface, loudness]))
	var anim := _avatar.get_node("%AnimationPlayer") as AnimationPlayer
	anim.play(&"walk_forward", 0.0)
	var length := anim.current_animation_length
	_steps.clear()
	for i in 21:
		anim.seek(length * float(i) / 20.0 * 0.999, true)
		steps._process(0.0)
	_check(_steps.size() == 2, "one walk cycle = two steps (got %d)" % _steps.size())
	_check(not _steps.is_empty() and _steps[0][0] == "grass", "the default surface is grass")
	_steps.clear()
	anim.play(&"run_forward", 0.0)
	for i in 21:
		anim.seek(anim.current_animation_length * float(i) / 20.0 * 0.999, true)
		steps._process(0.0)
	_check(_steps.size() == 2 and _steps[0][1] > -13.0, "a run cycle is two louder steps (%s)" % [_steps])
	_steps.clear()
	anim.play(&"idle", 0.0)
	for i in 10:
		anim.seek(anim.current_animation_length * float(i) / 10.0, true)
		steps._process(0.0)
	_check(_steps.is_empty(), "standing still is silent")
	var players := 0
	for child in steps.get_children():
		if child is AudioStreamPlayer3D and child.bus == "SFX":
			players += 1
	_check(players == 2, "a peer steps through two 3D players on the SFX bus (%d)" % players)


func _check_range() -> void:
	var camera := Camera3D.new()
	root.add_child(camera)
	camera.global_position = Vector3(0.0, 2.0, 60.0)
	camera.make_current()
	_steps.clear()
	var steps := _footsteps()
	var anim := _avatar.get_node("%AnimationPlayer") as AnimationPlayer
	anim.play(&"walk_forward", 0.0)
	for i in 21:
		anim.seek(anim.current_animation_length * float(i) / 20.0 * 0.999, true)
		steps._process(0.0)
	_check(_steps.is_empty(), "a peer 60 m from the camera makes no sound")
	camera.global_position = Vector3(0.0, 2.0, 5.0)
	for i in 21:
		anim.seek(anim.current_animation_length * float(i) / 20.0 * 0.999, true)
		steps._process(0.0)
	_check(_steps.size() == 2, "…and 5 m away it does (%d)" % _steps.size())
	camera.queue_free()


func _check_weights() -> void:
	_check(WeatherAudio.light_weight(0.0) == 0.0 and WeatherAudio.heavy_weight(0.0) == 0.0, "no rain, no sound")
	_check(is_equal_approx(WeatherAudio.light_weight(0.55), 1.0) and WeatherAudio.heavy_weight(0.55) > 0.2 and WeatherAudio.heavy_weight(0.55) < 0.3, "rain: the light loop at full, the heavy one a quarter")
	_check(is_equal_approx(WeatherAudio.heavy_weight(1.0), 1.0) and WeatherAudio.light_weight(1.0) < 0.35, "storm: the heavy loop at full, the light one faint")
	_check(WeatherAudio.heavy_weight(0.3) == 0.0, "below 0.4 the heavy loop is off")
	_check(WeatherAudio.breeze_weight(0.0) == 0.0 and WeatherAudio.gale_weight(0.0) == 0.0, "still air, no wind sound")
	_check(is_equal_approx(WeatherAudio.breeze_weight(0.6), 1.0) and WeatherAudio.gale_weight(0.6) > 0.25, "a moderate wind: the breeze at full, the gale rising")
	_check(is_equal_approx(WeatherAudio.gale_weight(1.0), 1.0) and WeatherAudio.breeze_weight(1.0) < 0.45, "a storm wind: the gale at full, the breeze fading")
	_check(WeatherAudio.night_weight(1.0) == 0.0 and WeatherAudio.night_weight(0.0) == 1.0 and WeatherAudio.night_weight(0.2) > 0.2 and WeatherAudio.night_weight(0.2) < 0.9, "night grows as the light fails")
	_check(is_equal_approx(WeatherAudio.leaves_weight(0.0), WeatherAudio.LEAVES_CALM) and is_equal_approx(WeatherAudio.leaves_weight(0.7), 1.0) and is_equal_approx(WeatherAudio.leaves_weight(1.0), 1.0) and WeatherAudio.leaves_weight(0.4) > WeatherAudio.leaves_weight(0.2), "the leaves whisper in still air and rustle louder with the wind")
	_check(WeatherAudio.morning_weight(2.0) == 0.0 and WeatherAudio.morning_weight(12.0) == 0.0 and WeatherAudio.morning_weight(23.0) == 0.0, "no birds at night or at noon")
	_check(WeatherAudio.morning_weight(4.8) > 0.05 and WeatherAudio.morning_weight(4.8) < 0.95 and is_equal_approx(WeatherAudio.morning_weight(7.0), 1.0) and WeatherAudio.morning_weight(9.5) > 0.05 and WeatherAudio.morning_weight(9.5) < 0.95, "the chorus rises before dawn and thins out by late morning")
	_check(WeatherAudio.morning_weight(19.0) == 0.0, "no dawn chorus at dusk")
	_check(WeatherAudio.dry_weight(0.0) == 1.0 and WeatherAudio.dry_weight(0.35) == 0.0 and WeatherAudio.dry_weight(0.1) > 0.0, "rain drowns the crickets")


func _check_surfaces() -> void:
	# A collider marked `surface = wood` (the bridges) under the feet wins over everything else.
	var body := StaticBody3D.new()
	var shape := CollisionShape3D.new()
	var box := BoxShape3D.new()
	box.size = Vector3(4.0, 0.4, 4.0)
	shape.shape = box
	body.add_child(shape)
	body.set_meta("surface", "wood")
	root.add_child(body)
	body.global_position = Vector3(40.0, -0.2, 0.0)
	await physics_frame
	await physics_frame
	var probe := Node3D.new()
	root.add_child(probe)
	_check(Footsteps.surface_at(probe, Vector3(40.0, 0.0, 0.0)) == "wood", "a collider tagged wood gives wood")
	_check(Footsteps.surface_at(probe, Vector3(-40.0, 0.0, 0.0)) == "grass", "open ground is grass")
	# Plots are soil: the provider answers for the cell that holds one.
	var grid := (load("res://scenes/island/PlotGrid.tscn") as PackedScene).instantiate()
	root.add_child(grid)
	grid.call("apply_world_state", {"full": true, "plots": [{"idx": 1, "col": 2, "row": -1, "state": "empty"}]})
	_check(grid.is_in_group(Footsteps.PROVIDER_GROUP), "the plot grid is a surface provider")
	_check(Footsteps.surface_at(probe, Vector3(6.4, 0.0, -2.8)) == "dirt", "a plot cell gives dirt")
	_check(Footsteps.surface_at(probe, Vector3(0.0, 0.0, 0.0)) == "grass", "a cell without a plot stays grass")
	# The mainland ground mirrors its shader: cliffs are stone, high ground snow, the shoreline and the sand biome sand.
	var meadow := Color(1.0, 0.0, 0.0, 0.0)
	_check(MainlandTerrain.surface_for(10.0, 1.0, meadow, 42.0) == "grass", "a meadow is grass")
	_check(MainlandTerrain.surface_for(10.0, 1.0, Color(0.0, 0.0, 0.0, 1.0), 42.0) == "sand", "the sand biome is sand")
	_check(MainlandTerrain.surface_for(0.2, 1.0, meadow, 42.0) == "sand", "the shoreline is sand")
	_check(MainlandTerrain.surface_for(10.0, 1.0, Color(0.0, 0.0, 1.0, 0.0), 42.0) == "stone", "the rock biome is stone")
	_check(MainlandTerrain.surface_for(50.0, 0.95, Color(0.0, 0.0, 1.0, 0.0), 42.0) == "snow", "high ground is snow, rock biome or not")
	_check(MainlandTerrain.surface_for(41.5, 0.95, meadow, 42.0) == "grass", "just below the snow line it is not snow yet")
	_check(MainlandTerrain.surface_for(50.0, 0.5, meadow, 42.0) == "stone", "a cliff is stone, snow or not")
	for surface in ["sand", "snow"]:
		_check(Footsteps.STEPS.has(surface) and (Footsteps.STEPS[surface] as Array).size() == 4, "%s has four step variants" % surface)
	# Underground everything is stone.
	_audio.set("zone", "cave")
	_check(Footsteps.surface_at(probe, Vector3(-40.0, 0.0, 0.0)) == "stone", "the cave floor is stone")
	_audio.set("zone", "island")
	for node in [body, probe, grid]:
		node.queue_free()


func _check_weather() -> void:
	var script := GDScript.new()
	script.source_code = "extends Node\nsignal storm_flash\nvar rain := 0.0\nvar wind := 0.0\nfunc rain_level() -> float:\n\treturn rain\nfunc wind_level() -> float:\n\treturn wind\n"
	script.reload()
	var fake := Node.new()
	fake.set_script(script)
	fake.add_to_group("weather")
	root.add_child(fake)
	var weather_audio: WeatherAudio = null
	for child in _audio.get_children():
		if child is WeatherAudio:
			weather_audio = child
	_check(weather_audio != null, "Audio carries the weather sound")
	if weather_audio == null:
		return
	fake.set("rain", 1.0)
	for i in 60:
		weather_audio._tick(0.25)
	_check(is_equal_approx(weather_audio.level, 1.0), "the sound follows a storm (level %.2f)" % weather_audio.level)
	var players: Array[AudioStreamPlayer] = []
	for child in weather_audio.get_children():
		if child is AudioStreamPlayer:
			players.append(child)
	fake.set("wind", 0.5)
	for i in 60:
		weather_audio._tick(0.25)
	_check(players.size() == 9 and players[0].playing and players[1].playing, "light and heavy rain loops are playing in a storm")
	_check(weather_audio.is_in_group(NightLight.GROUP) and weather_audio.is_in_group(DayCycle.CLOCK_GROUP), "the day cycle reaches the weather sound through the night-light and clock groups")
	_check(not players[7].playing, "no birds in the storm or at midday")
	_check(not players[5].playing, "no crickets in the daytime, or in the rain")
	weather_audio.set_daylight(0.0)
	for i in 4:
		weather_audio._tick(0.25)
	_check(not players[5].playing, "no crickets in a night storm: the rain drowns them")
	_check(players[2].playing and players[3].playing and is_equal_approx(weather_audio.wind, 1.0), "…and so are the breeze and the gale (wind %.2f)" % weather_audio.wind)
	_audio.set("zone", "cave")
	for i in 60:
		weather_audio._tick(0.25)
	_check(weather_audio.level == 0.0 and weather_audio.wind == 0.0 and not players[0].playing and not players[1].playing and not players[2].playing and not players[3].playing, "no rain and no wind underground")
	_check(not players[8].playing, "no rustling leaves underground")
	_audio.set("zone", "island")
	fake.set("rain", 0.0)
	fake.set("wind", 0.14)
	for i in 60:
		weather_audio._tick(0.25)
	_check(not players[0].playing and not players[1].playing, "dry weather stops the rain")
	_check(players[2].playing and not players[3].playing and weather_audio.wind > 0.15 and weather_audio.wind < 0.25, "a clear day is a quiet breeze (wind %.2f)" % weather_audio.wind)
	_check(players[5].playing, "dry night: the crickets sing")
	_check(players[8].playing, "a few island trees rustle even on a clear day")
	var island_db := players[8].volume_db
	_audio.set("zone", "mainland")
	weather_audio._tick(0.25)
	_check(players[8].volume_db > island_db + 3.0, "the mainland forest rustles louder than the island")
	_audio.set("zone", "island")
	weather_audio._owl_wait = 0.0
	weather_audio._tick(0.25)
	_check(players[6].playing, "at night the owl hoots now and then")
	weather_audio.set_daylight(1.0)
	for i in 4:
		weather_audio._tick(0.25)
	_check(not players[5].playing, "day comes, the crickets stop")
	weather_audio.set_daylight(0.0)
	_audio.set("zone", "cave")
	for i in 4:
		weather_audio._tick(0.25)
	_check(not players[5].playing, "no crickets underground")
	_audio.set("zone", "island")
	weather_audio.set_daylight(1.0)
	weather_audio.set_hour(6.5)
	for i in 4:
		weather_audio._tick(0.25)
	_check(players[7].playing and not players[5].playing, "a dry morning: the birds sing")
	_audio.set("zone", "cave")
	for i in 4:
		weather_audio._tick(0.25)
	_check(not players[7].playing, "no birds underground")
	_audio.set("zone", "island")
	fake.set("rain", 0.6)
	for i in 60:
		weather_audio._tick(0.25)
	_check(not players[7].playing, "the rain silences the birds")
	fake.set("rain", 0.0)
	weather_audio.set_hour(13.0)
	for i in 60:
		weather_audio._tick(0.25)
	_check(not players[7].playing, "by noon the chorus is over")
	weather_audio.set_hour(12.0)
	fake.queue_free()
