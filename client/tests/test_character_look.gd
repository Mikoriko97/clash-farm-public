extends SceneTree
## Headless test for scripts/character_look.gd (ADR-025; 4.5-head base — ADR-032): four colours repaint the body's
## palette zones, junk colours fall back to the defaults, every body vertex points at a cell centre with forearms and hands
## in the skin cell, equal looks share one material, the face decal follows `look.face`, and the local player follows
## `player_look` while a peer avatar follows its own `look`.
## Run: godot --headless --path client --script res://tests/test_character_look.gd

const RED := {"skin": "#ff0000", "shirt": "#00ff00", "pants": "#0000ff", "shoes": "#ffff00"}
const BLUE := {"skin": "#102030", "shirt": "#203040", "pants": "#304050", "shoes": "#405060"}

var _failures := 0
var _frames := 0
var _player: Node
var _peer: PeerAvatar


func _check(ok: bool, what: String) -> void:
	if not ok:
		_failures += 1
		push_error("FAIL: " + what)


func _init() -> void:
	_player = (load("res://scenes/player/Player.tscn") as PackedScene).instantiate()
	root.add_child(_player)
	_peer = (load("res://scenes/player/PeerAvatar.tscn") as PackedScene).instantiate() as PeerAvatar
	root.add_child(_peer)


func _process(_delta: float) -> bool:
	_frames += 1
	if _frames < 3:
		return false
	_check_colors()
	_check_mesh()
	_check_repaint()
	_check_parts()
	_check_outfit()
	_check_body()
	_check_lod()
	_check_builds()
	print("RESULT: %s (%d failures)" % ["PASS" if _failures == 0 else "FAIL", _failures])
	quit(1 if _failures > 0 else 0)
	return true


func _body(node: Node) -> MeshInstance3D:
	return node.find_child(CharacterLook.BODY_NODE, true, false) as MeshInstance3D


func _cell(node: Node, cell: int) -> String:
	var material := _body(node).get_surface_override_material(0) as StandardMaterial3D
	return material.albedo_texture.get_image().get_pixel(cell, 0).to_html(false)


func _check_colors() -> void:
	var defaults := CharacterLook.colors_of(null)
	_check(defaults["skin"] == CharacterLook.DEFAULTS["skin"] and defaults.size() == 7, "no look = the defaults (colours + the derived trim)")
	var mixed := CharacterLook.colors_of({"skin": "#ff0000", "shirt": "nope", "pants": 5, "shoes": "#12345", "hat": "#ffffff"})
	_check(mixed["skin"] == Color("#ff0000"), "a valid colour is taken")
	_check(mixed["shirt"] == defaults["shirt"] and mixed["pants"] == defaults["pants"] and mixed["shoes"] == defaults["shoes"], "junk colours fall back")
	_check(not mixed.has("hat"), "unknown slots are ignored")
	# Fresh scenes already wear the default look, not the grey mannequin.
	_check(_cell(_player, 14) == CharacterLook.DEFAULTS["skin"].to_html(false), "player starts in the default look")
	_check(_body(_peer) != null, "peer scene has a body")


