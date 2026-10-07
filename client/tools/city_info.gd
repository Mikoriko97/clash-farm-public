extends SceneTree
## Друкує для кожної будівлі площі: світову позицію дверей і AABB моделі (перевірка розміщення).


func _init() -> void:
	var city: Node3D = (load("res://scenes/city/City.tscn") as PackedScene).instantiate()
	root.add_child(city)
	await process_frame
	for b: Node in city.find_children("*", "CityBuilding", false, false):
		var piece: Node3D = b.get_node("Piece")
		var box := AABB()
		var first := true
		for mi: Node in piece.find_children("*", "MeshInstance3D", true, false):
			var m := mi as MeshInstance3D
			var gb := m.global_transform * m.get_aabb()
			box = gb if first else box.merge(gb)
			first = false
		var door: Vector3 = b.approach_target()
		print("%s origin=%s door=%s box_min=%s box_max=%s" % [b.name, (b as Node3D).global_position, door, box.position, box.end])
	quit()
