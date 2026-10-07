class_name StickRun
extends RefCounted
## Біг від джойстика на екрані: стік, відтягнутий до зовнішнього краю кола (довжина вектора ≥ `RUN_AT`), біжить, як Shift на
## клавіатурі; назад до кроку — нижче `WALK_BELOW` (гістерезис, щоб палець на краю не смикав крок/біг). Те саме кільце малює
## web (`Joystick.tsx`, `RUN_AT`): змінюєш поріг тут — зміни й там.

const RUN_AT := 0.85
const WALK_BELOW := 0.75


## Чи біжить гравець при векторі стіка `stick` (довжина 0..1), якщо перед цим він `was_running`.
static func runs(stick: Vector2, was_running: bool) -> bool:
	return stick.length() >= (WALK_BELOW if was_running else RUN_AT)
