class_name ResourceGrid
extends Node3D
## Feeds `world_state.resources` to the ResourceNode children (trees and rocks
## placed in the editor under this node) and plays `play_fx` on them.
## Since 0017 the server owns positions and may add nodes (bought strips, regrowth):
## an unknown idx is instanced from `tree_scene` / `rock_scene`, a full snapshot
## drops dynamic nodes it no longer lists; the eight scene-placed starters stay.

@export var tree_scene: PackedScene
@export var rock_scene: PackedScene
## Prop scenes carry no script of their own (Island.tscn attaches it per instance): a
## spawned copy gets the same one before it enters the tree.
const RESOURCE_SCRIPT := preload("res://scripts/resource_node.gd")

var _nodes: Dictionary = {}
var _dynamic: Dictionary = {}


func _ready() -> void:
	_index()
	var bridge: Node = get_node_or_null(^"/root/Bridge")
	if bridge != null:
		bridge.message_received.connect(_on_bridge_message)


func node(idx: int) -> ResourceNode:
	if _nodes.is_empty():
		_index()
	return _nodes.get(idx) as ResourceNode


func _index() -> void:
	for child: Node in get_children():
		if child is ResourceNode:
			_nodes[(child as ResourceNode).idx] = child


func _on_bridge_message(type: String, payload: Dictionary) -> void:
	match type:
		"world_state":
			apply_world_state(payload)
		"play_fx":
			_play_fx(payload)


## `{ resources: [{idx, kind, hits_left, respawn_at?}] }`; garbage entries are skipped.
func apply_world_state(payload: Dictionary) -> void:
	var entries: Variant = payload.get("resources", [])
	if typeof(entries) != TYPE_ARRAY:
		return
	var seen: Dictionary = {}
	for entry: Variant in entries:
		if typeof(entry) != TYPE_DICTIONARY:
			continue
		var idx := _idx_of((entry as Dictionary).get("idx"))
		var target := node(idx)
		if target == null:
			target = _spawn(idx, str((entry as Dictionary).get("kind", "")))
		if target == null:
			push_warning("ResourceGrid: no node for idx %s" % str((entry as Dictionary).get("idx")))
			continue
		seen[idx] = true
		target.apply(entry as Dictionary)
	if bool(payload.get("full", false)):
		for idx: int in _dynamic.keys():
			if not seen.has(idx):
				(_dynamic[idx] as Node).queue_free()
				_dynamic.erase(idx)
				_nodes.erase(idx)


## A node the scene does not carry (0017): instanced by kind at the origin; `apply`
## moves it to the server's point.
func _spawn(idx: int, kind: String) -> ResourceNode:
	if idx < 0:
		return null
	var scene := tree_scene if kind == "tree" else rock_scene if kind == "rock" else null
	if scene == null:
		return null
	var raw := scene.instantiate() as Node3D
	if raw == null:
		return null
	if not (raw is ResourceNode):
		raw.set_script(RESOURCE_SCRIPT)
	var inst := raw as ResourceNode
	if inst == null:
		raw.free()
		return null
	inst.name = "%s%d" % ["Tree" if kind == "tree" else "Rock", idx]
	inst.idx = idx
	inst.kind = kind
	add_child(inst)
	_nodes[idx] = inst
	_dynamic[idx] = inst
	return inst


## A number, else -1 (never a node): malformed payload is ignored (rules/godot.md).
static func _idx_of(v: Variant) -> int:
	if typeof(v) == TYPE_FLOAT or typeof(v) == TYPE_INT:
		return int(v)
	return -1


func _play_fx(payload: Dictionary) -> void:
	var target: Variant = payload.get("target", {})
	if typeof(target) != TYPE_DICTIONARY or not (target as Dictionary).has("resource_idx"):
		return
	var found := node(_idx_of((target as Dictionary).get("resource_idx")))
	if found == null:
		return
	var fx := str(payload.get("fx", ""))
	if fx == "lightning":
		var weather: Weather = get_tree().get_first_node_in_group(Weather.GROUP) as Weather
		if weather != null:
			weather.strike_at(found.global_position)
	found.play_fx(fx)
