extends SceneTree
## Re-extracts multi-part objects from the farm pack FBX keeping the original part
## transforms (the week-1 cut lost them for buildings). The group's basis is kept
## (it carries the pack's up-axis fix), its origin is dropped, and the whole thing is
## shifted so it stands on y = 0 centred on x/z. Meshes are embedded, the shared atlas
## material is applied, and the result overwrites assets/farm_pack/pieces/<name>.tscn.
## Run: godot --headless --path client --script res://tools/extract_group.gd -- --names=littlehouse_a,farmhouse_a01
##      [--fbx=res://farm_builds/Pack_build_2/LowPoly_Farm_02_FBX.fbx --atlas=res://assets/farm_pack/farm_pack2_atlas.tres --fix-normals]
## (другий пак будівель власника, 2026-09-16: ті самі правила, інший файл і атлас;
## його нормалі вивернуті проти обходу трикутників, тож `--fix-normals` перераховує їх з обходу).

const FBX := "res://farm_builds/LowPoly_Farm_Pack_fbx.FBX"
const ATLAS := "res://assets/farm_pack/farm_pack_atlas.tres"
const OUT_DIR := "res://assets/farm_pack/pieces/"

var _fix_normals := false


func _initialize() -> void:
	var names: PackedStringArray = []
	var fbx_path := FBX
	var atlas_path := ATLAS
	_fix_normals = false
	for arg: String in OS.get_cmdline_user_args():
		if arg.begins_with("--names="):
			names = arg.trim_prefix("--names=").split(",", false)
		elif arg.begins_with("--fbx="):
			fbx_path = arg.trim_prefix("--fbx=")
		elif arg.begins_with("--atlas="):
			atlas_path = arg.trim_prefix("--atlas=")
		elif arg == "--fix-normals":
			_fix_normals = true
	var scene := load(fbx_path) as PackedScene
	if scene == null:
		push_error("fbx not loaded")
		quit(1)
		return
	var fbx: Node = scene.instantiate()
	var atlas := load(atlas_path) as Material
	var failures := 0
	for name: String in names:
		var group := _find_group(fbx, name)
		if group == null:
			push_error("no group for " + name)
			failures += 1
			continue
		var err := _save_piece(group, name, atlas)
		if err != OK:
			push_error("save failed for %s: %d" % [name, err])
			failures += 1
	fbx.free()
	print("extract: %d ok, %d failed" % [names.size() - failures, failures])
	quit(1 if failures > 0 else 0)


## First top-level node named `<name>001` / `<name>_001` (the pack numbers duplicates).
func _find_group(fbx: Node, name: String) -> Node3D:
	var wanted := name.to_lower()
	for child: Node in fbx.get_children():
		var n := String(child.name).to_lower()
		if n == wanted:
			return child as Node3D
		# "littlehouse_a001", "Stoneroad_a001", "fence_c1001", "coop_a002": strip the numeric suffix.
		var stem := n.rstrip("0123456789").trim_suffix("_")
		if stem == wanted or (n.begins_with(wanted) and n.substr(wanted.length()).lstrip("0123456789_").is_empty()):
			return child as Node3D
	return null


func _save_piece(group: Node3D, name: String, atlas: Material) -> int:
	var root := Node3D.new()
	root.name = name
	var holder := Node3D.new()
	holder.name = "Parts"
	root.add_child(holder)
	holder.owner = root
	var copy := group.duplicate() as Node3D
	copy.name = "Group"
	copy.transform = Transform3D(group.transform.basis, Vector3.ZERO)
	holder.add_child(copy)
	_adopt(copy, root, atlas)
	# Stand on the ground, centred: measure in root space, then move the holder.
	var box := _bounds(copy)
	holder.position = Vector3(-box.get_center().x, -box.position.y, -box.get_center().z)
	var packed := PackedScene.new()
	var err := packed.pack(root)
	if err != OK:
		return err
	err = ResourceSaver.save(packed, OUT_DIR + name + ".tscn")
	root.free()
	return err


## Sets owners for packing, embeds mesh copies, applies the atlas material.
func _adopt(node: Node, owner: Node, atlas: Material) -> void:
	node.owner = owner
	if node is MeshInstance3D:
		var mi := node as MeshInstance3D
		if mi.mesh != null:
			var mesh := mi.mesh.duplicate(true) as Mesh
			mesh.resource_path = ""
			if _fix_normals:
				mesh = _regen_normals(mesh)
			mi.mesh = mesh
		mi.material_override = atlas
	for child: Node in node.get_children():
		_adopt(child, owner, atlas)


## Merged AABB of all meshes under `node`, in the space of `node`'s parent (no tree needed).
func _bounds(node: Node3D) -> AABB:
	var out := {"box": AABB(), "first": true}
	_accumulate(node, node.transform, out)
	return out["box"]


func _accumulate(node: Node, xf: Transform3D, out: Dictionary) -> void:
	if node is MeshInstance3D and (node as MeshInstance3D).mesh != null:
		var b: AABB = xf * (node as MeshInstance3D).get_aabb()
		out["box"] = b if out["first"] else (out["box"] as AABB).merge(b)
		out["first"] = false
	for child: Node in node.get_children():
		var child_xf := xf * (child as Node3D).transform if child is Node3D else xf
		_accumulate(child, child_xf, out)


## Перебудовує кожну поверхню з обходу трикутників: нормалі паку 2 дивляться всередину,
## і на сонці міст був чорним.
static func _regen_normals(mesh: Mesh) -> ArrayMesh:
	var out := ArrayMesh.new()
	for s: int in mesh.get_surface_count():
		var st := SurfaceTool.new()
		st.create_from(mesh, s)
		st.generate_normals()
		st.index()
		st.commit(out)
		out.surface_set_material(s, mesh.surface_get_material(s))
	return out
