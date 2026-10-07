class_name ResourceNode
extends Node3D
## A tree or rock placed in Island.tscn that the server tracks (migration 0007):
## `idx` matches `world_state.resources`, hits left and regrowth come from the
## server. A used-up node disappears (no stump, no collider, not interactable) until the
## server brings it back — after its respawn time somewhere free as a sapling; a hit shakes it.
## Since 0017 the server also owns the point (`x_dm`, `z_dm`: the node moves
## there) and the growth stage (`stage` 0/1/2: sapling, young, grown — scale;
## only grown ones can be gathered). Pure view: no economy, no rules.

@export var idx: int = 0
@export_enum("tree", "rock") var kind: String = "tree"
## How far from the centre the character stops (Interaction.walk_to).
@export var approach: float = 1.3

var hits_left: int = 0
var respawn_at_unix: float = 0.0
var _gone := false
var stage: int = 2
## Piece scale per growth stage (grown = 1).
const STAGE_SCALE := [0.45, 0.75, 1.0]

@onready var _piece: Node3D = get_node_or_null(^"Piece")
@onready var _highlight: MeshInstance3D = get_node_or_null(^"%Highlight")


func _ready() -> void:
	add_to_group(Interaction.GROUP)
	# Стартові вузли сцени стоять, поки сервер не скаже інакше (`world_state.resources`).
	_piece_scale()


func interaction_kind() -> String:
	return "resource"


func interaction_index() -> int:
	return idx


## `approach` is a floor; a scaled-up rock stops the character at its real edge.
func approach_distance() -> float:
	return maxf(approach, Interaction.body_radius(self) + Interaction.STOP_MARGIN)


func set_highlighted(on: bool) -> void:
	if _highlight != null:
		_highlight.visible = on


func is_depleted() -> bool:
	return hits_left <= 0


## `{ idx, kind, hits_left, respawn_at? }` from `world_state.resources`.
func apply(entry: Dictionary) -> void:
	var hits: Variant = entry.get("hits_left", 0)
	if typeof(hits) != TYPE_FLOAT and typeof(hits) != TYPE_INT:
		push_warning("ResourceNode %d: bad hits_left" % idx)
		return
	hits_left = maxi(int(hits), 0)
	respawn_at_unix = PlotNode._parse_iso(str(entry.get("respawn_at", "")))
	var st: Variant = entry.get("stage", 2)
	if typeof(st) == TYPE_FLOAT or typeof(st) == TYPE_INT:
		stage = clampi(int(st), 0, 2)
	var x: Variant = entry.get("x_dm")
	var z: Variant = entry.get("z_dm")
	if (typeof(x) == TYPE_FLOAT or typeof(x) == TYPE_INT) and (typeof(z) == TYPE_FLOAT or typeof(z) == TYPE_INT):
		position = Vector3(float(x) / 10.0, position.y, float(z) / 10.0)
	_refresh()


func _piece_scale() -> void:
	if _piece != null:
		var s: float = float(STAGE_SCALE[stage])
		_piece.scale = Vector3(s, s, s)


func is_grown() -> bool:
	return stage >= 2


func _refresh() -> void:
	_set_gone(is_depleted())
	if _piece == null or is_depleted():
		return
	var s: float = float(STAGE_SCALE[stage])
	_piece.scale = Vector3(s, s, s)


## A used-up node vanishes for real (owner request 2026-09-30: «має пропадати, а не лишатись
## маленьким»): hidden, colliders off (the character walks through, the nav grid and the wild
## flora treat the spot as free) and out of the interaction group, so nothing highlights it.
func _set_gone(gone: bool) -> void:
	if gone == _gone:
		return
	_gone = gone
	visible = not gone
	for shape: Node in find_children("*", "CollisionShape3D", true, false):
		(shape as CollisionShape3D).set_deferred("disabled", gone)
	if gone:
		remove_from_group(Interaction.GROUP)
	elif not is_in_group(Interaction.GROUP):
		add_to_group(Interaction.GROUP)
	if is_inside_tree():
		get_tree().call_group(NavGrid.GROUP, "mark_dirty")


## One hit: a short shake of the piece (`play_fx` gather); a lightning strike
## whips it much harder and it settles back over a second.
func play_fx(fx: String) -> void:
	if _piece == null or not is_inside_tree():
		return
	var tween := create_tween()
	if fx == "lightning":
		tween.tween_property(_piece, "rotation:z", 0.35, 0.05)
		tween.tween_property(_piece, "rotation:z", -0.25, 0.12)
		tween.tween_property(_piece, "rotation:z", 0.12, 0.18)
		tween.tween_property(_piece, "rotation:z", 0.0, 0.6)
		return
	tween.tween_property(_piece, "rotation:z", 0.09, 0.06)
	tween.tween_property(_piece, "rotation:z", -0.06, 0.08)
	tween.tween_property(_piece, "rotation:z", 0.0, 0.1)
