extends SceneTree
## Друкує сонце/місяць і напрям сонця для неба по годинах — перевірка плавності:
## godot --headless --path client --script res://tools/inspect_daycycle.gd


func _init() -> void:
	var scene: Node3D = (load("res://scenes/island/Island.tscn") as PackedScene).instantiate()
	root.add_child(scene)
	var cycle: DayCycle = scene.get_node("DayCycle")
	var sun: DirectionalLight3D = scene.get_node("Sun")
	var env: Environment = (scene.get_node("WorldEnvironment") as WorldEnvironment).environment
	var sky := env.sky.sky_material as ShaderMaterial
	print("hour  energy  light_elev  sky_sun_y")
	for h: float in [4.5, 5.0, 5.5, 6.0, 6.5, 7.0, 12.0, 18.0, 18.5, 19.0, 19.5, 20.0, 20.5, 20.9, 21.0, 21.1, 21.3, 21.6, 22.5]:
		cycle.freeze(h)
		var d: Vector3 = sky.get_shader_parameter("sun_dir")
		print("%5.1f  %6.3f  %6.1f  %6.3f" % [h, sun.light_energy, -sun.rotation_degrees.x, d.y])
	quit()
