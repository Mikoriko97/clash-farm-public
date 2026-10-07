class_name MainlandNode
extends Node3D
## One shared resource node on the mainland (docs/MAINLAND.md §5): a tree, rock,
## ore vein, herb… placed where the server says (`mainland_nodes`), drawn with a
## placeholder piece picked by `kind`. Pure view: hits and respawn are the
## server's; the character only walks up and asks.

## Kinds the character walks through (no collider): small plants.
const SOFT_KINDS: Array[String] = ["herb", "mushroom", "mushroom_rare", "reed", "tree_fallen"]

var id: String = ""
var kind: String = ""
var hits_left: int = 0
var chunk: Vector2i = Vector2i.ZERO
var _gone := false
## Owner of the node (MainlandNodes) says it stands on dry land; a used-up node stays hidden anyway.
var _placed := true

var _piece: Node3D
@onready var _highlight: MeshInstance3D = get_node_or_null(^"%Highlight")
@onready var _body: StaticBody3D = get_node_or_null(^"Body")
@onready var _shape: CollisionShape3D = get_node_or_null(^"Body/Collision")


func _ready() -> void:
	add_to_group(Interaction.GROUP)


## Gives the node its look: `scene` is a farm_pack piece (or any Node3D scene).
func setup(node_id: String, node_kind: String, scene: PackedScene) -> void:
	id = node_id
	kind = node_kind
	name = "Node_" + node_id
	if _piece != null:
		_piece.queue_free()
	_piece = scene.instantiate() as Node3D if scene != null else null
	if _piece == null:
		return
	_piece.name = "Piece"
	add_child(_piece)
	_fit_collider()


## Cylinder around the trunk or the whole rock; plants get no collider at all.
func _fit_collider() -> void:
	if _shape == null or _body == null:
		return
	if kind in SOFT_KINDS:
		_shape.disabled = true
		return
	var box := AABB()
	var first := true
	for found: Node in _piece.find_children("*", "MeshInstance3D", true, false):
		var mi := found as MeshInstance3D
		var local := (mi.global_transform.affine_inverse() * global_transform).affine_inverse()
		var b := local * mi.get_aabb()
		box = b if first else box.merge(b)
		first = false
	if first:
		return
	var radius := minf(box.size.x, box.size.z) * 0.5
	if kind.begins_with("tree"):
		radius = clampf(radius * 0.35, 0.3, 0.7)
	else:
		radius = clampf(radius * 0.9, 0.3, 1.6)
	var cylinder := CylinderShape3D.new()
	cylinder.radius = radius
	cylinder.height = maxf(box.size.y, 0.5)
	_shape.shape = cylinder
	_shape.position = Vector3(0.0, cylinder.height * 0.5, 0.0)
	_shape.disabled = false
	if _highlight != null:
		var ring := radius + 0.6
		_highlight.scale = Vector3(ring, 1.0, ring)
	# Contact shadow: the trunk's foot for trees, the whole footprint for rocks and ore.
	ContactShadow.single(self, clampf(radius * 6.0 if kind.begins_with("tree") else maxf(box.size.x, box.size.z) * 1.15, 1.2, 6.0))


func interaction_kind() -> String:
	return "mnode"


func interaction_index() -> int:
	return -1


func interaction_id() -> String:
	return id


func approach_distance() -> float:
	return maxf(1.0, Interaction.body_radius(self) + Interaction.STOP_MARGIN)


func set_highlighted(on: bool) -> void:
	if _highlight != null:
		_highlight.visible = on


func is_depleted() -> bool:
	return hits_left <= 0


## `{ id, kind, x_dm, z_dm, hits_left }` from `mainland_nodes`; position is set by the owner.
func apply(entry: Dictionary) -> void:
	hits_left = maxi(int(entry.get("hits_left", 0)), 0)
	_set_gone(is_depleted())


func set_placed(on: bool) -> void:
	_placed = on
	_show()


func _show() -> void:
	visible = _placed and not _gone


## A used-up node vanishes (owner request 2026-09-30): hidden, solid parts off, out of the
## interaction group; it comes back with the server's `hits_left` after its respawn time.
func _set_gone(gone: bool) -> void:
	if gone == _gone:
		return
	_gone = gone
	_show()
	if _shape != null:
		_shape.set_deferred("disabled", gone or kind in SOFT_KINDS)
	if gone:
		remove_from_group(Interaction.GROUP)
	elif not is_in_group(Interaction.GROUP):
		add_to_group(Interaction.GROUP)


## One hit: a short shake of the piece.
func play_fx(_fx: String) -> void:
	if _piece == null or not is_inside_tree():
		return
	var tween := create_tween()
	tween.tween_property(_piece, "rotation:z", 0.09, 0.06)
	tween.tween_property(_piece, "rotation:z", -0.06, 0.08)
	tween.tween_property(_piece, "rotation:z", 0.0, 0.1)
