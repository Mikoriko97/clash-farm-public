extends RefCounted
## Меш «деталі» (сцени з паку) для MultiMesh: флора материка й острова, групи флори,
## інструменти запікання. Без `class_name` — підключається `preload`, щоб кеш класів
## редактора не був умовою запуску headless-інструментів.


## First MeshInstance3D of a scene with its transform relative to the scene root
## (farm_pack pieces carry a Z-up correction that instances must keep).
## `hint` picks a level of detail by node name (Simple Trees: `lod0_near`, `lod1_mid`,
## `lod2_far`); a piece without such names gives its first mesh.
static func first(scene: PackedScene, hint: String = "") -> Dictionary:
	var node := scene.instantiate()
	var out := {}
	var found: Array[Node] = node.find_children("*", "MeshInstance3D", true, false)
	if node is MeshInstance3D:
		out = {"mesh": (node as MeshInstance3D).mesh, "xform": Transform3D.IDENTITY}
	elif not found.is_empty():
		var mi := found[0] as MeshInstance3D
		var matched := false
		if not hint.is_empty():
			for candidate: Node in found:
				if String(candidate.name).contains(hint):
					mi = candidate as MeshInstance3D
					matched = true
					break
		var xform := mi.transform
		var parent := mi.get_parent()
		while parent != null and parent != node and parent is Node3D:
			xform = (parent as Node3D).transform * xform
			parent = parent.get_parent()
		out = {"mesh": mi.mesh, "xform": xform, "matched": matched}
	node.free()
	return out
