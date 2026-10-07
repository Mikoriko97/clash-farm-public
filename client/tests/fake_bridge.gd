extends Node
## Test double of the `Bridge` autoload: records what Godot sends (`sent`, pairs [type, payload]) and carries `mode`, `look` and
## the bridge signals. A test renames the real autoload out of the way and adds this one to `root` under the name "Bridge".

signal message_received(type: String, payload: Dictionary)
signal mode_changed(mode: String)

var mode: String = "walk"
var look: Dictionary = {}
var zone: String = "island"
var sent: Array = []


func send(type: String, payload: Dictionary) -> void:
	sent.append([type, payload])


## Payloads sent with `type`, in order.
func sent_of(type: String) -> Array:
	var out: Array = []
	for entry: Array in sent:
		if entry[0] == type:
			out.append(entry[1])
	return out


func set_mode(new_mode: String) -> void:
	mode = new_mode
	mode_changed.emit(mode)
