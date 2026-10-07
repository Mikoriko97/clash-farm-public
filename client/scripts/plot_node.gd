class_name PlotNode
extends Node3D
## One farm plot. Pure view: the state comes from `world_state` via PlotGrid,
## a click only reports `plot_selected(idx)` through Bridge. Crops are farm_pack
## pieces chosen by `crop_id` (editable `crops` map in PlotNode.tscn) and grow
## through stages from the visual timer `planted_at → ready_at` (server clock).

signal selected(idx: int)

const STATE_EMPTY := "empty"
const STATE_GROWING := "growing"
const STATE_READY := "ready"
## Visual growth stages: progress threshold → scale of the crop pieces.
const STAGES: Array = [[0.0, 0.3], [0.35, 0.6], [0.7, 0.85]]
const READY_SCALE := 1.0

@export var idx: int = 0
## Cell on the island grid (server `col`, `row`); PlotGrid positions the node.
var col: int = 0
var row: int = 0
## Every plot of this plot's bed, itself included (PlotGrid._relink).
var bed_mates: Array = []
var _link_flags: Array = [false, false, false, false]
var _highlighted: bool = false
## crop_id (e.g. "seed_wheat") → PackedScene; unknown crops use `default_crop`.
@export var crops: Dictionary = {}
## crop_id → Array[PackedScene] of growth stages (sprout … ripe, any count ≥ 2): the stage
## for the progress is instanced at scale 1 instead of scaling one piece (wheat, 2026-09-16).
@export var crop_stages: Dictionary = {}
@export var default_crop: PackedScene
## Materials are scene properties (not preload) so the exporter ships them and the owner can swap them.
@export var soil_empty: Material
@export var soil_growing: Material
## A plot fills its 3 m cell (PlotGrid.CELL_M), so neighbours in a bed touch
## with no gap and every plot has the same size; the outline sits just outside.
const SOIL_HALF := 1.5
const EDGE_OUT := 0.1

var state: String = STATE_EMPTY
var crop_id: String = ""
var planted_at_unix: float = 0.0
var ready_at_unix: float = 0.0
var server_offset: float = 0.0
var _shown_crop: String = ""
## Growth stage the crop shows now (`_visual_stage`): `_process` redraws only when it changes, not every frame.
var _shown_stage: float = -1.0

## The soil is a farm_pack field piece (scenes/island/PlotNode.tscn `Soil`); its
## mesh takes the empty / growing material.
@onready var _soil: MeshInstance3D = _find_mesh(%Soil)
@onready var _crop: Node3D = %Crop
@onready var _body: StaticBody3D = %Body
## Golden outline strips north / east / south / west; only the bed's outer edges show.
@onready var _edges: Array = [
	get_node_or_null(^"%EdgeN"), get_node_or_null(^"%EdgeE"),
	get_node_or_null(^"%EdgeS"), get_node_or_null(^"%EdgeW"),
]
## Corner squares NE / SE / SW / NW closing the outline where two outer edges meet.
@onready var _corners: Array = [
	get_node_or_null(^"%CornerNE"), get_node_or_null(^"%CornerSE"),
	get_node_or_null(^"%CornerSW"), get_node_or_null(^"%CornerNW"),
]
@onready var _fx: Dictionary = {
	"plant": get_node_or_null(^"%FxPlant"),
	"harvest": get_node_or_null(^"%FxHarvest"),
	"water": get_node_or_null(^"%FxWater"),
	"fertilize": get_node_or_null(^"%FxWater"),
}


func _ready() -> void:
	add_to_group(Interaction.GROUP)
	_refresh()


func interaction_kind() -> String:
	return "plot"


func interaction_index() -> int:
	return idx


## Soil box is 2.4 m wide: stop just outside its edge.
func approach_distance() -> float:
	return 2.1


## One outline around the whole bed while the character is within reach
## (Interaction): every plot of the bed lights only the edges facing away from a neighbour.
func set_highlighted(on: bool) -> void:
	show_edges(on)
	for mate: Variant in bed_mates:
		if mate != self and is_instance_valid(mate):
			(mate as PlotNode).show_edges(on)


func show_edges(on: bool) -> void:
	_highlighted = on
	_apply_links()


## Bed neighbours north / east / south / west: the soil stretches into the gap
## towards each one so the bed reads as a single field, and that side has no outline.
func set_links(north: bool, east: bool, south: bool, west: bool) -> void:
	_link_flags = [north, east, south, west]
	_apply_links()


func _apply_links() -> void:
	if _edges.size() < 4:
		return  # not ready yet; _ready() applies the stored flags
	var d := SOIL_HALF + EDGE_OUT
	var offsets := [Vector3(0.0, 0.03, -d), Vector3(d, 0.03, 0.0), Vector3(0.0, 0.03, d), Vector3(-d, 0.03, 0.0)]
	var shown := [false, false, false, false]
	for i in 4:
		var edge := _edges[i] as MeshInstance3D
		shown[i] = _highlighted and not bool(_link_flags[i])
		if edge != null:
			edge.position = offsets[i]
			edge.visible = shown[i]
	# Corner i sits between edge i (N,E,S,W) and edge (i+1) % 4: NE, SE, SW, NW.
	var corner_pos := [Vector3(d, 0.03, -d), Vector3(d, 0.03, d), Vector3(-d, 0.03, d), Vector3(-d, 0.03, -d)]
	for i in 4:
		var corner := _corners[i] as MeshInstance3D
		if corner != null:
			corner.position = corner_pos[i]
			corner.visible = bool(shown[i]) and bool(shown[(i + 1) % 4])


