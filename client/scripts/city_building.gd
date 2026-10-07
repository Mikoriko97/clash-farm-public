@tool
class_name CityBuilding
extends Node3D
## A building on the city square (docs/MAINLAND.md §7): the piece, a collider,
## a door marker the character walks to, a name plate on the facade. What the
## door opens is React's business (`player_near { kind: "building", id }`).
##
## Hand-editable in scenes/city/City.tscn: move/turn the building, set
## `door_offset` (where the character stops, just outside the wall) and
## `door_yaw_deg` (which way the door faces). Children are laid out by the
## script from those two values, because overrides of instanced children do not
## survive the web export (2026-09-09). The sign is not hung over the marker but
## on the wall behind it: a ray from the marker back into the building finds the
## facade, so plates never float in the field. A stone plinth under the piece
## hides gaps between the model's base and the ground. `@tool`: the editor shows
## the same layout as the game (sign on the facade from the model's bounds, ring
## at the door, plinth), so the owner can place buildings by eye. Dragging the
## `Door` marker or the `Sign` in the editor writes their transforms back into the
## exported values (`door_offset`/`door_yaw_deg`, `sign_offset`/`sign_yaw_deg`,
## the latter turning `sign_auto` off), so hand placement survives the export.

@export var id: String = ""
@export_enum("market", "workshop", "hall", "showcase", "monument") var kind: String = "showcase"
@export var label: String = "":
	set(v):
		label = v
		_refresh()
@export var near_radius: float = 6.0
## Where the character stops, in the building's local space (just outside the wall).
@export var door_offset: Vector3 = Vector3(0.0, 0.0, 6.0):
	set(v):
		door_offset = v
		_refresh()
## Which way the door faces, degrees around Y: 0 = local +Z, 90 = local +X.
@export_range(-180.0, 180.0) var door_yaw_deg: float = 0.0:
	set(v):
		door_yaw_deg = v
		_refresh()
## Height of the name plate above the ground (auto placement).
@export var sign_height: float = 3.2:
	set(v):
		sign_height = v
		_refresh()
## true: the plate finds the facade behind the door marker itself; false: it stays
## where `sign_offset`/`sign_yaw_deg` put it (set by dragging the Sign in the editor).
@export var sign_auto: bool = true:
	set(v):
		sign_auto = v
		_refresh()
@export var sign_offset: Vector3 = Vector3(0.0, 3.2, 6.0):
	set(v):
		sign_offset = v
		_refresh()
@export_range(-180.0, 180.0) var sign_yaw_deg: float = 0.0:
	set(v):
		sign_yaw_deg = v
		_refresh()
## Stone base under the piece: footprint of the model, this much above ground and 0.6 m below.
@export var plinth: bool = true:
	set(v):
		plinth = v
		_refresh()
@export var plinth_height: float = 0.12
@export var plinth_material: Material

var _auto_sign_pos := Vector3.ZERO
var _auto_sign_yaw := 0.0

@onready var _door: Marker3D = get_node_or_null(^"Door")
@onready var _body: StaticBody3D = get_node_or_null(^"Body")
@onready var _highlight: MeshInstance3D = get_node_or_null(^"%Highlight")
@onready var _sign: Node3D = get_node_or_null(^"%Sign")
@onready var _name: Label3D = get_node_or_null(^"%Name")


func _ready() -> void:
	if Engine.is_editor_hint():
		set_process(true)
	else:
		add_to_group(Interaction.GROUP)
	_refresh()
	if not Engine.is_editor_hint() and sign_auto:
		# The trimesh collider is queryable only after a physics step.
		_hang_sign.call_deferred()


## Editor only: the owner drags Door or Sign, the exported values follow.
func _process(_delta: float) -> void:
	if not Engine.is_editor_hint() or _door == null:
		return
	if not _door.position.is_equal_approx(door_offset) or absf(wrapf(rad_to_deg(_door.rotation.y) - door_yaw_deg, -180.0, 180.0)) > 0.01:
		door_offset = _door.position
		door_yaw_deg = rad_to_deg(_door.rotation.y)
	if _sign == null:
		return
	var expected := _expected_sign_position()
	if not _sign.position.is_equal_approx(expected) or absf(wrapf(rad_to_deg(_sign.rotation.y) - _expected_sign_yaw(), -180.0, 180.0)) > 0.01:
		sign_auto = false
		sign_offset = _sign.position
		sign_yaw_deg = rad_to_deg(_sign.rotation.y)


func _expected_sign_position() -> Vector3:
	return _auto_sign_pos if sign_auto else sign_offset


func _expected_sign_yaw() -> float:
	return _auto_sign_yaw if sign_auto else sign_yaw_deg


