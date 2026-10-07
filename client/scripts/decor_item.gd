class_name DecorItem
extends Node3D
## One decor piece the server placed (migration 0009): DecorGrid instances it
## from `world_state.decor` at the server's point and yaw and feeds it the item
## id; this node only shows the catalog item's farm_pack piece. Interactable of
## kind "decor" so the character can stand next to it and React offers "remove".

## item_id -> piece scene; edit in DecorItem.tscn when the catalog grows.
@export var pieces: Dictionary = {}

## Server placement id (uuid) and the grid's running index for `player_near`.
var placement_id: String = ""
var idx: int = -1
var item_id: String = ""

@onready var _marker: MeshInstance3D = %Marker
@onready var _holder: Node3D = %Holder
@onready var _highlight: MeshInstance3D = get_node_or_null(^"%Highlight")


func _ready() -> void:
	add_to_group(Interaction.GROUP)
	_refresh()


func interaction_kind() -> String:
	return "decor"


func interaction_index() -> int:
	return idx


func interaction_id() -> String:
	return placement_id


## Stop just outside the piece's own collider (a lantern is thin, a windmill is not).
func approach_distance() -> float:
	return maxf(Interaction.body_radius(self) + Interaction.STOP_MARGIN, 0.6)


## Reach grows with the piece so `player_near` fires from the approach spot.
func interaction_radius() -> float:
	return approach_distance() + 1.2


## Meshes of the placed piece only (never the marker or the highlight ring).
func piece_meshes() -> Array[MeshInstance3D]:
	var out: Array[MeshInstance3D] = []
	if _holder == null:
		return out
	for node: Node in _holder.find_children("*", "MeshInstance3D", true, false):
		out.append(node as MeshInstance3D)
	return out


func set_highlighted(on: bool) -> void:
	if _highlight != null:
		_highlight.visible = on


## Piece scene for a catalog item, or null when the pack has none for it.
func piece_for(id: String) -> PackedScene:
	return pieces.get(id) as PackedScene


func apply(new_item_id: String) -> void:
	if new_item_id == item_id:
		return
	item_id = new_item_id
	_refresh()


func _refresh() -> void:
	if _marker == null or _holder == null:
		return  # not ready yet; _ready() calls _refresh() again
	for child: Node in _holder.get_children():
		child.queue_free()
	var scene := piece_for(item_id)
	if not item_id.is_empty() and scene == null:
		push_warning("DecorItem: no piece for %s" % item_id)
	if scene != null:
		_holder.add_child(scene.instantiate())
	_marker.visible = scene == null
	if _highlight != null:
		var ring := maxf(Interaction.body_radius(self) + 0.35, 0.8) / 1.25
		_highlight.scale = Vector3(ring, 1.0, ring)
