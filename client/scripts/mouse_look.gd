extends Node
## Огляд мишею на ПК (запит власника 2026-09-29): клік по землі захоплює курсор, далі рух
## миші обертає камеру без кнопок, WASD — крок, Esc або будь-яка панель React (`set_mode ui`)
## відпускає курсор. На сенсорних екранах вимкнено — там тап по землі веде персонажа.
## Автолоад `MouseLook`; сам рух камери робить `IslandInput._on_motion` через `captured()`.

## Чи хоче гравець захоплення (память між панелями: після закриття панелі курсор лишається
## вільним, бо браузер дозволяє захоплення лише з жесту — наступний клік по землі поверне).
var wanted := false


func _ready() -> void:
	set_process_unhandled_input(true)
	var bridge: Node = get_node_or_null(^"/root/Bridge")
	if bridge != null:
		bridge.mode_changed.connect(_on_mode_changed)


## Headless (тести, інструменти) не захоплює. Сенсорний пристрій визначає сама подія кліку:
## емульована з тапу миша має `device == DEVICE_ID_EMULATION` (перевіряє IslandInput),
## бо `is_touchscreen_available()` на ноутбуках із сенсором каже «так» і для миші.
func enabled() -> bool:
	return DisplayServer.get_name() != "headless"


func captured() -> bool:
	return Input.mouse_mode == Input.MOUSE_MODE_CAPTURED


## Викликати з обробника кліку: браузер приймає pointer lock лише з жесту користувача.
## `false`, коли захоплення тут не має сенсу (headless) — тоді клік веде персонажа.
func capture() -> bool:
	if not enabled():
		return false
	wanted = true
	Input.mouse_mode = Input.MOUSE_MODE_CAPTURED
	return true


func release() -> void:
	wanted = false
	if Input.mouse_mode != Input.MOUSE_MODE_VISIBLE:
		Input.mouse_mode = Input.MOUSE_MODE_VISIBLE


func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventKey and (event as InputEventKey).pressed and (event as InputEventKey).keycode == KEY_ESCAPE:
		release()


## Панель React забирає екран — курсор потрібен; після повернення в `walk` браузер усе одно
## вимагає нового кліку, тож просто відпускаємо.
func _on_mode_changed(mode: String) -> void:
	if mode == "ui" and captured():
		Input.mouse_mode = Input.MOUSE_MODE_VISIBLE
