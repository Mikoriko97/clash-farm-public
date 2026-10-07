extends SceneTree
## Кладе авторські кліпи бази 4,5 голови в бібліотеку персонажа `character_anims.tres`: дії з інструментами (ADR-034,
## `tool_anims.glb`, генератор `build_tool_anims.py`), «життя» — варіанти спокою, реакції, емоції (ADR-035, `life_anims.glb`,
## `build_life_anims.py`) і ходьба з бігом (`locomotion_anims.glb`, `build_locomotion.py`); усі GLB імпортуються з ретаргетом `bone_map_base45.tres` і з незмінними треками (кулак, пальці).
## Кожен кліп — `assets/character/anims/<ім'я>.res`, однойменний у бібліотеці замінюється, решта не чіпається.
## Run: godot --headless --path client --script res://tools/add_tool_anims.gd

const ANIMS_DIR := "res://assets/character/anims/"
const LIBRARY_PATH := "res://assets/character/character_anims.tres"
## Джерело → кліпи; `LOOPING` — ті, що крутяться по колу (решта — одноразові).
const SOURCES: Dictionary = {
	"res://assets/character/base45/tool_anims.glb": ["chop", "mine", "dig", "reap", "hammer", "water"],
	"res://assets/character/base45/life_anims.glb": ["idle_look", "idle_stretch", "idle_scratch", "cheer", "shake_no", "wave", "laugh",
		"shrug", "thumbs_up", "heart", "clap"],
	# ходьба й біг через IK під швидкість гри (аудит анімацій 2026-10-03, `build_locomotion.py`) замість ретаргету мокапу
	"res://assets/character/base45/locomotion_anims.glb": ["walk_forward", "run_forward"],
}
const LOOPING: Array[String] = ["walk_forward", "run_forward"]


func _init() -> void:
	var library := load(LIBRARY_PATH) as AnimationLibrary
	var added := 0
	var wanted := 0
	for source: String in SOURCES:
		var clips: Array = SOURCES[source]
		wanted += clips.size()
		added += _add(library, source, clips)
	print("library: %d of %d clips added, save=%s" % [added, wanted, error_string(ResourceSaver.save(library, LIBRARY_PATH))])
	quit(0 if added == wanted else 1)


func _add(library: AnimationLibrary, source: String, clips: Array) -> int:
	var root := (load(source) as PackedScene).instantiate()
	var player := root.find_child("AnimationPlayer", true, false) as AnimationPlayer
	var skeleton := root.find_child("Skeleton3D", true, false) as Skeleton3D
	if player == null or skeleton == null or library == null:
		push_error("add_tool_anims: no AnimationPlayer/Skeleton3D in %s or no library" % source)
		root.free()
		return 0
	var added := 0
	for clip: String in clips:
		if not player.has_animation(clip):
			push_error("add_tool_anims: %s has no clip %s (has %s)" % [source, clip, player.get_animation_list()])
			continue
		var anim := player.get_animation(clip).duplicate(true) as Animation
		anim.resource_name = clip
		anim.loop_mode = Animation.LOOP_LINEAR if clip in LOOPING else Animation.LOOP_NONE
		_rebase(anim, skeleton)
		var path := ANIMS_DIR + clip + ".res"
		if ResourceSaver.save(anim, path) != OK:
			push_error("add_tool_anims: save failed " + path)
			continue
		anim.take_over_path(path)
		if library.has_animation(clip):
			library.remove_animation(clip)
		library.add_animation(clip, anim)
		added += 1
		print("%-13s len=%.2f tracks=%d" % [clip, anim.length, anim.get_track_count()])
	root.free()
	return added


## Треки файлу йдуть від його кореня (`Base45/Skeleton3D:Bone`); сцени персонажа анімують `Skeleton3D:Bone` від вузла `Base45`.
## Кістки, яких у скелеті нема, і не-кісткові треки відкидаються.
func _rebase(anim: Animation, skeleton: Skeleton3D) -> void:
	var i := anim.get_track_count() - 1
	while i >= 0:
		var path := anim.track_get_path(i)
		if path.get_subname_count() == 0 or skeleton.find_bone(String(path.get_subname(0))) < 0:
			anim.remove_track(i)
		else:
			anim.track_set_path(i, NodePath("Skeleton3D:" + String(path.get_subname(0))))
		i -= 1
