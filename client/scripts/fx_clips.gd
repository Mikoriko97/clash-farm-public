class_name FxClips
extends RefCounted
## Bridge `play_fx` → character clip (assets/character/character_anims.tres) and the
## tool in the right hand (Player.tscn RightHand). `play_fx.tool` overrides the tool
## ("" = bare hands, the mainland says so when the player owns none).
const TABLE: Dictionary = {
	"plant": {"clip": &"kneel", "tool": ""},
	"harvest": {"clip": &"reap", "tool": "scythe"},
	"water": {"clip": &"water", "tool": "can"},
	"fertilize": {"clip": &"water", "tool": "can"},
	"place": {"clip": &"hammer", "tool": "hammer"},
	"gather": {"clip": &"pickup", "tool": ""},
	"chop": {"clip": &"chop", "tool": "axe"},
	"mine": {"clip": &"mine", "tool": "pickaxe"},
	"dig": {"clip": &"dig", "tool": "spade"},
	"interact": {"clip": &"interact", "tool": ""},
	"dance": {"clip": &"dance", "tool": ""},
	# Реакції й емоції (ADR-035): `cheer` — новий рівень, кінець навчання; `fail` — дія не вдалась; `emote_*` — колесо емоцій у HUD.
	# `after`: реакція чекає, поки персонаж доробить поточну дію (рівень приходить разом зі збором, і жест не обриває косу).
	"cheer": {"clip": &"cheer", "tool": "", "after": true},
	"fail": {"clip": &"shake_no", "tool": "", "after": true},
	"emote_wave": {"clip": &"wave", "tool": ""},
	"emote_cheer": {"clip": &"cheer", "tool": ""},
	"emote_laugh": {"clip": &"laugh", "tool": ""},
	"emote_dance": {"clip": &"dance", "tool": ""},
	"emote_heart": {"clip": &"heart", "tool": ""},
	"emote_shrug": {"clip": &"shrug", "tool": ""},
	"emote_thumbs_up": {"clip": &"thumbs_up", "tool": ""},
	"emote_clap": {"clip": &"clap", "tool": ""},
}