func _check_mesh() -> void:
	var body := _body(_player)
	var arrays := body.mesh.surface_get_arrays(0)
	var uvs: PackedVector2Array = arrays[Mesh.ARRAY_TEX_UV]
	var verts: PackedVector3Array = arrays[Mesh.ARRAY_VERTEX]
	var bones := (arrays[Mesh.ARRAY_BONES] as PackedInt32Array).size()
	_check(body.skin != null and bones >= verts.size() * 4 and bones % verts.size() == 0, "skinned (%d weights per vertex)" % (bones / maxi(verts.size(), 1)))
	var used := {}
	var hand_cell := -1
	var widest := 0.0
	var head_cell := -1
	var highest := -9.0
	var foot_cell := -1
	var lowest := 9.0
	for i in uvs.size():
		var cell := int(floor(uvs[i].x * float(CharacterLook.CELLS)))
		used[cell] = true
		_check(absf(uvs[i].x * float(CharacterLook.CELLS) - float(cell) - 0.5) < 0.001 and absf(uvs[i].y - 0.5) < 0.001, "uv %d sits at a cell centre" % i)
		if absf(verts[i].x) > widest:
			widest = absf(verts[i].x)
			hand_cell = cell
		if verts[i].y > highest:
			highest = verts[i].y
			head_cell = cell
		if verts[i].y < lowest:
			lowest = verts[i].y
			foot_cell = cell
	_check(hand_cell == CharacterLook.ARM_SKIN_CELL, "the hand tip is skin: cell %d" % hand_cell)
	_check(head_cell in CharacterLook.SKIN_CELLS, "the head top is skin: cell %d" % head_cell)
	_check(foot_cell in CharacterLook.SHOE_CELLS, "the sole is shoe: cell %d" % foot_cell)
	_check(used.has(CharacterLook.SHIRT_CELLS[0]) and used.has(CharacterLook.PANTS_CELLS[0]) and used.has(CharacterLook.SHOE_CELLS[0]), "shirt, trousers and shoes have their cells")
	# Trousers end at the waist (0.86 model units): no trouser vertex on the chest.
	var lip := 0
	for i in uvs.size():
		if verts[i].y > 0.9 and int(floor(uvs[i].x * float(CharacterLook.CELLS))) in CharacterLook.PANTS_CELLS:
			lip += 1
	_check(lip == 0, "no trouser colour above the waist (%d vertices)" % lip)


func _check_repaint() -> void:
	CharacterLook.on_bridge("player_look", {"look": RED}, _player)
	_check(_cell(_player, 14) == "ff0000" and _cell(_player, 13) == "00ff00" and _cell(_player, 10) == "0000ff" and _cell(_player, 7) == "ffff00", "player_look repaints the four slots")
	_check(_cell(_player, CharacterLook.ARM_SKIN_CELL) == "ff0000", "forearms and hands follow the skin")
	CharacterLook.on_bridge("something_else", {"look": BLUE}, _player)
	_check(_cell(_player, 14) == "ff0000", "other messages do nothing")
	_peer.apply({"id": "p1", "nick": "Peer", "x_dm": 0, "z_dm": 0, "yaw_deg": 0, "anim": "idle", "look": RED}, true)
	_check(_body(_peer).get_surface_override_material(0) == _body(_player).get_surface_override_material(0), "equal looks share one material")
	_peer.apply({"id": "p1", "nick": "Peer", "x_dm": 0, "z_dm": 0, "yaw_deg": 0, "anim": "idle", "look": BLUE})
	_check(_cell(_peer, 10) == "304050" and _cell(_player, 10) == "0000ff", "a peer has its own look")
	_peer.apply({"id": "p1", "nick": "Peer", "x_dm": 0, "z_dm": 0, "yaw_deg": 0, "anim": "idle"})
	_check(_cell(_peer, 14) == CharacterLook.DEFAULTS["skin"].to_html(false), "a peer without a look gets the default one")


func _mount(node: Node) -> Node3D:
	return node.find_child("Mount", true, false) as Node3D


func _face_cell(node: Node) -> int:
	var face := node.find_child(CharacterFace.NODE, true, false) as MeshInstance3D
	var mat := face.get_surface_override_material(0) as StandardMaterial3D if face != null else null
	if mat == null:
		return -1
	return int(round(mat.uv1_offset.x * CharacterFace.COLS)) + int(round(mat.uv1_offset.y * CharacterFace.ROWS)) * CharacterFace.COLS


