# Звуки гри — джерела й збірка

Усі звуки — зі [Freesound](https://freesound.org) під ліцензією **Creative Commons 0** (можна копіювати, змінювати, поширювати й використовувати
комерційно без згадки автора; жодного `BY` чи `NC` тут немає). Подяки авторам не обовʼязкові, але перелік нижче — для відстеження джерел і
заміни. Дата добору: 2026-09-30.

**Як зібрано.** Сирі файли — HQ-прев'ю Freesound (`https://cdn.freesound.org/previews/<id//1000>/<id>_<uid>-hq.ogg`, OGG Vorbis 44,1–96 кГц): оригінали
потребують входу в акаунт, а прев'ю доступне без нього. `client/tools/build_audio.py` (запуск у Blender із MCP-сервером: `python client/tools/blender_rpc.py run
client/tools/build_audio.py`, `SRC` — тека з прев'ю) декодує їх аудіодвигуном Blender, обрізає тишу, змішує шари, стискає динаміку текстурних звуків, вирівнює гучність
(RMS −19…−24 дБFS для дій, піки −5…−9 дБFS для інтерфейсу, м'який лімітер −1,5 дБFS) і пише WAV: дії 44,1 кГц моно, ембієнт 32 кГц моно, петлі — рівнопотужним
перехресним згасанням кінця в початок. Godot стискає WAV у QOA (`.import`), ембієнти з `edit/loop_mode=1`.

**Як замінити звук.** Знайти інший CC0-семпл, завантажити прев'ю, додати його id у `RECIPES` (`build_audio.py`), перезібрати, `godot --headless --path client --import`, експортувати паки.
Якщо є акаунт Freesound — краще підставити оригінал (WAV/FLAC) замість прев'ю: скрипт читає `.ogg`, для оригіналу змінити `load()`.

## Дії у світі (`client/assets/audio/*.wav`, автолоад `Audio`)

| Клітинка | Джерела (Freesound id · назва · автор) |
|---|---|
| `plant` | 370367 · Step On Dirt · LilMati; 542752 · Seeds.wav · PanPiper5 (із затримкою 0,14 с) |
| `dig` | 651293 · Digging in wet course sand (2) · f3bbbo |
| `harvest` | 699491 · Plant_Harvest_02 · Valenspire; 388020 · Celery Snap 2.wav · saturdaysoundguy |
| `fertilize` | 235278 · seeds (cristal jar)4.wav · Godowan |
| `water` | 240465 · pour_water.wav · Godowan |
| `place` | 454222 · thud pint glass beer down on table wood knock… · kyles |
| `chop` | 452554 · axe chop into wood little debris.wav · kyles |
| `mine` | 233633 · Pickaxe #1.wav · abstraktgeneriert |
| `gather` | 538152 · Game Rustle Leaves · Fupicat |
| `lightning` | 743447 · Thunderclap one shot · breadmanelex |
| `sleep` | 83235 · Fluffing Blanket.wav · mlestn1; 151239 · Yawn 2 · OwlStorm |
| `sit` | 523698 · Sit down.wav · Bobblyboo |
| `interact` | 202314 · Click (1).mp3 · 7778 |

## Ембієнт (петлі)

| Зона | Джерело | Фрагмент |
|---|---|---|
| `ambient_island` | 578524 · Calm ocean waves · SamsterBirdies | 20–72 с, перехід 4 с → 48 с |
| `ambient_mainland` | 231537 · Forest_Birds (loop) 02.wav · VKProduktion | 0–30 с, перехід 2 с → 28 с |
| `ambient_cave` | 516566 · Dark Dungeon Ambience · Kinoton | 30–83 с, перехід 5 с → 48 с |

## Кроки (`client/assets/audio/step_<поверхня>_<n>.wav`, `Footsteps`)

| Поверхня | Джерела (id · назва · автор) |
|---|---|
| `grass` (1–4) | 421135 · Footstep_Grass_5.wav · GiocoSound; 390760 · FX_001_footstep_grass_l.wav · d00121058; 347371 · Soft Grass Foot Step · DRFX; 384867 · Right Grass/Grassy Footstep 2 (перші 0,35 с) · Ali_6868 |
| `dirt` (1–4) | 487866, 487867, 487868 · Footsteps_Dirt_Single_01…03.wav · BenDrain; 223153 · dirt/gravel footstep 4 · Yoyodaman234 |
| `stone` (1–4) | 197781 · Footstep on Stone 4 · sgossner; 390764 · FX_005_footstep_stone_l.wav · d00121058; 517137 · Footsteps Stone (6).wav · Wdomino; 422762 · Footsteps_Stone_4.wav · KieranKeegan |
| `wood` (1–3) | 434759 · Wood step Sample 4 · Notarget; 421153 · Footstep_Wood_Toe_1.wav · GiocoSound; 406750 · StepOnWood-014.wav · kretopi |
| `sand` (1–4) | 211457 · sand_step.ogg · pgi; 464699 · sandwalking_step1.wav · BranndyBottle; 516252 · STEP SAND 02.mp3 · wyronroberth; 778568 · Steps_Fine_Snow_Or_Sand_Strong_29 · BlondPanda |
| `snow` (1–4) | 369777, 369778, 369779 · Snow Step 1…3.wav · morganpurkis; 276919 · Snowstep4.wav · kodimynatt (кінець зрізаний згасанням 0,06 с) |

## Дощ (`ambient_rain_light.wav`, `ambient_rain_heavy.wav`, `WeatherAudio`)

| Петля | Джерело | Фрагмент |
|---|---|---|
| легкий дощ | 200273 · Rain light 2 (rural) · jmbphilmes | 12–56 с, перехід 4 с → 40 с |
| сильний дощ | 200272 · Rain heavy 2 (rural) · jmbphilmes | 8–52 с, перехід 4 с → 40 с |

## Вітер (`ambient_wind_breeze.wav`, `ambient_wind_gale.wav`, `WeatherAudio`)

| Петля | Джерело | Фрагмент |
|---|---|---|
| бриз | 181801 · breeze.wav · keweldog | 6–54 с, перехід 4 с → 44 с |
| шквал | 459981 · Strong Wind · florianreichelt | 4–44 с, перехід 3,4 с → 36 с |

## Ніч (`ambient_night.wav`, `night_owl.wav`, `WeatherAudio`)

| Звук | Джерело | Фрагмент |
|---|---|---|
| цвіркуни (петля) | 522298 · Crickets At Night - Clean sound · Defelozedd94 | 20–69 с, перехід 4 с → 45 с |
| сова (одиночний гук) | 398734 · owl.wav · Anthousai | ціле, ≈ 4 с |

## Ранок (`ambient_morning.wav`, `WeatherAudio`)

| Звук | Джерело | Фрагмент |
|---|---|---|
| ранковий хор (петля) | 611453 · Dawn chorus.wav · Synge101 | 40–79 с, перехід 3 с → 36 с (джерело 96 с, рівномірний спів без гучних одиночних птахів) |

## Листя на вітрі (`ambient_leaves.wav`, `WeatherAudio`)

| Звук | Джерело | Фрагмент |
|---|---|---|
| шелест листя (петля) | 381717 · Noise of wind in tree foliage. · urupin | 54–98 с, перехід 4 с → 40 с (джерело 182 с, яскравий шелест без низького гулу; порівняно 81792 · Silencyo Mistral pine trees — там переважає гул мікрофона, 452317 · kyles — тихий) |

## Двері (`client/assets/audio/door_*.wav`, `HouseNode`, `Interaction`)

| Клітинка | Джерело |
|---|---|
| `door_open` | 244425 · creaking-door-open01.flac · Aiyumi |
| `door_close` | 216879 · squeaky wooden door solid close.wav · CastIronCarousel |
| `door_bell` | 192761 · Ryuuzan_shop_door_bell00.wav · ryuuzan |

Грім — той самий `lightning.wav` (743447 · Thunderclap one shot · breadmanelex), що й удар блискавки по дереву.

## Інтерфейс (`web/public/audio/ui/*.wav`, WebAudio у `hud/sound.ts`)

| Звук | Джерело |
|---|---|
| `click` | 687107 · Mouse 2 Button Fast Click · aphom000 |
| `open` | 201643 · Menu Scroll/ Selection Sound · CS279 |
| `close` | 264445 · Button Sound Closed 2 · kickhat |
| `coin` | 213985 · coin clatter 5.wav · FenrirFangs |
| `error` | 349899 · Digital Error · dotY21 |
| `toast` | 740423 · Message Notification 4 · AnthonyRox |
| `levelup` | 442943 · Level Up · qubodup |
| `craft` | 270588 · Anvil Hit 2 · michorvath (перші 0,75 с); 545238 · Item Sparkle · Mr._Fritz_ (із затримкою 0,32 с) — загальний, для рецептів поза родинами нижче |
| `cook` (`dish_*`) | 839155 · Quick_vegetable_chop_9 · BenParamoreAudio; 234124 · Sizzle.wav · CHEATSman115 (із 0,25 с, −5 дБ); 26478 · stirring-teapot.wav · osivo (із 0,6 с) |
| `build` (`decor_*`) | 501316 · Sawing wood · JappeHallunken; 342542 · Outside Hammer · sgrowe (із 0,85 с, +12 дБ: джерело тихе) |
| `forge` (`tool_*`) | 270589 · Anvil Hit 1 · michorvath (двічі: 0 і 0,45 с); 234782 · Steam/hiss · wubitog (гартування, із 0,9 с) |
| `smelt` (`ingot_*`) | 368498 · bellows.wav · lensson (міх, перша секунда); 386166 · Gas furnace - Ignition happy · ldezem (із 0,7 с, спалах печі); 386150 · Forge - Coal burning with fan on close · ldezem (3,0–4,8 с, −8 дБ, із 1,0 с); 386221 · Steel - Hot steel burning wood short · ldezem (гаряча сталь, із 1,5 с, +6 дБ); компресор, бо спалах глушить решту |

Сторінка звуку: `https://freesound.org/people/<автор>/sounds/<id>/`. Запасні кандидати (теж CC0): удар сокири 583272 · Axe Chop.mp3 (Evcy); кирка 638696 · Pickaxe (CaptainYulef);
грім 38244 · thunder1.wav (nednednerb); монета 336937 · Coin8 (The-Sacha-Rush, 8-бітна); сповіщення 571513 · Soft-Notifications Bell (LegitCheese); рівень 528958 · Level Up / Mission Complete (Beetlemuse);
острів 404696 · By the sea (OSFX), 517678 · Seagulls by the sea (TeTeNoise); материк 471891 · Spring Birds (hargissssound), 723913 · Forest birds seamless loop (Magnesus); печера 402929 · Drops in the small cave (Selector), 636125 · ambience cave7 (fonografico).

## Чого ще нема (кандидати на наступну задачу)

Кроки в боковій ходьбі (`walk_left/right`) і присіді.