## Applies a plot entry from `world_state`; unknown fields are ignored.
func apply(entry: Dictionary, now_unix: float, local_now: float) -> void:
	var next_state: String = str(entry.get("state", STATE_EMPTY))
	if next_state not in [STATE_EMPTY, STATE_GROWING, STATE_READY]:
		push_warning("PlotNode %d: unknown state %s" % [idx, next_state])
		return
	state = next_state
	crop_id = str(entry.get("crop_id", ""))
	planted_at_unix = _parse_iso(str(entry.get("planted_at", "")))
	ready_at_unix = _parse_iso(str(entry.get("ready_at", "")))
	server_offset = now_unix - local_now
	_refresh()


## Seconds left by the server clock (0 when ready/unknown).
func remaining_seconds() -> float:
	if state != STATE_GROWING or ready_at_unix <= 0.0:
		return 0.0
	return maxf(0.0, ready_at_unix - _server_now())


## 0..1 growth progress by the server clock; 1 when ready or timing unknown.
func progress() -> float:
	if state == STATE_READY:
		return 1.0
	if state != STATE_GROWING or ready_at_unix <= planted_at_unix:
		return 1.0
	return clampf((_server_now() - planted_at_unix) / (ready_at_unix - planted_at_unix), 0.0, 1.0)


## Index into `crop_stages[crop_id]` for the progress: last stage when ready, the rest
## spread evenly over the growing time.
func stage_index(count: int) -> int:
	if count <= 1:
		return 0
	if state == STATE_READY:
		return count - 1
	return clampi(int(floor(progress() * float(count - 1))), 0, count - 2)


## Piece scale for the current progress (STAGES thresholds, READY_SCALE when ready).
func stage_scale() -> float:
	if state == STATE_READY:
		return READY_SCALE
	var p := progress()
	var scale: float = STAGES[0][1]
	for stage: Array in STAGES:
		if p >= float(stage[0]):
			scale = float(stage[1])
	return scale


static func _find_mesh(node: Node) -> MeshInstance3D:
	if node is MeshInstance3D:
		return node as MeshInstance3D
	var found: Array[Node] = node.find_children("*", "MeshInstance3D", true, false)
	return found[0] as MeshInstance3D if not found.is_empty() else null


func _server_now() -> float:
	return Time.get_unix_time_from_system() + server_offset


func _refresh() -> void:
	if _soil == null or _crop == null:
		return  # not ready yet; _ready() calls _refresh() again with the stored state
	_soil.material_override = soil_empty if state == STATE_EMPTY else soil_growing
	_apply_links()
	_crop.visible = state != STATE_EMPTY
	if state != STATE_EMPTY:
		_ensure_crop_pieces()
	var s := 1.0 if _staged() else stage_scale()
	for anchor: Node in _crop.get_children():
		(anchor as Node3D).scale = Vector3(s, s, s)
	_shown_stage = _visual_stage()


## What growth changes on screen: the stage piece (staged crops) or the scale of the one piece.
func _visual_stage() -> float:
	return float(stage_index((crop_stages[crop_id] as Array).size())) if _staged() else stage_scale()


func _staged() -> bool:
	return crop_stages.has(crop_id) and (crop_stages[crop_id] as Array).size() >= 2


## Every anchor under Crop holds one instance of the crop's piece; swapped when the crop changes.
func _ensure_crop_pieces() -> void:
	var scene: PackedScene = null
	var key := crop_id
	if _staged():
		var stages: Array = crop_stages[crop_id]
		var i := stage_index(stages.size())
		scene = stages[i] as PackedScene
		key = "%s:%d" % [crop_id, i]
	if _shown_crop == key:
		return
	if scene == null:
		scene = crops.get(crop_id) as PackedScene
	if scene == null:
		scene = default_crop
	for anchor: Node in _crop.get_children():
		for old: Node in anchor.get_children():
			# Detach now: a queued node would still sit under the anchor this frame.
			anchor.remove_child(old)
			old.queue_free()
		if scene != null:
			anchor.add_child(scene.instantiate())
	_shown_crop = key


func _process(_delta: float) -> void:
	if state != STATE_GROWING or ready_at_unix <= 0.0:
		return
	if remaining_seconds() <= 0.0:
		# Visual only: the server decides readiness; this just stops the sprout look.
		state = STATE_READY
	elif _visual_stage() == _shown_stage:
		return
	_refresh()


## One-shot particle burst for a bridge `play_fx` aimed at this plot.
func play_fx(kind: String) -> void:
	var particles := _fx.get(kind) as CPUParticles3D
	if particles != null:
		particles.restart()
		particles.emitting = true


## Called by PlotGrid after its raycast hit this plot.
func report_selected() -> void:
	_on_click()


func _on_click() -> void:
	var bridge: Node = get_node_or_null(^"/root/Bridge")
	if bridge != null and bridge.mode == "ui":
		return
	selected.emit(idx)
	if bridge != null:
		bridge.send("plot_selected", {"idx": idx})


static func _parse_iso(text: String) -> float:
	if text.is_empty():
		return 0.0
	var unix := Time.get_unix_time_from_datetime_string(text)
	return float(unix) if unix > 0 else 0.0
