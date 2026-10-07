# BRIDGE_PROTOCOL — Godot ⇄ React

Механізм: Godot `JavaScriptBridge.get_interface("window").cf` для викликів у React; React викликає у Godot через колбек, зареєстрований Godot-ом (`JavaScriptBridge.create_callback`) як `window.cf.toGodot`. Однопотоковий експорт → виклики в межах кадру, без черг і таймерів.

Джерело істини — `shared/bridge-protocol.json` (JSON Schema). TS-типи і GDScript-валідатор генеруються з нього. Цей документ — опис.

## Принципи

1. **Один напрямок для даних, інший для намірів.** React → Godot: стан і команди відображення. Godot → React: наміри гравця (клік, вибір). Godot ніколи не запитує і не чекає відповіді.
2. **Godot не отримує секретів.** Жодних сесій, сум у CLASH, цін, адрес. Лише публічний стан світу.
3. **Кожне повідомлення `{ v: 1, type: string, payload: object }`.** Невідомий `type` або невалідний `payload` → ігнор + `console.warn` (React) / `push_warning` (Godot). Ніколи не кидати виняток на вхідних даних.
4. **Ідемпотентність.** `world_state` — повний або частковий снапшот; Godot застосовує «як є», без припущень про порядок.
5. **Без `eval`.** Godot не викликає `JavaScriptBridge.eval` з рядками від сервера/гравця.

## React → Godot