func _check_parts() -> void:
	# Nothing chosen yet: no head parts, and the face decal shows the calm expression of the atlas.
	var bare := _mount(_peer)
	_check(bare == null or bare.get_child_count() == 0, "the default look wears no head parts")
	_check(_face_cell(_peer) == CharacterFace.CELLS["face_neutral"], "the default face is the calm one")
	var look := RED.duplicate()
	look.merge({"hair": "hair_short", "face": "face_smile", "beard": "", "hat": "hat_straw", "glasses": "glasses_round", "hair_color": "#ff0000"})
	CharacterLook.on_bridge("player_look", {"look": look}, _player)
	var mount := _mount(_player)
	var names: Array[String] = []
	for c in mount.get_children():
		names.append(String(c.name))
	names.sort()
	_check(names == ["glasses", "hair", "hat"], "the chosen parts are mounted, one per slot: %s" % [names])
	_check(String(mount.get_node("hat").get_meta("part_id")) == "hat_straw" and _face_cell(_player) == CharacterFace.CELLS["face_smile"], "each slot holds the requested part, the face its expression")
	_check(CharacterFace.material(1) == CharacterFace.material(1), "an expression is one shared material")
	# The hat sits on the head: above the neck bone, inside the character's scaled body.
	var hat_mesh := CharacterParts.meshes_of(mount.get_node("hat"))[0]
	var centre := (hat_mesh.global_transform * hat_mesh.get_aabb()).get_center()
	var above_feet: float = centre.y - _player.global_position.y
	_check(above_feet > 1.3 and above_feet < 1.5 and absf(centre.x) < 0.2, "the hat is over the head (%.2f m above the feet): %s" % [above_feet, centre])
	# role_hair follows hair_color, baked materials stay as exported.
	var hair_mesh := CharacterParts.meshes_of(mount.get_node("hair"))[0]
	_check((hair_mesh.get_surface_override_material(0) as StandardMaterial3D).albedo_color == Color("#ff0000"), "the hair takes the hair colour")
	var hat_material := hat_mesh.get_surface_override_material(0)
	_check(hat_material == null, "a hat keeps its own colours")
	# Changing a slot swaps the part; an empty id clears it; junk and foreign ids are ignored.
	look["hat"] = ""
	look["hair"] = "hair_long"
	CharacterLook.on_bridge("player_look", {"look": look}, _player)
	_check(mount.get_node_or_null("hat") == null and String(mount.get_node("hair").get_meta("part_id")) == "hair_long", "an empty id clears a slot and a new id swaps the part")
	look["hair"] = "hair_nope"
	look["beard"] = "hat_cap"
	look["face"] = 7
	CharacterLook.on_bridge("player_look", {"look": look}, _player)
	_check(mount.get_node_or_null("hair") == null and mount.get_node_or_null("beard") == null, "unknown ids and ids of another slot are ignored")
	_check(_face_cell(_player) == CharacterFace.CELLS["face_neutral"], "a bad face falls back to the calm one")
	_check(CharacterParts.template("hair_short") != null and CharacterParts.template("nope") == null, "templates come from the parts library")
	# A peer with the same look shares the hair material.
	_peer.apply({"id": "p2", "nick": "P", "x_dm": 0, "z_dm": 0, "yaw_deg": 0, "anim": "idle", "look": look})
	_check(_mount(_peer).get_node_or_null("hair") == null, "the peer follows its own look, not the player's")
	# A hat over the whole head flattens bulky hair into the short cut; a low band or wreath and long hair stay as they are.
	_check(CharacterParts.ids_of({"hair": "hair_bun", "hat": "hat_cap"})["hair"] == "hair_short", "a cap flattens a bun into the short cut")
	_check(CharacterParts.ids_of({"hair": "hair_afro", "hat": "hat_straw"})["hair"] == "hair_short", "a straw hat flattens an afro")
	_check(CharacterParts.ids_of({"hair": "hair_bun", "hat": "hat_headband"})["hair"] == "hair_bun", "a headband lets the bun stay")
	_check(CharacterParts.ids_of({"hair": "hair_long", "hat": "hat_cap"})["hair"] == "hair_long", "long hair is not flattened")


func _outfit(node: Node) -> MeshInstance3D:
	var sk := node.find_child("Skeleton3D", true, false) as Skeleton3D
	return sk.get_node_or_null("Outfit") as MeshInstance3D


