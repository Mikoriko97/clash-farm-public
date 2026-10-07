extends SceneTree
## Headless test: MultiMeshBuffer fills a MultiMesh exactly like set_instance_transform /
## set_instance_color would (same transforms and colours read back), grows past its
## capacity, and an empty buffer leaves zero instances.
## Run (with a window: the headless dummy renderer keeps no MultiMesh data):
##   godot --path client --script res://tests/test_multimesh_buffer.gd

const MMBuffer := preload("res://scripts/multimesh_buffer.gd")

var _failures := 0


func _check(ok: bool, what: String) -> void:
	if not ok:
		_failures += 1
		push_error("FAIL: " + what)


func _init() -> void:
	_check_roundtrip()
	_check_growth_and_empty()
	print("RESULT: %s (%d failures)" % ["PASS" if _failures == 0 else "FAIL", _failures])
	quit(1 if _failures > 0 else 0)


func _check_roundtrip() -> void:
	var mesh := BoxMesh.new()
	var buf := MMBuffer.new(3, true)
	var xforms: Array[Transform3D] = [
		Transform3D(Basis(Vector3.UP, 0.7).scaled(Vector3(1.5, 1.5, 1.5)), Vector3(1.0, 2.0, 3.0)),
		Transform3D(Basis(Quaternion(Vector3.UP, Vector3(0.3, 1.0, 0.1).normalized())), Vector3(-4.0, 0.5, 9.0)),
		Transform3D.IDENTITY,
	]
	var colors: Array[Color] = [Color(0.2, 0.4, 0.6, 1.0), Color(0.9, 0.1, 0.3, 0.5), Color.WHITE]
	for i: int in 3:
		buf.add(xforms[i], colors[i])
	var mm := buf.build(mesh)
	_check(mm.instance_count == 3 and mm.use_colors and mm.mesh == mesh, "built multimesh has 3 coloured instances")
	# Reference: the slow path.
	var ref := MultiMesh.new()
	ref.transform_format = MultiMesh.TRANSFORM_3D
	ref.use_colors = true
	ref.mesh = mesh
	ref.instance_count = 3
	for i: int in 3:
		ref.set_instance_transform(i, xforms[i])
		ref.set_instance_color(i, colors[i])
	var same := ref.buffer.size() == mm.buffer.size()
	for k: int in ref.buffer.size():
		# Colours live as half floats in the engine, hence the tolerance.
		same = same and absf(ref.buffer[k] - mm.buffer[k]) < 0.002
	_check(same, "buffer layout matches the engine's own")
	for i: int in 3:
		var got := mm.get_instance_transform(i)
		_check(got.is_equal_approx(xforms[i]), "transform %d reads back" % i)
		var c := mm.get_instance_color(i)
		_check(absf(c.r - colors[i].r) < 0.002 and absf(c.g - colors[i].g) < 0.002 and absf(c.b - colors[i].b) < 0.002 and absf(c.a - colors[i].a) < 0.002, "colour %d reads back" % i)


func _check_growth_and_empty() -> void:
	var buf := MMBuffer.new(1)
	for i: int in 10:
		buf.add(Transform3D(Basis.IDENTITY, Vector3(float(i), 0.0, 0.0)))
	var mm := buf.build(BoxMesh.new())
	_check(mm.instance_count == 10 and not mm.use_colors, "buffer grows past its capacity: %d" % mm.instance_count)
	_check(mm.get_instance_transform(9).origin.x == 9.0, "last instance kept its place")
	var empty := MMBuffer.new(0).build(BoxMesh.new())
	_check(empty.instance_count == 0, "empty buffer gives zero instances")
