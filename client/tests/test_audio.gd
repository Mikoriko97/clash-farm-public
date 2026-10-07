extends SceneTree
## Headless test: the Audio autoload creates the SFX and Ambient buses, maps play_fx to
## clips (unknown fx is ignored), switches the zone ambient with a crossfade on two
## players, clamps and applies volumes (0 mutes the bus) and reacts to bridge messages.
## Run: godot --headless --path client --script res://tests/test_audio.gd

var _failures := 0
var _frames := 0
var _audio: Node


func _check(ok: bool, what: String) -> void:
	if not ok:
		_failures += 1
		push_error("FAIL: " + what)


func _init() -> void:
	# `--script` runs carry no autoloads: add the Audio singleton by hand, as the game has it.
	_audio = load("res://scripts/audio.gd").new()
	_audio.name = "Audio"
	root.add_child(_audio)


func _process(delta: float) -> bool:
	_frames += 1
	if _frames == 2:
		_check_buses()
		_check_fx()
		_check_zone()
		_check_volume()
	if _frames == 6:
		_check_fade_running(delta)
		print("RESULT: %s (%d failures)" % ["PASS" if _failures == 0 else "FAIL", _failures])
		quit(1 if _failures > 0 else 0)
		return true
	return false


func _check_buses() -> void:
	var sfx := AudioServer.get_bus_index("SFX")
	var amb := AudioServer.get_bus_index("Ambient")
	_check(sfx != -1 and amb != -1, "buses SFX and Ambient exist")
	_check(sfx != amb, "two distinct buses")
	_check(AudioServer.get_bus_send(sfx) == &"Master", "SFX sends to Master")
	var players := 0
	for child in _audio.get_children():
		if child is AudioStreamPlayer:
			players += 1
	_check(players == 8, "6 sfx players + 2 ambient players, got %d" % players)


func _check_fx() -> void:
	_check(_audio.call("play_fx", "harvest") == true, "harvest has a clip")
	_check(_audio.call("play_fx", "water") == true, "water has its own clip")
	_check(_audio.CLIPS.has("door_open") and _audio.CLIPS.has("door_close") and _audio.CLIPS.has("door_bell"), "the door sounds are clips")
	_check(_audio.call("play_fx", "dance") == false, "unknown fx is ignored")
	var playing := 0
	for child in _audio.get_children():
		if child is AudioStreamPlayer and child.bus == "SFX" and child.playing:
			playing += 1
	_check(playing == 2, "two sfx players busy, got %d" % playing)


func _check_zone() -> void:
	_audio.call("_on_bridge_message", "set_zone", {"zone": "island"})
	_check(_audio.get("zone") == "island", "zone island set via bridge message")
	var live: AudioStreamPlayer = null
	for child in _audio.get_children():
		if child is AudioStreamPlayer and child.bus == "Ambient" and child.playing:
			live = child
	_check(live != null, "island ambient is playing")
	if live != null:
		var wav := live.stream as AudioStreamWAV
		_check(wav != null and wav.loop_mode == AudioStreamWAV.LOOP_FORWARD, "ambient loops forward")
		_check(wav != null and wav.loop_end > 22050 * 20, "loop_end covers the whole file: %d" % (wav.loop_end if wav != null else -1))
	_audio.call("set_zone", "nowhere")
	_check(_audio.get("zone") == "island", "unknown zone keeps the current ambient")
	_audio.call("set_zone", "cave")
	_check(_audio.get("zone") == "cave", "zone switched to cave")
	var playing := 0
	for child in _audio.get_children():
		if child is AudioStreamPlayer and child.bus == "Ambient" and child.playing:
			playing += 1
	_check(playing == 2, "both ambient players run during the crossfade, got %d" % playing)


func _check_volume() -> void:
	_audio.call("_on_bridge_message", "set_volume", {"sfx": 250, "ambient": -3})
	_check(_audio.get("sfx_volume") == 100 and _audio.get("ambient_volume") == 0, "volumes clamp to 0..100")
	_check(AudioServer.is_bus_mute(AudioServer.get_bus_index("Ambient")), "0 mutes the bus")
	_check(not AudioServer.is_bus_mute(AudioServer.get_bus_index("SFX")), "100 does not mute")
	_audio.call("set_volume", 50, 50)
	var db := AudioServer.get_bus_volume_db(AudioServer.get_bus_index("SFX"))
	_check(db < -11.0 and db > -13.0, "50 %% is about -12 dB, got %.1f" % db)


func _check_fade_running(delta: float) -> void:
	_check(delta >= 0.0, "process runs")
	_check(_audio.is_processing(), "crossfade still processing after a few frames")