func _check_outfit() -> void:
	_check(_outfit(_player) == null, "no outfit unless one is chosen")
	var look := RED.duplicate()
	look.merge({"outfit": "outfit_jacket", "outfit_color": "#ff8800"})
	CharacterLook.on_bridge("player_look", {"look": look}, _player)
	var jacket := _outfit(_player)
	_check(jacket != null and String(jacket.get_meta("part_id")) == "outfit_jacket", "the jacket is on")
	var skeleton := _player.find_child("Skeleton3D", true, false) as Skeleton3D
	_check(jacket.get_parent() == skeleton and jacket.get_node(jacket.skeleton) == skeleton, "it is skinned to the character's skeleton")
	# Bind poses come from the game's own rest pose, so the shell sits on the body exactly like the body's own skin.
	var head := skeleton.find_bone("Head")
	var bound := false
	for i in jacket.skin.get_bind_count():
		if jacket.skin.get_bind_name(i) == "Head":
			bound = jacket.skin.get_bind_pose(i).is_equal_approx(skeleton.get_bone_global_rest(head).affine_inverse())
	_check(bound and jacket.skin.get_bind_count() >= 50, "bind poses are the rest inverse of the game's bones (%d binds)" % jacket.skin.get_bind_count())
	var material := jacket.get_surface_override_material(0) as StandardMaterial3D
	_check(material != null and material.albedo_color == Color("#ff8800"), "the cloth takes the outfit colour")
	# It hugs the torso: its box sits over the chest, arms included.
	var box := jacket.get_aabb()
	_check(box.position.y > 0.75 and box.end.y < 1.45 and box.size.x > 1.0, "the jacket covers torso and arms: %s" % box)
	# Same outfit twice keeps the node; another swaps it; empty and junk clear it.
	CharacterLook.on_bridge("player_look", {"look": look}, _player)
	_check(_outfit(_player) == jacket, "the same outfit is not rebuilt")
	look["outfit"] = "outfit_dress"
	CharacterLook.on_bridge("player_look", {"look": look}, _player)
	_check(String(_outfit(_player).get_meta("part_id")) == "outfit_dress", "another outfit replaces it")
	look["outfit"] = "hat_cap"
	CharacterLook.on_bridge("player_look", {"look": look}, _player)
	_check(_outfit(_player) == null, "an id of another kind is ignored")
	# Free cloth is seen from both sides (a knee lifting the hem folds it over): no back-face culling on any surface.
	look["outfit"] = "outfit_skirt"
	CharacterLook.on_bridge("player_look", {"look": look}, _player)
	var skirt := _outfit(_player)
	var two_sided := skirt != null and skirt.mesh.get_surface_count() >= 1
	for surface in skirt.mesh.get_surface_count():
		var cloth := (skirt.get_surface_override_material(surface) if skirt.get_surface_override_material(surface) != null else skirt.mesh.surface_get_material(surface)) as BaseMaterial3D
		two_sided = two_sided and cloth != null and cloth.cull_mode == BaseMaterial3D.CULL_DISABLED
	_check(two_sided, "outfit cloth is double-sided")
	look["outfit"] = "outfit_cape"
	CharacterLook.on_bridge("player_look", {"look": look}, _player)
	look["outfit"] = ""
	CharacterLook.on_bridge("player_look", {"look": look}, _player)
	_check(_outfit(_player) == null, "an empty id takes the outfit off")
	_check(CharacterOutfit.template("outfit_vest") != null and CharacterOutfit.template("outfit_nope") == null, "templates come from the outfit library")


