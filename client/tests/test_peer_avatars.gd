extends SceneTree
## Headless test: PeerAvatars spawns, updates, interpolates and drops avatars
## from `peers_state`; malformed entries are ignored; PosePublisher reports
## integers in decimetres/degrees.
## Run: godot --headless --path client --script res://tests/test_peer_avatars.gd

var _failures := 0
var _frames := 0
var _peers: PeerAvatars
var _stage := 0


func _check(ok: bool, what: String) -> void:
	if not ok:
		_failures += 1
		push_error("FAIL: " + what)


func _init() -> void:
	var world := Node3D.new()
	root.add_child(world)
	_peers = PeerAvatars.new()
	_peers.avatar_scene = load("res://scenes/player/PeerAvatar.tscn")
	_peers.grace_ms = 0
	# Here only message-driven drops are checked; with grace 0 the timed purge (test_client_fixes) would drop everyone between frames.
	_peers.purge_secs = 3600.0
	world.add_child(_peers)


func _process(_delta: float) -> bool:
	_frames += 1
	if _frames == 2:
		_peers.apply([
			{"id": "aa", "nick": "Оля", "x_dm": 100, "z_dm": -50, "yaw_deg": 90, "anim": "walk_forward"},
			{"id": "bb", "nick": "Ігор", "x_dm": 0, "z_dm": 0, "yaw_deg": 0, "anim": "idle"},
			{"id": "", "x_dm": 1, "z_dm": 1, "yaw_deg": 0},
			{"id": "cc", "x_dm": "far", "z_dm": 0, "yaw_deg": 0},
			"junk",
		])
		_check(_peers.count() == 2, "two valid peers spawned, two malformed ignored: %d" % _peers.count())
		var a := _peers.avatar("aa")
		_check(a != null and a.global_position.is_equal_approx(Vector3(10.0, 0.0, -5.0)), "first pose snaps into place")
		_check(a != null and (a.get_node("%Nick") as Label3D).text == "Оля", "nick shown")
		_check(a != null and (a.get_node("%AnimationPlayer") as AnimationPlayer).current_animation == "walk_forward", "clip from the server plays")
		_peers.apply("nope")
		_check(_peers.count() == 2, "non-array payload ignored")
		# Move aa 2 m east: the avatar glides there over the next frames.
		_peers.apply([{"id": "aa", "nick": "Оля", "x_dm": 120, "z_dm": -50, "yaw_deg": 180, "anim": "run_forward"}])
		_stage = 1
	elif _stage == 1 and _frames == 4:
		var a := _peers.avatar("aa")
		_check(a != null and a.global_position.x > 10.05 and a.global_position.x < 12.0, "interpolating towards the new pose: x=%.2f" % a.global_position.x)
		_check(_peers.count() == 1 and _peers.avatar("bb") == null, "peer missing from the list is dropped (grace 0)")
		_stage = 2
	elif _stage == 2 and _frames == 120:
		var a := _peers.avatar("aa")
		_check(a != null and absf(a.global_position.x - 12.0) < 0.2, "arrived at the new pose: x=%.2f" % a.global_position.x)
		_test_publisher()
		_test_ground()
		print("RESULT: %s (%d failures)" % ["PASS" if _failures == 0 else "FAIL", _failures])
		quit(1 if _failures > 0 else 0)
		return true
	return false


func _test_publisher() -> void:
	var player: Player = (load("res://scenes/player/Player.tscn") as PackedScene).instantiate()
	root.add_child(player)
	player.global_position = Vector3(1.26, 0.0, -3.44)
	player.face_point(Vector3(1.26, 0.0, -2.0))
	var publisher := PosePublisher.new()
	publisher.player = player
	root.add_child(publisher)
	var pose := publisher.current_pose()
	_check(pose["x_dm"] == 13 and pose["z_dm"] == -34, "pose in whole decimetres: %s" % str(pose))
	_check(pose["yaw_deg"] == 0 and pose["anim"] == "idle", "yaw facing +z is 0, idle clip: %s" % str(pose))


## Avatars stand on the zone's terrain (`height_at`), not at y = 0 — on the mainland peers ran under the map.
func _test_ground() -> void:
	var stub := GDScript.new()
	stub.source_code = "extends Node
func height_at(x: float, z: float) -> float:
	return 5.0 + x * 0.1
"
	stub.reload()
	var ground := Node.new()
	ground.set_script(stub)
	root.add_child(ground)
	var world := Node3D.new()
	root.add_child(world)
	var peers := PeerAvatars.new()
	peers.avatar_scene = load("res://scenes/player/PeerAvatar.tscn")
	peers.set("_ground", ground)
	world.add_child(peers)
	peers.apply([{"id": "gg", "nick": "G", "x_dm": 100, "z_dm": 20, "yaw_deg": 0, "anim": "idle"}])
	var a := peers.avatar("gg")
	_check(a != null and absf(a.global_position.y - 6.0) < 0.001, "avatar snaps onto the terrain height: y=%.2f" % (a.global_position.y if a != null else -1.0))
