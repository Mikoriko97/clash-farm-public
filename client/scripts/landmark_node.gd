class_name LandmarkNode
extends Node3D
## A landmark marker on the mainland (docs/MAINLAND.md §4): a signpost with the
## landmark's name where the server says it stands. Walking up to it offers
## «Оглянути»; the server decides whether the discovery counts. Pure view.

var id: String = ""
var landmark_name: String = ""
var discovered: bool = false

@onready var _label: Label3D = get_node_or_null(^"%Name")
@onready var _highlight: MeshInstance3D = get_node_or_null(^"%Highlight")


func _ready() -> void:
	add_to_group(Interaction.GROUP)
	_refresh()


func interaction_kind() -> String:
	return "landmark"


func interaction_index() -> int:
	return -1


func interaction_id() -> String:
	return id


func approach_distance() -> float:
	return maxf(1.6, Interaction.body_radius(self) + Interaction.STOP_MARGIN)


func interaction_radius() -> float:
	return 4.0


func set_highlighted(on: bool) -> void:
	if _highlight != null:
		_highlight.visible = on


## `{ id, name, x_dm, z_dm, discovered }` from `mainland_landmarks`; position is set by the owner.
func apply(entry: Dictionary) -> void:
	id = str(entry.get("id", id))
	landmark_name = str(entry.get("name", landmark_name))
	discovered = bool(entry.get("discovered", false))
	_refresh()


func _refresh() -> void:
	if _label == null:
		return
	_label.text = ("★ " if discovered else "") + landmark_name
	_label.modulate = Color(1.0, 0.9, 0.55) if discovered else Color(0.95, 0.97, 1.0)
