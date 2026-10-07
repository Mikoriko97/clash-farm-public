class_name CharacterLife
extends Node
## «Життя» персонажа (ADR-035, хвиля C з docs/PRELAUNCH_REVIEW.md §1.3). Дитина сцени персонажа (`Player`, `PeerAvatar`):
## * обличчя: кліпає кожні 2,5–5,5 с, а на час кліпу дії чи емоції бере свій вираз (`CLIP_FACES`, клітинки атласу `CharacterFace`);
##   після кліпу — базовий вираз з вигляду гравця;
## * свій персонаж (`own`): постоявши без руху 8–15 с, робить випадкову дію спокою (озирнутися, потягнутися, почухати потилицю);
##   голова повертається до цілі поруч (`Interaction.near_changed`), через `LookAtModifier3D` після анімації;
## * стрибок: стиснення-розтягнення моделі на відриві й приземленні.
## Чужі персонажі отримують кліпи від сервера присутності (той самий `anim`), тож їхні дії, реакції й емоції видно й так.
## Чистий вигляд: жодної логіки гри, жодних запитів.

## Свій персонаж: варіанти спокою, погляд на ціль.
@export var own := false
@export var idle_min := 8.0
@export var idle_max := 15.0
@export var blink_min := 2.5
@export var blink_max := 5.5

const IDLE_VARIANTS: Array[String] = ["idle_look", "idle_stretch", "idle_scratch"]
const BLINK_SECONDS := 0.13
## Клітинки атласу обличчя (`base45_face.EXPRESSIONS`).
const NEUTRAL := 0
const SMILE := 1
const JOY := 2
const SURPRISED := 3
const BLINK := 4
const SAD := 5
const DETERMINED := 6
const SLEEPY := 7
## Кліп → вираз на його час; заплющені очі (JOY, BLINK, SLEEPY) не кліпають.
const CLIP_FACES: Dictionary = {
	"cheer": JOY, "laugh": JOY, "clap": JOY, "dance": JOY, "idle_stretch": JOY,
	"wave": SMILE, "heart": SMILE, "thumbs_up": SMILE, "reap": SMILE, "water": SMILE, "pickup": SMILE, "plant": SMILE, "kneel": SMILE,
	"shake_no": SAD, "hit": SAD, "shrug": SURPRISED, "idle_look": NEUTRAL,
	"chop": DETERMINED, "mine": DETERMINED, "dig": DETERMINED, "hammer": DETERMINED, "push": DETERMINED,
	"sleeping": BLINK, "lie_down": SLEEPY, "sitting_idle": SMILE,
}
const EYES_CLOSED: Array[int] = [JOY, BLINK, SLEEPY]
## Голова до цілі ближче за цю відстань (м).
const LOOK_RANGE := 2.5
## Стрибок: (x/z, y) масштаб на відриві й на приземленні.
const STRETCH := Vector2(0.94, 1.08)
const SQUASH := Vector2(1.08, 0.88)
## Реакція чекає кінця поточної дії не довше (с); застаріла вже ні до чого.
const REACT_WAIT := 4.0

var _anim: AnimationPlayer
var _model: Node3D
var _face: MeshInstance3D
var _look: LookAtModifier3D
var _look_target: Node3D
var _override := -1
var _blink_left := 0.0
var _blink_wait := 3.0
var _idle_for := 0.0
var _idle_after := 10.0
var _last_clip: StringName = &""
var _model_scale := Vector3.ONE
var _reaction: StringName = &""
var _reaction_wait := 0.0


func _ready() -> void:
	var host := get_parent()
	_anim = host.get_node_or_null(^"%AnimationPlayer") as AnimationPlayer
	_model = host.get_node_or_null(^"%Model") as Node3D
	if _model != null:
		_model_scale = _model.scale
		_face = _model.find_child(CharacterFace.NODE, true, false) as MeshInstance3D
	if _anim != null:
		_anim.current_animation_changed.connect(_on_clip)
	_blink_wait = randf_range(blink_min, blink_max)
	_idle_after = randf_range(idle_min, idle_max)
	if own:
		_setup_look()


func _process(delta: float) -> void:
	_blink(delta)
	if own:
		_react_pending(delta)
		_idle(delta)


## Вираз на час кліпу (або −1 — базовий з вигляду).
func _on_clip(clip: StringName) -> void:
	_override = int(CLIP_FACES.get(String(clip), -1))
	_paint()
	if clip == &"jump":
		_bounce(STRETCH)
	elif _last_clip == &"jump" and clip != &"jump":
		_bounce(SQUASH)
	_last_clip = clip
	_idle_for = 0.0