func _check_body() -> void:
	_check(CharacterBody.key_of(null) == "body_m" and CharacterBody.key_of({"body": "body_f"}) == "body_f", "the body defaults to male, body_f asks for the female one")
	_check(CharacterBody.key_of({"body": "body_x"}) == "body_m" and CharacterBody.key_of({"body": 1}) == "body_m", "junk body ids stay male")
	var skeleton := _player.find_child("Skeleton3D", true, false) as Skeleton3D
	_check(skeleton.get_node_or_null(^"body_f") == null, "the female mesh is only built when asked for")
	var look := RED.duplicate()
	look["body"] = "body_f"
	CharacterLook.on_bridge("player_look", {"look": look}, _player)
	var female := skeleton.get_node_or_null(^"body_f") as MeshInstance3D
	_check(female != null and female.visible and not _body(_player).visible, "the female body shows and the male one hides")
	_check(female.get_node(female.skeleton) == skeleton and female.skin != null and female.skin.get_bind_count() >= 50, "it is skinned to the same skeleton (%d binds)" % female.skin.get_bind_count())
	var material := female.get_surface_override_material(0) as StandardMaterial3D
	_check(material != null and material.albedo_texture.get_image().get_pixel(14, 0).to_html(false) == "ff0000", "it is painted with the same palette")
	var uvs: PackedVector2Array = female.mesh.surface_get_arrays(0)[Mesh.ARRAY_TEX_UV]
	var at_centres := true
	for uv in uvs:
		at_centres = at_centres and absf(uv.x * float(CharacterLook.CELLS) - floorf(uv.x * float(CharacterLook.CELLS)) - 0.5) < 0.001 and absf(uv.y - 0.5) < 0.001
	_check(uvs.size() > 1000 and at_centres, "every female vertex points at a palette cell centre")
	var male_box := _body(_player).mesh.get_aabb()
	_check(absf(female.mesh.get_aabb().size.y - male_box.size.y) < 0.02 and absf(female.mesh.get_aabb().size.x - male_box.size.x) < 0.05, "same height and reach: the animations fit both")
	var face := _player.find_child(CharacterFace.NODE, true, false) as MeshInstance3D
	_check(face != null and face.visible and face.get_node(face.skeleton) == skeleton, "the face stays on the head of the same skeleton")
	# Outfits come from the library of the body that wears them and are rebuilt when the body changes.
	look["outfit"] = "outfit_jacket"
	CharacterLook.on_bridge("player_look", {"look": look}, _player)
	var jacket := _outfit(_player)
	var cut := CharacterOutfit.template("outfit_jacket", "body_f").mesh
	_check(jacket != null and String(jacket.get_meta("body")) == "body_f" and jacket.mesh.get_surface_count() == cut.get_surface_count() and jacket.mesh.get_aabb().is_equal_approx(cut.get_aabb()), "the female jacket is cut on the female body")
	_check(CharacterOutfit.template("outfit_jacket", "body_f").mesh != CharacterOutfit.template("outfit_jacket").mesh, "the two bodies have their own outfit meshes")
	look["body"] = "body_m"
	CharacterLook.on_bridge("player_look", {"look": look}, _player)
	_check(_body(_player).visible and not female.visible, "back to the male body")
	_check(String(_outfit(_player).get_meta("body")) == "body_m", "the outfit follows the body back")
	look["body"] = "body_f"
	look["outfit"] = ""
	_peer.apply({"id": "p3", "nick": "P", "x_dm": 0, "z_dm": 0, "yaw_deg": 0, "anim": "idle", "look": look})
	_check(not _body(_peer).visible and _body(_player).visible, "a peer wears its own body, independent of the player's")


## LOD (ADR-028): рівні імпорту живуть у перебудованих мешах і вмикаються за розумних відстаней; дрібні деталі не тягнуть тіней.
func _check_lod() -> void:
	var look := RED.duplicate()
	look.merge({"hair": "hair_short", "beard": "beard_full", "glasses": "glasses_round", "outfit": "outfit_jacket", "body": "body_m"})
	CharacterLook.on_bridge("player_look", {"look": look}, _player)
	var skeleton := _player.find_child("Skeleton3D", true, false) as Skeleton3D
	var male := _body(_player).mesh
	_check(MeshLod.count_of(male, 0) >= 3 and MeshLod.first_error_of(male, 0) < 0.05, "the remapped male body keeps its LODs (%d, first error %.3f m)" % [MeshLod.count_of(male, 0), MeshLod.first_error_of(male, 0)])
	var raw := CharacterBody.template("body_f").mesh
	_check(MeshLod.first_error_of(raw, 0) < 0.05, "the base GLBs report their LOD error in metres (armature at scale 1)")
	look["body"] = "body_f"
	CharacterLook.on_bridge("player_look", {"look": look}, _player)
	var female := (skeleton.get_node(^"body_f") as MeshInstance3D).mesh
	_check(MeshLod.count_of(female, 0) >= 3 and MeshLod.first_error_of(female, 0) < 0.05, "the female body has working LODs (first error %.3f m)" % MeshLod.first_error_of(female, 0))
	var jacket := (skeleton.get_node(^"Outfit") as MeshInstance3D).mesh
	_check(MeshLod.count_of(jacket, 0) >= 3 and MeshLod.first_error_of(jacket, 0) < 0.05, "the outfit shell has working LODs (first error %.3f m)" % MeshLod.first_error_of(jacket, 0))
	# Tiny details stop being drawn (and stop casting shadows) far from the camera; hair keeps both.
	var mount := _mount(_player)
	var face := _player.find_child(CharacterFace.NODE, true, false) as MeshInstance3D
	_check(face.visibility_range_end > 0.0 and face.cast_shadow == GeometryInstance3D.SHADOW_CASTING_SETTING_OFF, "the face is hidden and shadowless at a distance")
	for slot in ["beard", "glasses"]:
		for node in CharacterParts.meshes_of(mount.get_node(slot)):
			_check(node.visibility_range_end > 0.0 and node.cast_shadow == GeometryInstance3D.SHADOW_CASTING_SETTING_OFF, "%s is hidden and shadowless at a distance" % slot)
	for node in CharacterParts.meshes_of(mount.get_node("hair")):
		_check(node.visibility_range_end == 0.0 and node.cast_shadow != GeometryInstance3D.SHADOW_CASTING_SETTING_OFF, "hair stays drawn with its shadow")


