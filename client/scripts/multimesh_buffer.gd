extends RefCounted
## Підключається як `const MMBuffer := preload("res://scripts/multimesh_buffer.gd")`,
## а не через class_name: так скрипт і експорт не залежать від кешу класів редактора.
## Заливка MultiMesh одним буфером замість циклу `set_instance_transform` /
## `set_instance_color`: кожен такий виклик у GDScript іде через RenderingServer
## окремо (у wasm — десятки мікросекунд), тож плитка трави на 7 000 квадів або
## вид флори на чанку коштували десятки мілісекунд у кадрі. Тут трансформи й
## кольори пишуться у PackedFloat32Array (12 float на екземпляр: три рядки
## 3×4 трансформа; +4 на колір), а `apply` віддає масив рушію одним викликом.

var _data := PackedFloat32Array()
var _stride: int
var _colors: bool
var _count := 0


func _init(capacity: int, colors: bool = false) -> void:
	_colors = colors
	_stride = 16 if colors else 12
	_data.resize(maxi(capacity, 0) * _stride)


func count() -> int:
	return _count


func add(xform: Transform3D, color: Color = Color.WHITE) -> void:
	var i := _count * _stride
	if i + _stride > _data.size():
		_data.resize(i + _stride)
	var b := xform.basis
	var o := xform.origin
	# Engine layout: three rows of the 3x4 matrix; Basis.x/y/z are its columns.
	_data[i] = b.x.x
	_data[i + 1] = b.y.x
	_data[i + 2] = b.z.x
	_data[i + 3] = o.x
	_data[i + 4] = b.x.y
	_data[i + 5] = b.y.y
	_data[i + 6] = b.z.y
	_data[i + 7] = o.y
	_data[i + 8] = b.x.z
	_data[i + 9] = b.y.z
	_data[i + 10] = b.z.z
	_data[i + 11] = o.z
	if _colors:
		_data[i + 12] = color.r
		_data[i + 13] = color.g
		_data[i + 14] = color.b
		_data[i + 15] = color.a
	_count += 1


## Новий MultiMesh з `mesh` і всіма доданими екземплярами.
func build(mesh: Mesh) -> MultiMesh:
	var mm := MultiMesh.new()
	mm.transform_format = MultiMesh.TRANSFORM_3D
	mm.use_colors = _colors
	mm.mesh = mesh
	apply(mm)
	return mm


## Заливає екземпляри в готовий MultiMesh (формат 3D, use_colors як у буфера).
func apply(mm: MultiMesh) -> void:
	mm.use_colors = _colors
	mm.instance_count = _count
	if _count == 0:
		return
	if _data.size() != _count * _stride:
		_data.resize(_count * _stride)
	mm.buffer = _data