func _blink(delta: float) -> void:
	if _face == null:
		return
	if _blink_left > 0.0:
		_blink_left -= delta
		if _blink_left <= 0.0:
			_paint()
		return
	_blink_wait -= delta
	if _blink_wait > 0.0:
		return
	_blink_wait = randf_range(blink_min, blink_max)
	if current_cell() in EYES_CLOSED:
		return
	_blink_left = BLINK_SECONDS
	_face.set_surface_override_material(0, CharacterFace.material(BLINK))


## Клітинка, яку обличчя показує зараз без кліпання: вираз кліпу або базовий з вигляду (`CharacterFace.apply` пише його в meta).
func current_cell() -> int:
	if _override >= 0:
		return _override
	return int(_face.get_meta("base_cell", 0)) if _face != null else 0


func _paint() -> void:
	if _face != null and _blink_left <= 0.0:
		_face.set_surface_override_material(0, CharacterFace.material(current_cell()))


func _idle(delta: float) -> void:
	if _anim == null or not get_parent().has_method("play_action"):
		return
	# Лише зі звичайного спокою: у печері `idle_torch` тримає ліхтар, а варіант спокою його б опустив.
	if String(_anim.current_animation) != "idle":
		_idle_for = 0.0
		return
	_idle_for += delta
	if _idle_for < _idle_after:
		return
	_idle_for = 0.0
	_idle_after = randf_range(idle_min, idle_max)
	var variant := StringName(IDLE_VARIANTS[randi() % IDLE_VARIANTS.size()])
	if _anim.has_animation(variant):
		get_parent().call("play_action", variant, "")


## Реакція (`FxClips` з `after`): одразу, якщо персонаж вільний, інакше — щойно доробить поточну дію.
func react(clip: StringName) -> void:
	_reaction = clip
	_reaction_wait = REACT_WAIT
	_react_pending(0.0)


func _react_pending(delta: float) -> void:
	if _reaction == &"":
		return
	_reaction_wait -= delta
	var host := get_parent()
	if _reaction_wait < 0.0 or not host.has_method("play_action"):
		_reaction = &""
	elif not host.call("is_acting"):
		host.call("play_action", _reaction, "")
		_reaction = &""


func _bounce(shape: Vector2) -> void:
	if _model == null:
		return
	var tween := create_tween()
	var squashed := Vector3(_model_scale.x * shape.x, _model_scale.y * shape.y, _model_scale.z * shape.x)
	tween.tween_property(_model, "scale", squashed, 0.07)
	tween.tween_property(_model, "scale", _model_scale, 0.16).set_trans(Tween.TRANS_BACK).set_ease(Tween.EASE_OUT)


## Голова повертається до цілі поруч: модифікатор кістки голови після анімації, ціль — вузол, що ставимо на найближчий об'єкт.
func _setup_look() -> void:
	var skeleton := _model.find_child("Skeleton3D", true, false) as Skeleton3D if _model != null else null
	if skeleton == null:
		return
	_look_target = Node3D.new()
	_look_target.name = "LookTarget"
	_look_target.top_level = true
	add_child(_look_target)
	_look = LookAtModifier3D.new()
	_look.name = "HeadLook"
	_look.bone_name = "Head"
	_look.forward_axis = SkeletonModifier3D.BONE_AXIS_PLUS_Z
	_look.use_angle_limitation = true
	_look.primary_limit_angle = deg_to_rad(110.0)
	_look.secondary_limit_angle = deg_to_rad(50.0)
	_look.duration = 0.35
	_look.influence = 0.0
	skeleton.add_child(_look)
	_look.target_node = _look.get_path_to(_look_target)
	var interaction := get_parent().get_parent().get_node_or_null(^"Interaction") if get_parent().get_parent() != null else null
	if interaction != null and interaction.has_signal("near_changed"):
		interaction.near_changed.connect(_on_near.bind(interaction))


func _on_near(kind: String, idx: int, distance: float, interaction: Node) -> void:
	var node: Node3D = interaction.call("find_target", kind, idx) if kind != "" and idx >= 0 else null
	if node == null or distance > LOOK_RANGE:
		_look_at(null)
		return
	_look_at(node.global_position + Vector3.UP * 0.4)


## Повертає голову до точки (null — прямо). Плавність дає `duration` модифікатора й твін впливу.
func _look_at(point: Variant) -> void:
	if _look == null:
		return
	if point != null:
		_look_target.global_position = point
	create_tween().tween_property(_look, "influence", 0.8 if point != null else 0.0, 0.3)
