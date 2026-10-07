extends SceneTree
## Headless test: стилізована база 4,5 голови (`assets/character/base45/base45.glb`, генератор
## `client/blender/characters/build_base45.py`) готова до анімацій гри — скелет ретаргетовано через BoneMap (motion_scale
## = висота стегон), кожна кістка, яку чіпають треки `character_anims.tres`, є в скелеті; зріст ≈1,80 і голова ≈0,40
## одиниці моделі (4,5 голови); тіло в бюджеті трикутників; наліпка обличчя на кістці голови. Без помилок рушія.
## Run: godot --headless --path client --script res://tests/test_base45.gd

const ErrorLog := preload("res://tests/error_log.gd")
const BASE := "res://assets/character/base45/base45.glb"
const ANIMS := "res://assets/character/character_anims.tres"
const BODY_TRIS_MAX := 6500

var _failures := 0
var _log := ErrorLog.new()


func _initialize() -> void:
	OS.add_logger(_log)
	var model := (load(BASE) as PackedScene).instantiate()
	var skeleton := model.find_child("Skeleton3D", true, false) as Skeleton3D
	_check(skeleton != null, "the model has a Skeleton3D")
	if skeleton != null:
		_check_skeleton(skeleton)
		_check_meshes(skeleton)
	model.free()
	OS.remove_logger(_log)
	_check(_log.errors.is_empty(), "no engine errors: %s" % "; ".join(_log.errors.slice(0, 3)))
	print("RESULT: %s (%d failures)" % ["PASS" if _failures == 0 else "FAIL", _failures])
	quit(1 if _failures > 0 else 0)


func _check_skeleton(skeleton: Skeleton3D) -> void:
	var hips := skeleton.get_bone_global_rest(skeleton.find_bone("Hips")).origin.y
	_check(absf(skeleton.motion_scale - hips) < 0.05, "retargeted: motion_scale %.2f = hip height %.2f" % [skeleton.motion_scale, hips])
	var lib := load(ANIMS) as AnimationLibrary
	var missing := {}
	for anim_name: StringName in lib.get_animation_list():
		var anim := lib.get_animation(anim_name)
		for i in anim.get_track_count():
			var bone := anim.track_get_path(i).get_concatenated_subnames()
			if bone != "" and skeleton.find_bone(bone) < 0:
				missing[bone] = true
	_check(missing.is_empty(), "every animated bone exists: missing %s" % ", ".join(missing.keys()))
	var head := skeleton.get_bone_global_rest(skeleton.find_bone("Head")).origin.y
	_check(absf(head - 1.40) < 0.03, "the head joint sits at the chin (%.2f)" % head)


func _check_meshes(skeleton: Skeleton3D) -> void:
	var body := skeleton.find_child("Base45_Body", false, false) as MeshInstance3D
	var face := skeleton.find_child("Base45_Face", false, false) as MeshInstance3D
	_check(body != null and face != null, "body and face decal are skinned to the skeleton")
	if body == null or face == null:
		return
	var box := body.get_aabb()
	var heads := box.end.y / maxf(box.end.y - 1.40, 0.01)
	_check(absf(box.end.y - 1.80) < 0.05 and absf(heads - 4.5) < 0.2, "1.80 tall, 4.5 heads (%.2f, %.2f)" % [box.end.y, heads])
	_check(absf(box.position.y) < 0.02, "the soles stand on the ground (%.3f)" % box.position.y)
	var tris := _tris(body.mesh)
	_check(tris > 1000 and tris <= BODY_TRIS_MAX, "body within the budget: %d triangles" % tris)
	_check(_tris(face.mesh) >= 100, "the face decal covers the face")
	var skin := face.skin
	_check(skin != null and skin.get_bind_count() >= 1, "the face decal is skinned")


func _tris(mesh: Mesh) -> int:
	var n := 0
	for s in mesh.get_surface_count():
		var arrays := mesh.surface_get_arrays(s)
		var indices: PackedInt32Array = arrays[Mesh.ARRAY_INDEX]
		n += indices.size() / 3 if indices.size() > 0 else (arrays[Mesh.ARRAY_VERTEX] as PackedVector3Array).size() / 3
	return n


func _check(ok: bool, what: String) -> void:
	if not ok:
		_failures += 1
		push_error("FAIL: " + what)
