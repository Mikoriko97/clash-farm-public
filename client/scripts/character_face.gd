class_name CharacterFace
extends RefCounted
## Обличчя бази персонажа (ADR-032): наліпка `Base45_Face` на голові з атласом 4 × 2 виразів
## (`assets/character/base45/base45_face_atlas.png`, генератор `client/blender/characters/base45_face.py`). Вираз — зсув UV
## у спільному атласі, тож кожен вираз — один крихітний матеріал на всіх персонажів (без blend shapes: GLES3).
## Вигляд гравця обирає базовий вираз (`look.face`: `face_neutral` | `face_smile` | `face_joy` | `face_cool` | `face_surprised`);
## невідоме чи відсутнє — спокійне. Решта клітинок атласу (кліпання, сум, сонність) — для реакцій і емоцій.

const NODE := "Base45_Face"
const ATLAS: Texture2D = preload("res://assets/character/base45/base45_face_atlas.png")
const COLS := 4
const ROWS := 2
## Клітинки атласу (рядок за рядком): 0 neutral, 1 smile, 2 joy, 3 surprised, 4 blink, 5 sad, 6 determined, 7 sleepy.
const CELLS: Dictionary = {"face_neutral": 0, "face_smile": 1, "face_joy": 2, "face_cool": 6, "face_surprised": 3}
const DEFAULT := "face_neutral"
## Риси обличчя здалеку не видно (і тінь від наліпки не потрібна).
const RANGE := 24.0

static var _materials: Dictionary = {}


## Клітинка атласу за виглядом.
static func cell_of(look: Variant) -> int:
	var id := DEFAULT
	if typeof(look) == TYPE_DICTIONARY and typeof((look as Dictionary).get("face")) == TYPE_STRING:
		id = String((look as Dictionary)["face"])
	return int(CELLS.get(id, CELLS[DEFAULT]))


## Ставить вираз на наліпку обличчя під `root`; безпечно викликати знову (і без наліпки — нічого не робить).
static func apply(root: Node, look: Variant) -> void:
	var face := root.find_child(NODE, true, false) as MeshInstance3D
	if face == null:
		return
	face.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	face.visibility_range_end = RANGE
	# базовий вираз — для `CharacterLife`, що на час дії ставить свій і повертає цей
	face.set_meta("base_cell", cell_of(look))
	face.set_surface_override_material(0, material(cell_of(look)))


## Спільний матеріал клітинки `cell` атласу (alpha scissor: краї рис чіткі, без сортування прозорості).
static func material(cell: int) -> StandardMaterial3D:
	var cached: StandardMaterial3D = _materials.get(cell)
	if cached != null:
		return cached
	var mat := StandardMaterial3D.new()
	mat.albedo_texture = ATLAS
	mat.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA_SCISSOR
	mat.alpha_scissor_threshold = 0.5
	mat.roughness = 0.85
	mat.uv1_offset = Vector3(float(cell % COLS) / COLS, float(cell / COLS) / ROWS, 0.0)
	mat.resource_name = "face_%d" % cell
	_materials[cell] = mat
	return mat
