#!/usr/bin/env bash
# Усі Godot-тести клієнта (docs/WORKFLOW.md): кожен — окремий headless-процес. Окрім `RESULT: PASS`, звіряє «неочікувані помилки движка»:
# будь-який `ERROR:`/`SCRIPT ERROR:` рядок, крім відомого шуму на виході (витоки ресурсів під час quit), валить тест, бо те, що
# движок друкує в консоль, гравець бачить у DevTools (скріншот власника 2026-10-01: потік «Cannot convert argument 3»).
# `WARNING:` не валить: тести свідомо шлють хибні повідомлення, і гра відхиляє їх через `push_warning`.
# Steam-лаунчер Godot інколи мовчить — до 3 спроб. Запуск з кореня репо:  bash client/tools/run_tests.sh [шаблон-імені]
#   GODOT=<шлях до godot> перевизначає рушій. Вихід 0 лише коли всі тести зелені й без помилок.
set -u
GODOT="${GODOT:-S:/SteamLibrary/steamapps/common/Godot Engine/godot.windows.opt.tools.64.exe}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# Тести, що потребують вікна/GPU і в headless падають самі по собі (docs/STATE.md, «Відомі проблеми»).
SKIP="test_multimesh_buffer"
NOISE='resources still in use at exit|RID allocations of type|Pages in use exist at exit|at: ~Dependency|^ +at: '
FILTER="${1:-}"
failed=0
ran=0
for f in "$ROOT"/client/tests/test_*.gd; do
	name="$(basename "$f" .gd)"
	[[ -n "$FILTER" && "$name" != *$FILTER* ]] && continue
	[[ " $SKIP " == *" $name "* ]] && { echo "SKIP  $name"; continue; }
	log="$(mktemp)"
	for _ in 1 2 3; do
		# Тест, що завис (помилка в _process кожен кадр і quit() не настає), не має блокувати весь прогін. `-k`: рідний процес
		# Windows може не почути SIGTERM від timeout з Git Bash (2026-10-03 тест провисів 2 год) — тоді через 10 с жорстко.
		timeout -k 10 300 "$GODOT" --headless --path "$ROOT/client" --script "res://tests/$name.gd" >"$log" 2>&1
		grep -q "RESULT:" "$log" && break
	done
	ran=$((ran + 1))
	result="$(grep -m1 "RESULT:" "$log" || echo "no RESULT line")"
	bad="$(grep -E "^(SCRIPT )?ERROR:|^USER ERROR" "$log" | grep -Ev "$NOISE" | grep -v "^ERROR: FAIL:" || true)"
	if [[ "$result" != *"PASS (0 failures)"* || -n "$bad" ]]; then
		failed=$((failed + 1))
		echo "FAIL  $name — $result"
		[[ -n "$bad" ]] && echo "$bad" | sort | uniq -c | sort -rn | head -5 | sed 's/^/        /'
	else
		echo "ok    $name"
	fi
	rm -f "$log"
done
echo "== $ran tests, $failed with failures or unexpected engine errors"
exit $((failed > 0))
