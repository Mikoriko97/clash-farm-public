class_name WildBush
extends Node3D
## Ціль взаємодії для одного твердого куща дикої флори острова (`IslandWild`): кущі
## малюються MultiMesh-ом і самі вузлами не є, тож цей легкий вузол дає їм `player_near`
## (kind «bush», id «x_dm,z_dm»), щоб React запропонував «Clear bush». Прибирання
## безкоштовне й без інструмента (рішення власника 2026-09-30): сервер лише запамʼятовує
## місце, а `IslandWild` тримає флору подалі від нього. Пріоритет нижчий за все інше —
## біля грядки чи дерева пропонується їхня дія, а не кущ.

## Радіус колайдера куща (м): від нього залежить, де зупиняється персонаж.
var radius: float = 0.9
var bush_id: String = ""


func _ready() -> void:
	add_to_group(Interaction.GROUP)


func interaction_kind() -> String:
	return "bush"


func interaction_index() -> int:
	return -1


func interaction_id() -> String:
	return bush_id


## Найнижчий пріоритет: кущ обирається, лише коли поруч нічого іншого.
func interaction_priority() -> int:
	return 1


## Куш «чіпають» трохи здалеку, ніж центр: колайдер товстий.
func interaction_radius() -> float:
	return radius + 1.3


func approach_distance() -> float:
	return radius + Interaction.STOP_MARGIN + 0.15


func set_highlighted(_on: bool) -> void:
	pass


## Ідентифікатор за позицією в дециметрах: так місце знає й сервер (він округлить до 0,5 м).
static func id_for(x_m: float, z_m: float) -> String:
	return "%d,%d" % [roundi(x_m * 10.0), roundi(z_m * 10.0)]


## Точка світу з id «x_dm,z_dm» на висоті `like.y`; сама `like`, якщо id зіпсований.
static func point_of(id: String, like: Vector3) -> Vector3:
	var parts := id.split(",")
	if parts.size() != 2 or not parts[0].is_valid_int() or not parts[1].is_valid_int():
		return like
	return Vector3(float(parts[0].to_int()) / 10.0, like.y, float(parts[1].to_int()) / 10.0)
