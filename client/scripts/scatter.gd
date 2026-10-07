class_name Scatter
extends RefCounted
## Детермінована розстановка «синім шумом» (docs/MAINLAND.md §9). Кандидати
## лежать на сітці з тремтінням: клітинка (i, j) виду `layer` завжди дає ту саму
## точку, пріоритет і кидок густини (хеш від сіду, виду, i, j), тому сусідні
## чанки, повторне запікання й острів після розширення сходяться без швів.
## Приймання — за пріоритетом, якщо точка не залазить у радіус уже прийнятих
## (варіант Bridson «dart throwing» з радіусом на вид: дерева не ростуть одне
## в одному, кущ не стоїть у стовбурі) і якщо кидок < густини в точці
## (`density.call(pos) -> float`, 0…1). Гаї, галявини, біоми, схили,
## «під кроною» — усе в густині; тут лише відстані.

## Комірка просторового хешу (м); радіус виду має бути ≤ CELL.
const CELL := 8.0
const STRIDE := 4

var _buckets: Dictionary = {}
var _count := 0


func clear() -> void:
	_buckets.clear()
	_count = 0


func count() -> int:
	return _count


## Займає `p` слідом радіуса `r`; `anchor` позначає крону (правило «під деревом»).
func add(p: Vector2, r: float, anchor: bool) -> void:
	var key := Vector2i(floori(p.x / CELL), floori(p.y / CELL))
	var b: PackedFloat32Array = _buckets.get(key, PackedFloat32Array())
	b.append(p.x)
	b.append(p.y)
	b.append(r)
	b.append(1.0 if anchor else 0.0)
	_buckets[key] = b
	_count += 1


## Чи слід `r` у точці `p` перекриє щось уже прийняте.
func blocked(p: Vector2, r: float) -> bool:
	var n := ceili((r + CELL) / CELL)
	var cx := floori(p.x / CELL)
	var cz := floori(p.y / CELL)
	for dz: int in range(-n, n + 1):
		for dx: int in range(-n, n + 1):
			var b: PackedFloat32Array = _buckets.get(Vector2i(cx + dx, cz + dz), PackedFloat32Array())
			var i := 0
			while i < b.size():
				var d := r + b[i + 2]
				var ex := p.x - b[i]
				var ez := p.y - b[i + 1]
				if ex * ex + ez * ez < d * d:
					return true
				i += STRIDE
	return false


## Чи є крона (anchor) не далі `dist` від `p`.
func near_anchor(p: Vector2, dist: float) -> bool:
	var n := ceili(dist / CELL)
	var cx := floori(p.x / CELL)
	var cz := floori(p.y / CELL)
	var d2 := dist * dist
	for dz: int in range(-n, n + 1):
		for dx: int in range(-n, n + 1):
			var b: PackedFloat32Array = _buckets.get(Vector2i(cx + dx, cz + dz), PackedFloat32Array())
			var i := 0
			while i < b.size():
				if b[i + 3] > 0.5:
					var ex := p.x - b[i]
					var ez := p.y - b[i + 1]
					if ex * ex + ez * ez <= d2:
						return true
				i += STRIDE
	return false


## Розкидає один вид по `rect` (м); повертає [x, z, yaw, scale, …] прийнятих.
## `quota.call(pos) -> bool` (необовʼязково) каже, чи тут ще потрібна точка
## (ліміти вузлів ресурсів на чанк) — відмова не займає місця.
func sample(kind: FloraKind, layer: int, seed: int, rect: Rect2, density: Callable, quota: Callable = Callable()) -> PackedFloat32Array:
	var step := maxf(kind.spacing_m, 0.5)
	var cand := PackedFloat32Array()
	var keys := PackedInt64Array()
	var base := mix(seed * 1000003 + layer)
	for j: int in range(floori(rect.position.y / step), ceili(rect.end.y / step)):
		for i: int in range(floori(rect.position.x / step), ceili(rect.end.x / step)):
			var h := mix(mix(base ^ (i * 73856093)) ^ (j * 19349663))
			var pos := Vector2((float(i) + unit(h)) * step, 0.0)
			h = mix(h)
			pos.y = (float(j) + unit(h)) * step
			if not rect.has_point(pos):
				continue
			var d: float = density.call(pos)
			if d <= 0.0:
				continue
			h = mix(h)
			if unit(h) >= d:
				continue
			h = mix(h)
			keys.append((int(unit(h) * 2147483647.0) << 32) | (cand.size() / 4))
			h = mix(h)
			var yaw := unit(h) * TAU
			h = mix(h)
			cand.append_array(PackedFloat32Array([pos.x, pos.y, yaw, lerpf(kind.scale_min, kind.scale_max, unit(h))]))
	keys.sort()
	var out := PackedFloat32Array()
	for k: int in keys:
		var idx := int(k & 0xFFFFFFFF) * 4
		var p := Vector2(cand[idx], cand[idx + 1])
		var r := kind.radius_m * cand[idx + 3]
		if blocked(p, r):
			continue
		if quota.is_valid() and not bool(quota.call(p)):
			continue
		add(p, r, kind.anchor)
		out.append_array(cand.slice(idx, idx + 4))
	return out


## splitmix64 на 64-бітних int GDScript (переповнення загортається; зсуви без знака).
static func mix(x: int) -> int:
	var z := x + -7046029254386353131
	z = (z ^ ((z >> 30) & 0x3FFFFFFFF)) * -4658895280553007687
	z = (z ^ ((z >> 27) & 0x1FFFFFFFFF)) * -7723592293110705685
	return z ^ ((z >> 31) & 0x1FFFFFFFF)


## Рівномірне число [0, 1) з хешу.
static func unit(h: int) -> float:
	return float((h >> 11) & 0x1FFFFFFFFFFFFF) / 9007199254740992.0
