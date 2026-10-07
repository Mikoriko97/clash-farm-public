extends CanvasLayer
## Накладка статистики (автолоад `DebugStats`): FPS, час кадру (середній / p95 /
## найгірший за секунду), виклики малювання, трикутники, обʼєкти, відеопамʼять
## (разом / текстури / буфери), вузли й фізичні тіла, спайки кадрів з причиною.
## Вмикається клавішею F3 або з dev-меню HUD (`debug_world { stats: true }`),
## у debug-збірках видима одразу. Коли видима, раз на `log_interval` с друкує
## рядок `[hud] …` у консоль (у браузері його читає консоль DevTools), а кожен
## кадр довший за `spike_ms` — рядок `[spike] …` з останньою поміткою (`mark`).
## Системи, що роблять великі порції роботи (стрімінг чанків, побудова флори,
## прогрів шейдерів, плитки трави), кличуть `FrameMark.mark("…")`, щоб спайк
## можна було пояснити. Це дешево: один рядок у змінну.

@export var log_interval: float = 2.0
@export var spike_ms: float = 40.0
const WINDOW := 60
const SPIKES_KEPT := 8

@onready var _label: Label = %Stats
var _since_log := 0.0
var _frames: PackedFloat32Array = PackedFloat32Array()
var _spikes: Array[String] = []
var _spike_count := 0
var _mark := ""
var _mark_at := 0


func _ready() -> void:
	# Кадри й спайки рахуються завжди (щоб F3 після старту показав і завантаження зони);
	# у release видимість вимкнена до F3.
	set_process(true)
	set_enabled(OS.is_debug_build())


func set_enabled(on: bool) -> void:
	visible = on
	if on:
		_since_log = log_interval


func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventKey and event.pressed and not event.echo and (event as InputEventKey).keycode == KEY_F3:
		set_enabled(not visible)


## Помітка для пояснення спайків (кличе `FrameMark.mark`): що саме зараз робить
## гра, наприклад "chunk (3,2) lod 0 build".
func mark(text: String) -> void:
	_mark = text
	_mark_at = Time.get_ticks_msec()


func _process(delta: float) -> void:
	_record_frame(delta)
	if not visible:
		return
	_label.text = _stats_text()
	_since_log += delta
	if _since_log >= log_interval:
		_since_log = 0.0
		print("[hud] " + _label.text.replace("\n", " | "))


func _record_frame(delta: float) -> void:
	var ms := delta * 1000.0
	_frames.append(ms)
	if _frames.size() > WINDOW:
		_frames.remove_at(0)
	if ms >= spike_ms:
		_spike_count += 1
		var age := Time.get_ticks_msec() - _mark_at
		var why := "%s (%d ms ago)" % [_mark, age] if not _mark.is_empty() else "no mark"
		var line := "%.0f ms: %s" % [ms, why]
		_spikes.append(line)
		if _spikes.size() > SPIKES_KEPT:
			_spikes.remove_at(0)
		# У release (грі) консоль браузера не засмічуємо: спайки видно на F3, у консоль — лише коли
		# накладка відкрита або збірка debug.
		if visible or OS.is_debug_build():
			print("[spike] " + line)


func _frame_stats() -> Vector3:
	## (середній, p95, найгірший) за вікно, мс.
	if _frames.is_empty():
		return Vector3.ZERO
	var sorted := _frames.duplicate()
	sorted.sort()
	var sum := 0.0
	for v: float in sorted:
		sum += v
	var p95 := sorted[mini(int(floor(sorted.size() * 0.95)), sorted.size() - 1)]
	return Vector3(sum / sorted.size(), p95, sorted[sorted.size() - 1])


func _stats_text() -> String:
	var f := _frame_stats()
	var lines := PackedStringArray()
	lines.append("FPS %d  frame %.1f / p95 %.1f / max %.1f ms" % [Engine.get_frames_per_second(), f.x, f.y, f.z])
	lines.append("process %.1f ms  physics %.1f ms" % [
		Performance.get_monitor(Performance.TIME_PROCESS) * 1000.0,
		Performance.get_monitor(Performance.TIME_PHYSICS_PROCESS) * 1000.0])
	lines.append("draw calls %d  objects %d  primitives %dk" % [
		Performance.get_monitor(Performance.RENDER_TOTAL_DRAW_CALLS_IN_FRAME),
		Performance.get_monitor(Performance.RENDER_TOTAL_OBJECTS_IN_FRAME),
		int(Performance.get_monitor(Performance.RENDER_TOTAL_PRIMITIVES_IN_FRAME) / 1000.0)])
	lines.append("video %.0f MB (tex %.0f, buf %.0f)  static %.0f MB" % [
		Performance.get_monitor(Performance.RENDER_VIDEO_MEM_USED) / 1048576.0,
		Performance.get_monitor(Performance.RENDER_TEXTURE_MEM_USED) / 1048576.0,
		Performance.get_monitor(Performance.RENDER_BUFFER_MEM_USED) / 1048576.0,
		Performance.get_monitor(Performance.MEMORY_STATIC) / 1048576.0])
	lines.append("nodes %d  orphans %d  bodies %d" % [
		Performance.get_monitor(Performance.OBJECT_NODE_COUNT),
		Performance.get_monitor(Performance.OBJECT_ORPHAN_NODE_COUNT),
		Performance.get_monitor(Performance.PHYSICS_3D_ACTIVE_OBJECTS)])
	lines.append("spikes >%d ms: %d" % [int(spike_ms), _spike_count])
	for s: String in _spikes:
		lines.append("  " + s)
	if not _mark.is_empty():
		lines.append("last: " + _mark)
	return "\n".join(lines)