## Статури (ADR-029): ключ тіла = стать + статура; кожне тіло — свій вузол на скелеті, свій набір вбрання, свої LOD.
func _check_builds() -> void:
	var expect := {
		{"body": "body_m", "build": "build_avg"}: "body_m", {"body": "body_m", "build": "build_slim"}: "body_m_slim", {"body": "body_m", "build": "build_sturdy"}: "body_m_sturdy",
		{"body": "body_f", "build": "build_avg"}: "body_f", {"body": "body_f", "build": "build_slim"}: "body_f_slim", {"body": "body_f", "build": "build_sturdy"}: "body_f_sturdy",
		{"build": "build_slim"}: "body_m_slim", {"body": "body_f", "build": "build_nope"}: "body_f", {"body": "body_m", "build": 2}: "body_m",
	}
	for look: Dictionary in expect:
		_check(CharacterBody.key_of(look) == expect[look], "%s wears %s (got %s)" % [look, expect[look], CharacterBody.key_of(look)])
	_check(CharacterBody.outfit_path_of("body_m").ends_with("/outfits.glb") and CharacterBody.outfit_path_of("body_f_sturdy").ends_with("/outfits_f_sturdy.glb"), "each body has its own outfit set")
	var skeleton := _player.find_child("Skeleton3D", true, false) as Skeleton3D
	var look := RED.duplicate()
	look.merge({"hair": "hair_short", "outfit": "outfit_jacket"})
	var heights: Array[float] = []
	for key: String in ["body_m_slim", "body_m_sturdy", "body_f_slim", "body_f_sturdy", "body_f", "body_m"]:
		look["body"] = "body_f" if key.begins_with("body_f") else "body_m"
		look["build"] = "build_slim" if key.ends_with("_slim") else ("build_sturdy" if key.ends_with("_sturdy") else "build_avg")
		CharacterLook.on_bridge("player_look", {"look": look}, _player)
		var shown: Array[String] = []
		for child in skeleton.get_children():
			if child is MeshInstance3D and (child.name == CharacterBody.MALE_NODE or String(child.name).begins_with("body_")) and (child as MeshInstance3D).visible:
				shown.append(String(child.name))
		var active := CharacterBody.MALE_NODE if key == "body_m" else key
		_check(shown == [active], "%s is the only body shown (%s)" % [key, shown])
		var mesh := (skeleton.get_node(NodePath(active)) as MeshInstance3D).mesh
		_check(MeshLod.count_of(mesh, 0) >= 3 and MeshLod.first_error_of(mesh, 0) < 0.05, "%s has working LODs" % key)
		heights.append(mesh.get_aabb().size.y)
		var worn := skeleton.get_node_or_null(^"Outfit") as MeshInstance3D
		_check(worn != null and String(worn.get_meta("body")) == key and MeshLod.first_error_of(worn.mesh, 0) < 0.05, "%s wears its own jacket" % key)
	for h in heights:
		_check(absf(h - heights[0]) < 0.02, "every body is as tall as the others (%.3f vs %.3f)" % [h, heights[0]])
	# Every outfit exists for every body.
	for key: String in ["body_m", "body_f", "body_m_slim", "body_m_sturdy", "body_f_slim", "body_f_sturdy"]:
		for outfit: String in ["outfit_vest", "outfit_jacket", "outfit_hoodie", "outfit_overalls", "outfit_apron", "outfit_cape", "outfit_skirt", "outfit_dress"]:
			_check(CharacterOutfit.template(outfit, key) != null, "%s has %s" % [key, outfit])
