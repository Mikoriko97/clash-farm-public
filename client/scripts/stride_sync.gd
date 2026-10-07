class_name StrideSync
extends Node
## Темп ходьби й бігу під справжню швидкість персонажа (аудит анімацій 2026-10-03). Кліпи `walk_forward`/`run_forward` зроблено
## через IK під швидкості гри (`client/blender/characters/build_locomotion.py`), тож на повному ходу темп 1, а стопа на землі стоїть;
## з частково нахиленим джойстиком, у скриптовому кроці (твін) чи в чужого персонажа (позиції з мережі) швидкість інша — і кліп
## прискорюється чи сповільнюється так, щоб стопа не ковзала. Дитина `Player` і `PeerAvatar`; лише вигляд.

## Швидкість (м/с сцени), під яку зроблено кліп; має збігатися з `walk_speed`/`run_speed` `player.gd` і `CLIPS` генератора.
const CLIP_SPEED: Dictionary = {&"walk_forward": 2.2, &"run_forward": 4.8}
const MIN_SCALE := 0.45
const MAX_SCALE := 1.4
## Згладжування виміряної швидкості (1/с): позиції з мережі приходять сходинками.
const SMOOTH := 8.0

var _anim: AnimationPlayer
var _host: Node3D
var _last := Vector3.ZERO
var _speed := 0.0
var _driving := false


func _ready() -> void:
	_host = get_parent() as Node3D
	_anim = _host.get_node_or_null(^"%AnimationPlayer") as AnimationPlayer if _host != null else null
	if _host != null:
		_last = _host.global_position


func _process(delta: float) -> void:
	if _anim == null or delta <= 0.0:
		return
	var speed := measure(delta)
	var nominal: float = CLIP_SPEED.get(_anim.current_animation, 0.0)
	if nominal <= 0.0:
		if _driving:
			_anim.speed_scale = 1.0
			_driving = false
		return
	_anim.speed_scale = clampf(speed / nominal, MIN_SCALE, MAX_SCALE)
	_driving = true


## Горизонтальна швидкість господаря: `velocity` тіла, а коли вона нуль (твін, чужий персонаж) — рух вузла між кадрами.
func measure(delta: float) -> float:
	var pos := _host.global_position
	var moved := Vector2(pos.x - _last.x, pos.z - _last.z).length() / delta
	_last = pos
	var body := _host as CharacterBody3D
	var raw := Vector2(body.velocity.x, body.velocity.z).length() if body != null and body.velocity != Vector3.ZERO else moved
	_speed = lerpf(_speed, raw, minf(1.0, delta * SMOOTH))
	return _speed