## Everything derived from the exported values; safe to call from setters before ready.
func _refresh() -> void:
	if not is_node_ready():
		return
	if _name != null:
		_name.text = label
	_lay_out()
	var old: Node = get_node_or_null(^"Plinth")
	if old != null:
		old.free()
	if plinth:
		_add_plinth()
	if Engine.is_editor_hint() and sign_auto and _sign != null and _door != null:
		var facing := _door.global_basis * Vector3.BACK
		facing.y = 0.0
		if facing.length() > 0.01:
			_sign.global_position = _facade_from_bounds(_door.global_position + Vector3(0.0, sign_height, 0.0), facing.normalized())
		_auto_sign_pos = _sign.position
		_auto_sign_yaw = rad_to_deg(_sign.rotation.y)


## Door marker and highlight ring from the exported door values.
func _lay_out() -> void:
	var yaw := Basis(Vector3.UP, deg_to_rad(door_yaw_deg))
	if _door != null:
		_door.position = door_offset
		_door.basis = yaw
	if _highlight != null:
		_highlight.position = Vector3(door_offset.x, 0.02, door_offset.z)
	if _sign != null and sign_auto:
		_sign.position = Vector3(door_offset.x, sign_height, door_offset.z)
		_sign.basis = yaw
	elif _sign != null:
		_sign.position = sign_offset
		_sign.basis = Basis(Vector3.UP, deg_to_rad(sign_yaw_deg))


## Slide the plate from the marker back to the facade it faces away from.
func _hang_sign() -> void:
	if _sign == null or _door == null or not is_inside_tree():
		return
	await get_tree().physics_frame
	if not is_inside_tree():
		return
	var facing := _door.global_basis * Vector3.BACK
	facing.y = 0.0
	if facing.length() < 0.01:
		return
	facing = facing.normalized()
	var from := _door.global_position + Vector3(0.0, sign_height, 0.0)
	var query := PhysicsRayQueryParameters3D.create(from, from - facing * 12.0, 1)
	var hit := get_world_3d().direct_space_state.intersect_ray(query)
	if hit.is_empty() or hit.get("collider") != _body:
		_sign.global_position = _facade_from_bounds(from, facing)
		return
	var normal: Vector3 = hit["normal"]
	normal.y = 0.0
	var out := normal.normalized() if normal.length() > 0.01 else facing
	_sign.global_position = (hit["position"] as Vector3) + out * 0.12
	_sign.look_at(_sign.global_position - out, Vector3.UP)


## No collider hit: the nearest face of the piece's bounds along the door direction.
func _facade_from_bounds(from: Vector3, facing: Vector3) -> Vector3:
	var box := _piece_bounds()
	if box.size == Vector3.ZERO:
		return from
	var local_from := to_local(from)
	var dir := (global_basis.inverse() * -facing).normalized()
	var best := local_from
	for step: int in 120:
		var p := local_from + dir * (float(step) * 0.1)
		if box.has_point(Vector3(p.x, box.position.y + 0.5, p.z)):
			best = p - dir * 0.15
			break
	return to_global(best)


func _piece_bounds() -> AABB:
	var piece: Node3D = get_node_or_null(^"Piece")
	if piece == null:
		return AABB()
	var box := AABB()
	var first := true
	for mi: Node in piece.find_children("*", "MeshInstance3D", true, false):
		var m := mi as MeshInstance3D
		var b := (global_transform.affine_inverse() * m.global_transform) * m.get_aabb()
		box = b if first else box.merge(b)
		first = false
	return box


## Stone slab under the model's footprint, slightly inset, from 0.6 m below ground.
func _add_plinth() -> void:
	var box := _piece_bounds()
	if box.size == Vector3.ZERO:
		return
	var mesh := BoxMesh.new()
	mesh.size = Vector3(maxf(box.size.x - 0.4, 0.5), 0.6 + plinth_height, maxf(box.size.z - 0.4, 0.5))
	var slab := MeshInstance3D.new()
	slab.name = "Plinth"
	slab.mesh = mesh
	slab.position = Vector3(box.get_center().x, plinth_height - mesh.size.y * 0.5, box.get_center().z)
	if plinth_material != null:
		slab.material_override = plinth_material
	add_child(slab)


func interaction_kind() -> String:
	return "building"


func interaction_index() -> int:
	return -1


func interaction_id() -> String:
	return id


func interaction_radius() -> float:
	return near_radius


func approach_target() -> Vector3:
	return _door.global_position if _door != null else global_position


func set_highlighted(on: bool) -> void:
	if _highlight != null:
		_highlight.visible = on
