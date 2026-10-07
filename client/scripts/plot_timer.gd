extends Label3D
## Підпис над грядкою: скільки лишилось до достигання (за серверним `ready_at` батьківського
## PlotNode) або «Ready!». Порожня грядка — без підпису. Оновлюється двічі на секунду,
## текст англійською (ADR-024). Запит власника 2026-09-29: «не зрозуміло, скільки чекати».

const REFRESH_SEC := 0.5
const READY_TEXT := "Ready!"

var _plot: PlotNode
var _left := 0.0


func _ready() -> void:
	_plot = get_parent() as PlotNode
	visible = false
	set_process(_plot != null)


func _process(delta: float) -> void:
	_left -= delta
	if _left > 0.0:
		return
	_left = REFRESH_SEC
	var next := text_for(_plot.state, _plot.remaining_seconds())
	visible = not next.is_empty()
	if next != text:
		text = next


## "" для порожньої, "Ready!" для стиглої, інакше час, що лишився: «<1 min», «12 min», «2 h 05».
static func text_for(state: String, seconds: float) -> String:
	if state == PlotNode.STATE_READY:
		return READY_TEXT
	if state != PlotNode.STATE_GROWING:
		return ""
	if seconds < 60.0:
		return "<1 min"
	var m := int(ceil(seconds / 60.0))
	if m < 60:
		return "%d min" % m
	return "%d h %02d" % [m / 60, m % 60]
