class_name FrameMark
## Помітка для накладки статистики (`scenes/debug/DebugHud.tscn`, автолоад
## `DebugStats`): системи, що роблять великі порції роботи, кличуть
## `FrameMark.mark("chunk (3,2) build")`, і спайк кадру в накладці показує, що
## саме тоді відбувалось. Окремий клас, а не звернення до автолоаду напряму, щоб
## скрипти компілювались і в headless-інструментах без автолоадів.


static func mark(text: String) -> void:
	var loop := Engine.get_main_loop() as SceneTree
	if loop == null:
		return
	var stats := loop.root.get_node_or_null(^"DebugStats")
	if stats != null:
		stats.call("mark", text)
