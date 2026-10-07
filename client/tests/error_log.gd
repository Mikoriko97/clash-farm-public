extends Logger
## Збирає все, що движок і скрипти друкують як помилку (`push_error`, SCRIPT ERROR, шейдери), для тестів: те, що Godot пише
## в консоль, гравець бачить у DevTools. Попередження (`push_warning`) не рахуються: гра свідомо ними відхиляє хибні повідомлення.
## Використання: `var log := ErrorLog.new(); OS.add_logger(log) … OS.remove_logger(log); log.errors`.

var errors: PackedStringArray = []


func _log_error(function: String, file: String, line: int, code: String, rationale: String, _editor_notify: bool, error_type: int, backtraces: Array[ScriptBacktrace]) -> void:
	if error_type == Logger.ERROR_TYPE_WARNING:
		return
	var where := " <- ".join(backtraces.map(func(b: ScriptBacktrace) -> String: return b.format(0, 0).replace("
", " < ")))
	errors.append("f%d %s:%d (%s) %s %s [%s]" % [Engine.get_process_frames(), file.get_file(), line, function, code, rationale, where])