| type | payload | Опис |
|---|---|---|
| `session_ready` | `{ island_id }` | Можна показувати світ. До цього — екран очікування. |
| `world_state` | `{ full: bool, plots: Plot[], decor: Decor[], resources?: Resource[], cleared?: [[x_dm, z_dm], …], expand?: { n, e, s, w }, weather?: { state, until }, level, now }` | `weather` — спільна погода з сервера (`clear\|cloudy\|fog\|rain\|storm`): Godot бере її замість локального розкладу; dev-меню (`debug_world`) усе ще перекриває.  `expand` — смуги розширення по боках (ADR-015): Godot перебудовує прямокутник острова (земля, колізія, межі, сітка маршрутів, трава, привид) — 24 м + 8 м на смугу.  `Decor = { id, item_id, x_dm, z_dm, yaw_deg }` — точка в дециметрах від центру острова і поворот у градусах (0009), Godot інстансує предмет сам; `Resource = { idx, kind: "tree"\|"rock", hits_left, respawn_at?, x_dm, z_dm, stage, variant }` — живі ресурси (0017): точку обирає сервер, Godot ставить вузол у `(x_dm, z_dm)` (стартові 8 стоять у сцені, невідомий `idx` інстансується з `tree_scene`/`rock_scene` `ResourceGrid`), `stage` 0/1/2 — саджанець/молоде/стигле (масштаб, збирати можна лише 2); на 0 ударів Godot показує пеньок до `respawn_at`, після якого сервер переносить вузол.  Снапшот або патч. `Plot = { idx, col, row, state: "empty"\|"growing"\|"ready", crop_id?, ready_at? }`; `col,row` — клітинка сітки 3 м від центру острова, Godot сам інстансує грядку на ній. Повний снапшот прибирає грядки, яких у ньому нема; патч лише оновлює/додає. Godot показує таймер лише візуально від `now`/`ready_at`. `cleared` (0023) — місця розчищених кущів (дм від центру острова): `IslandWild` не сіє дику флору ближче за 1,7 м і перебудовується; без поля список не змінюється. |
| `inventory_changed` | `{}` | Сигнал перемалювати підказки (Godot не тримає інвентар). |
| `set_mode` | `{ mode: "walk"\|"build"\|"ui", item?, footprint_dm? }` | `ui` = React відкрив модалку → Godot ігнорує ввід. `build` без `item` — привид грядки по сітці 3 м; з `item` (id декору) і `footprint_dm` — привид цього предмета, прилипання 0.5 м, поворот R. |
| `play_fx` | `{ fx: "harvest"\|"plant"\|"water"\|"fertilize"\|"place"\|"chop"\|"mine"\|"gather"\|"lightning"\|"cheer"\|"fail"\|"emote_<id>", target?: { plot_idx? \| slot? \| resource_idx? \| node_id? \| bush_id? }, tool?: "axe"\|"pickaxe"\|"hoe"\|"scythe"\|"spade"\|"hammer"\|"" }` | Візуальний ефект; персонаж грає кліп дії з інструментом у правій руці за таблицею `FxClips` (`client/scripts/fx_clips.gd`; `chop` = замах сокирою, `mine` = кирка по каменю, `harvest` = коса, `place` = молоток навколішки, `plant` = навколішки, `gather` = підняти з землі, `dig` = лопата); `tool` перекриває інструмент з таблиці (`""` = голіруч — так каже материк, коли інструмента нема); вузол здригається; `lightning` = спалах у бік дерева; `sleep` = коли панель дому закривається, екран темнішає на кілька секунд (ніч минає всередині), потім персонаж виходить; `sit` з `target.decor_idx` = персонаж сідає на лавку і сидить, поки гравець не піде); `cheer` (новий рівень) і `fail` (помилка дії) — реакції, що чекають кінця поточної дії; `emote_wave`/`cheer`/`laugh`/`dance`/`heart`/`shrug`/`thumbs_up`/`clap` — колесо емоцій HUD (ADR-035), сусіди бачать їх через `anim` присутності). |
| `camera_focus` | `{ target: "island"\|"mainland"\|{ plot_idx } }` | Керування камерою з UI. |
| `set_volume` | `{ sfx: 0…100, ambient: 0…100 }` | Гучність з меню (UX етап 2): `sfx` — дії у світі (і звуки інтерфейсу, які грає сам React), `ambient` — море/ліс/печера. Godot зберігає в `user://settings.cfg` `[audio]` і відповідає `volume_changed`. |
| `theme` | `{ season_id }` | Сезонний скін світу. |
| `player_look` | `{ look: { skin, shirt, pants, shoes, … } }` кольорами «#rrggbb» і id частин (`body: "body_m" \| "body_f"` — стать тіла, ADR-027; `build: "build_avg" \| "build_slim" \| "build_sturdy"` — статура, ADR-029; нема = чоловіче середньої статури) | Вигляд гравця (ADR-025): `CharacterLook` перефарбовує зони палітри моделі; `Bridge.look` памʼятає його для сцен, що зʼявляються пізніше (зміна зони). Невалідний колір → колір за замовчуванням. Таблиці кольорів і їх індекси — `web/src/character/palette.ts`, сервер зберігає лише індекси. |
| `camera_preview` | `{ on, yaw_deg?, focus? }` | Екран створення персонажа: камера впритул, персонаж дивиться в неї (`yaw_deg` повертає самого персонажа для огляду збоку й ззаду; `focus` = `body` (весь зріст, за замовчуванням) або `head` (обличчя зблизька: вкладки волосся, обличчя, капелюха); персонаж збоку від панелі); `on: false` повертає звичайну камеру. Рух у цей час блокує `set_mode ui`. |
| `peers_state` | `{ peers: [{ id, nick, look?, x_dm, z_dm, yaw_deg, anim }] }` (`look` — кольори «#rrggbb» за слотами `skin`/`shirt`/`pants`/`shoes`, як у `player_look`; без нього — вигляд за замовчуванням) | Сусіди з presence-сокета (5 разів/с): `PeerAvatars` спавнить/оновлює/прибирає аватари, позиція інтерполюється, кліп — за назвою. |
| `set_zone` | `{ zone: "island" \| "mainland" \| "cave", x_dm?, z_dm? }` | Міняє світ: Godot вивантажує сцену й ставить іншу; `x_dm`/`z_dm` — де поставити персонажа в новій зоні (вихід з печери → вхід у шахту); після цього шле `zone_changed`. |
| `mainland_nodes` | `{ chunks: [{ cx, cz, biome, nodes: [{ id, kind, x_dm, z_dm, hits_left }] }] }` | Вузли ресурсів вікна чанків (`GET /mainland/chunks`): `MainlandNodes` ставить/оновлює/прибирає, висота — з карти терену. |
| `mainland_landmarks` | `{ landmarks: [{ id, name, x_dm, z_dm, discovered }] }` | Орієнтири, які гравець уже може бачити (`GET /mainland/explored`): `MainlandLandmarks` ставить стовпи з дошкою-табличкою (світовий напис, видно до 45 м); відкриті — з зіркою. Клік веде персонажа до таблички, `player_near { kind: "landmark", id }` → дія «Оглянути». |
| `mainland_node` | `{ id, hits_left }` | Стан одного вузла після удару (відповідь `gather`); `play_fx` з `target.node_id` повертає персонажа до вузла й трясе його. |
| `teleport` | `{ zone, x_dm, z_dm }` | Presence-сервер не повірив, де стоїть персонаж (кадр `reset`): ставимо його туди, якщо зона збігається. |
| `set_quality` | `{ tier }` | Рівень графіки з меню: `auto`, `low`, `medium`, `high`. Godot застосовує (тіні, MSAA, glow, густота трави), зберігає в `user://settings.cfg` і відповідає `quality_changed`. |
| `house` | `{ inside: bool }` | `true`: персонаж іде до порога, проходить у двері і зникає, Godot відповідає `house_entered`; `false`: зʼявляється у дверях і виходить на поріг. |
| `walk_to` | `{ kind: "plot"\|"decor"\|"resource"\|"house", idx }` | Персонаж іде до обʼєкта і зупиняється поруч (у радіусі взаємодії). |
| `debug_world` | `{ hour?, weather?, day_length_min?, lightning?, stats? }` | Dev-меню HUD (візуально, лише на цьому клієнті): заморозити годину (`null` = живий час), погода `clear\|cloudy\|fog\|rain\|storm\|auto`, довжина доби у хвилинах, `lightning: true` — спалах блискавки, `stats: true|false` — накладка статистики кадру (те саме, що F3). |
| `build_rotate` | `{ delta_deg }` | Повернути привид декору (кнопка «Повернути» в HUD; клавіша R у Godot робить те саме). |
| `move_input` | `{ x, y }` | Вектор джойстика з HUD, обидві осі в [-1, 1]; `y < 0` — вперед. Нульовий вектор = відпустили. |
| `jump` | `{}` | Кнопка стрибка на телефоні (на клавіатурі — пробіл усередині Godot). Виконується на наступному фізичному такті, якщо персонаж на землі. |

## Godot → React

| type | payload | Опис |
|---|---|---|
| `ready` (`version` — «0.1.0 <дата UTC> <git-коміт>» зі штампа `res://build.txt`, який `addons/build_stamp` кладе в експорт; у редакторі — «0.1.0 dev») | `{ version }` | Движок завантажився, зареєстрував колбек. |
| `plot_selected` | `{ idx }` | Клік по ділянці. React вирішує, що показати. |
| `player_pose` | `{ x_dm, z_dm, yaw_deg, anim }` | Поза персонажа 8 разів/с (лише при зміні + серцебиття щосекунди), цілі дециметри/градуси; React шле її в presence-сокет. |
| `zone_changed` | `{ zone }` | Сцена світу змінилась (після `set_zone`); React на острові знову шле `world_state`, на материку чекає `player_chunk`. |
| `debug_probe` | `{ x, y, z, on_floor, cam_yaw_deg, ground_y?, city_y?, city_lowest_y? }` | Лише dev: відповідь на `debug_world { probe: true }` — де насправді персонаж і земля в цій збірці (перевірка web-експорту). |
| `quality_changed` | `{ tier, effective }` | Який рівень графіки діє: `tier` — вибір гравця, `effective` — що застосовано (у режимі `auto` слабка машина сама сходить на рівень нижче за p95 кадру). |
| `volume_changed` | `{ sfx, ambient }` | Гучність, що діє: при старті (збережена) і після кожного `set_volume`; web бере `sfx` для своїх UI-звуків. |
| `zone_ready` | `{ zone }` | Сцена зони справді готова (`ZoneSettle`: прогрів шейдерів, перші чанки й флора, дика рослинність острова; або тайм-аут 6 с). Оболонка тримає завісу переходу від `set_zone` (і від `ready` на старті) до цього повідомлення. |
| `player_chunk` | `{ cx, cz }` | Персонаж перейшов в інший чанк 64 м материка; React тягне вікно 7 × 7 і відповідає `mainland_nodes`. |
| `house_entered` | `{ ok }` | Відповідь на `house { inside: true }`: `ok` — персонаж дійшов до порога і пройшов у двері, React відкриває панель «Дім»; `false` — підхід скасовано (клік убік, модалка), персонаж лишається надворі. |
| `building_entered` | `{ building_id }` | Персонаж підійшов до будівлі міста (Фаза 2). |
| `mode_changed` | `{ mode }` | Підтвердження зміни режиму. |
| `error` | `{ code, detail }` | Нефатальна помилка рендера/ассетів. |
| `player_near` | `{ kind: "plot"\|"decor"\|"resource"\|"house"\|"mnode"\|"landmark"\|"building"\|"bush"\|"none", idx, distance, id? }` | Найближчий обʼєкт у радіусі 1.6 м змінився; Godot підсвічує його, React показує контекстну дію. `none` — нічого поруч. `bush` (з 2026-09-30): дикий кущ острова з колайдером (`WildBush`, `id = "x_dm,z_dm"`), найнижчий пріоритет — біля грядки, ресурсу чи декору обирається вони, а не кущ; дія «Clear bush» (`POST /island/clear`). |
| `place_selected` | `{ x_dm, z_dm, yaw_deg }` | Режим `build` з предметом декору: гравець підтвердив точку (дециметри від центру острова) і поворот (крок 15°). React шле `POST /decor/place`; сервер вирішує. |
| `cell_selected` | `{ col, row }` | Режим `build`: гравець підтвердив клітинку сітки під нову грядку (привид зелений). React шле `POST /plots/place`; сервер вирішує. |

## Життєвий цикл

1. React монтує canvas, стартує `Engine`. 2. Godot → `ready`. 3. React → `session_ready` (якщо є сесія) або тримає Godot у режимі «острів-заставка» без сесії. 4. Далі — події за таблицями.
Втрата контексту WebGL → React показує «перезавантажити», не намагається відновити стан із Godot.

## Тестування

- `web/src/bridge/__tests__`: кожне повідомлення проти схеми; невалідні — ігноруються без виключень.
- `client/tests/bridge_test.gd`: те саме з боку Godot (GUT або вбудований тест-раннер).
- Ручний сценарій: відкрити модалку → рух персонажа заблокований; закрити → відновлений.
