extends SceneTree
## One-off tool: turn the per-clip FBX files in assets/character into editable
## Animation resources + one AnimationLibrary used by scenes/player/Player.tscn.
## Run: godot --headless --path client --script res://tools/extract_anims.gd
## Re-run after adding/replacing a clip FBX. Safe to re-run: files are overwritten.

const CHARACTER_DIR := "res://assets/character/"
const ANIMS_DIR := "res://assets/character/anims/"
const LIBRARY_PATH := "res://assets/character/character_anims.tres"
const MODEL := "HumanM_Model.fbx"
## Clips that must loop; everything else is a one-shot.
const LOOPING := ["idle", "walk_forward", "walk_back", "walk_left", "walk_right",
	"run_forward", "sprint_forward", "till", "sitting_idle", "sleeping",
	"swim_idle", "swim_forward", "crouch_idle", "crouch_forward", "idle_torch", "push", "jump"]
## Multi-clip libraries (Quaternius UAL, CC0): source animation → our clip name.
## Processed after the single-clip FBX files, so a name here replaces the FBX one.
const LIBRARIES := {
	"ual2.glb": {
		# Locomotion and sitting stay on the pack/Mixamo clips: the UAL walk, jog and
		# sit read wrong on this rig (owner, 2026-09-09).
		"Idle": "idle",
		"Jump_Start": "jump_begin", "Jump": "jump", "Jump_Land": "jump_land",
		"Swim_Idle": "swim_idle", "Swim_Fwd": "swim_forward",
		"Crouch_Idle": "crouch_idle", "Crouch_Fwd": "crouch_forward",
		"Idle_Torch": "idle_torch", "Interact": "interact", "PickUp_Table": "pickup",
		"Fixing_Kneeling": "kneel", "Sword_Attack": "swing", "Push": "push",
		"Dance": "dance", "Roll": "roll", "Driving": "drive", "Hit_Chest": "hit",
	},
}


func _init() -> void:
	var skeleton := _model_skeleton()
	if skeleton == null:
		push_error("no Skeleton3D in " + MODEL)
		quit(1)
		return
	var library := AnimationLibrary.new()
	for file: String in DirAccess.get_files_at(CHARACTER_DIR):
		if not file.ends_with(".fbx") or file == MODEL:
			continue
		var clip_name := file.get_basename()
		_store(library, _extract(file, clip_name, skeleton, &""), clip_name)
	for file: String in LIBRARIES:
		var mapping: Dictionary = LIBRARIES[file]
		for source: String in mapping:
			var clip_name: String = mapping[source]
			_store(library, _extract(file, clip_name, skeleton, StringName(source)), clip_name)
	var err := ResourceSaver.save(library, LIBRARY_PATH)
	print("library %s: %d clips, save=%s" % [LIBRARY_PATH, library.get_animation_list().size(), error_string(err)])
	skeleton.get_parent().free()
	quit()


func _store(library: AnimationLibrary, anim: Animation, clip_name: String) -> void:
	if anim == null:
		return
	var path := ANIMS_DIR + clip_name + ".res"
	var err := ResourceSaver.save(anim, path)
	if err != OK:
		push_error("save failed %s: %s" % [path, error_string(err)])
		return
	anim.take_over_path(path)
	if library.has_animation(clip_name):
		library.remove_animation(clip_name)
	library.add_animation(clip_name, anim)


func _model_skeleton() -> Skeleton3D:
	var scene: PackedScene = load(CHARACTER_DIR + MODEL)
	var root: Node = scene.instantiate()
	var found: Array[Node] = root.find_children("*", "Skeleton3D", true, false)
	if found.is_empty():
		root.free()
		return null
	var mesh: MeshInstance3D = found[0].find_children("*", "MeshInstance3D", true, false)[0] as MeshInstance3D
	print("model aabb: pos=%s size=%s" % [mesh.get_aabb().position, mesh.get_aabb().size])
	return found[0] as Skeleton3D


## `source` picks one animation out of a library file; empty = the file's only clip.
func _extract(file: String, clip_name: String, skeleton: Skeleton3D, source: StringName) -> Animation:
	var scene: PackedScene = load(CHARACTER_DIR + file)
	if scene == null:
		push_error("cannot load " + file)
		return null
	var root: Node = scene.instantiate()
	var players: Array[Node] = root.find_children("*", "AnimationPlayer", true, false)
	if players.is_empty():
		push_error("no AnimationPlayer in " + file)
		root.free()
		return null
	var player: AnimationPlayer = players[0] as AnimationPlayer
	var source_name: StringName = source if source != &"" else player.get_animation_list()[0]
	if not player.has_animation(source_name):
		push_error("%s has no animation %s" % [file, source_name])
		root.free()
		return null
	var anim: Animation = player.get_animation(source_name).duplicate(true)
	anim.resource_name = clip_name
	anim.loop_mode = Animation.LOOP_LINEAR if clip_name in LOOPING else Animation.LOOP_NONE
	_rebase_tracks(anim, skeleton)
	var removed := _strip_unknown_bones(anim, skeleton)
	print("%-16s <- %-40s len=%.2f tracks=%d loop=%d removed=%s" % [clip_name, source_name, anim.length, anim.get_track_count(), anim.loop_mode, removed])
	root.free()
	return anim


## Library files keep their own node layout (`Armature/Skeleton3D:Bone`); the
## Player scene animates `Model/Skeleton3D`, so every bone track is re-pointed there.
func _rebase_tracks(anim: Animation, skeleton: Skeleton3D) -> void:
	var target := String(skeleton.get_parent().get_path_to(skeleton))
	for t: int in anim.get_track_count():
		var path := anim.track_get_path(t)
		if path.get_subname_count() > 0:
			anim.track_set_path(t, NodePath(target + ":" + String(path.get_subname(0))))


## Drops tracks that target bones the model skeleton does not have (proxy bones
## exist only in the clip files) so AnimationPlayer never logs unresolved tracks.
func _strip_unknown_bones(anim: Animation, skeleton: Skeleton3D) -> PackedStringArray:
	var removed: PackedStringArray = []
	var i := anim.get_track_count() - 1
	while i >= 0:
		var path := anim.track_get_path(i)
		if path.get_subname_count() > 0:
			var bone := String(path.get_subname(0))
			if skeleton.find_bone(bone) == -1:
				if not removed.has(bone):
					removed.append(bone)
				anim.remove_track(i)
		i -= 1
	return removed
