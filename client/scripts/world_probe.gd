class_name WorldProbe
extends RefCounted
## Dev probe behind `debug_world { probe: true }` → `debug_probe` (docs/BRIDGE_PROTOCOL.md): where the
## character and the ground really are in this build (dev only; the web export is checked with it).
## Moved out of IslandInput unchanged (2026-10-06) to keep that script under 300 lines.


static func report(scene: Node3D, player: Player, camera_rig: Node3D) -> Dictionary:
	var p := player.global_position
	var terrain: Node = scene.get_node_or_null(^"Terrain")
	var out := {"x": p.x, "y": p.y, "z": p.z, "on_floor": player.is_on_floor(), "cam_yaw_deg": rad_to_deg(camera_rig.rotation.y)}
	if terrain != null:
		out["ground_y"] = float(terrain.call("height_at", p.x, p.z))
	# What physics thinks the ground is: a ray down through the walk layer, the character excluded.
	var query := PhysicsRayQueryParameters3D.create(p + Vector3(0.0, 5.0, 0.0), p - Vector3(0.0, 30.0, 0.0), Interaction.WALK_LAYER)
	query.exclude = [player.get_rid()]
	var hit := player.get_world_3d().direct_space_state.intersect_ray(query)
	if hit.has("position"):
		out["physics_ground_y"] = (hit["position"] as Vector3).y
		out["physics_body"] = str((hit["collider"] as Node).name)
	out["physics_engine"] = str(ProjectSettings.get_setting("physics/3d/physics_engine", "?"))
	var city: Node3D = scene.get_node_or_null(^"City")
	if city != null:
		out["city_y"] = city.global_position.y
		var lowest := INF
		for mi: Node in city.find_children("*", "MeshInstance3D", true, false):
			var gb: AABB = (mi as MeshInstance3D).global_transform * (mi as MeshInstance3D).get_aabb()
			lowest = minf(lowest, gb.position.y)
		out["city_lowest_y"] = lowest
	return out
