# API v1 — контракт сервера

Базовий шлях `/api/v1`. JSON. Усі мутуючі запити: `POST`, cookie-сесія, заголовки `X-CSRF-Token`, `Idempotency-Key` (uuid), `Content-Type: application/json`, тіло ≤ 16 KB.
Джерело істини для схем — OpenAPI у `server/crates/api/openapi.yaml`; з нього генерується TS-клієнт. Цей документ — людський огляд.

## Загальні правила

- Відповідь помилки: `{ "error": { "code": "string", "message": "string", "request_id": "uuid" } }`. Коди стабільні, повідомлення — не для парсингу.
- Загальні (ADR-043): `409 busy` — конфлікт у базі (дедлок/серіалізація), повторити; `503 unavailable` + `Retry-After: 2` — база чи Redis на мить недоступні, повторити; `429 daily_cap` — `message` готовий для гравця («Daily limit reached, resets in X h Y min.»), `Retry-After` — секунди до 00:00 UTC.
- Чужий або неіснуючий ресурс → `404 not_found` (не розкриваємо існування).
- Rate-limit → `429 rate_limited` з `Retry-After`.
- Kill switch активний → `503 maintenance` або `409 shop_disabled` / `409 payments_disabled`.
- Усі часи — ISO-8601 UTC. Гроші — цілі: `price_usd_cents`, `amount_clash` (atomic units), `credits`.
- Idempotency: повтор із тим самим ключем протягом 24 год → та сама відповідь, без повторного ефекту.

## Auth

| Метод | Шлях | Опис |
|---|---|---|
| GET | `/auth/nonce?wallet=<sol:…|evm:0x…>&chain_id=` | Повертає `{ nonce, message, expires_at }`; `message` — готовий текст SIWS/SIWE (EIP-4361), клієнт підписує його байт-у-байт. `chain_id` лише для EVM, з дозволеного списку. Ліміт 10/хв/IP. |
| POST | `/auth/verify` | `{ wallet, signature, message }` (`signature` — hex сирих байтів: 64 ed25519 або 65 secp256k1) → cookies `cf_access` (15 хв, Path=/api), `cf_refresh` (7 дн, Path=/api/v1/auth), `cf_csrf` (читається JS), повертає `{ user }`. Nonce спалюється до перевірки підпису. Ліміт 5/хв/wallet. |
| POST | `/auth/guest` | Гостьовий острів без гаманця (0028, ADR-031): тіла нема → ті самі cookies, що й у `verify`, `{ user: { id, wallet: null } }`. CSRF-токена ще нема (перевіряється лише `Origin`), ліміт 10/хв/IP і 3 гості на добу на IPv4-адресу чи IPv6 /64 (`429 rate_limited`), вимкнено `guests_enabled`/`registrations_enabled` → `503 guests_disabled`/`registrations_disabled`. У браузері вже є жива сесія (cookie доступу дійсний) → `409 session_exists`, нічого не створюється (ADR-043). |
| POST | `/auth/link` | Гість прив'язує гаманець, острів лишається: тіло як у `verify` (той самий nonce-текст і підпис), потрібні сесія й `X-CSRF-Token` → нові cookies (усі попередні сесії гостя відкликано) і `{ user: { id, wallet } }`. Гаманець уже має акаунт → `409 wallet_has_account` (акаунти не зливаються); у акаунта вже є гаманець → `409 already_linked`; поганий підпис чи nonce → `401`. Ліміт як у `verify`. |
| POST | `/auth/refresh` | Ротація refresh; повторне використання старого → відкликання сімʼї. Виняток (ADR-036): той самий браузер (user agent) повторює щойно замінений токен протягом 30 с (втрачена відповідь, друга вкладка) — ротація від живого токена сімʼї, без відкликання; після виходу, відкликання чи бану живого токена нема → 401. Клієнт робить refresh під спільним для вкладок замком (Web Locks) і не виходить із сесії, якщо refresh не дістав відповіді (мережа, 5xx, 429). |
| POST | `/auth/logout` | Видаляє сесію з Redis. |
| GET | `/me` | `{ user: { id, wallet, nickname, look, reputation, flags, rename_free }, balances: [{ account, amount }] }` (`rename_free` — нік скинув модератор, наступна зміна без квитка, 0031) (`wallet` = `null` у гостя) — баланси лише з `balances`, ніколи не рахуються на клієнті. `nickname`/`look` = `null`, доки акаунт не створив персонажа (клієнт показує екран створення). |
| POST | `/me/character` | Створення персонажа один раз (ADR-025): `{ nickname, look: { skin 0..7, shirt 0..11, pants 0..11, shoes 0..5, hair_color 0..11, hair 0..9, face 0..4, beard 0..5, hat 0..7, glasses 0..2, outfit 0..8, outfit_color 0..11, body 0..1, build 0..2 } }` → `{ nickname, look }`. Нік: 3–16, латиниця/цифри/`_`, з літери, без `_` на краях і подвоєних → інакше `400 invalid_nickname` (у `message` причина); службове чи грубе слово → `400 nickname_not_allowed`; вигляд поза таблицями → `400 invalid_look` (поля `hair_color`, `hair`, `face`, `beard`, `hat`, `glasses`, `outfit`, `outfit_color`, `body`, `build` необовʼязкові: пропущене = 0 (`body`: 0 чоловіче, 1 жіноче тіло, ADR-027; `build`: 0 середня, 1 струнка, 2 міцна статура, ADR-029), тобто без зачіски, спокійне обличчя, без бороди, капелюха, окулярів і вбрання; 0 у `hair`/`beard`/`hat`/`glasses`/`outfit` = «нічого»; невідоме поле → `4xx`); нік або його «двійник» (регістр, `0/o`, `1/i/l`, `_`) уже зайнятий → `409 nickname_taken` (гонка: один переможець); персонаж уже є → `409 character_exists` (нік не змінюється). `Idempotency-Key` обовʼязковий: повтор з тим самим ключем віддає першу відповідь. Ліміт як у інших діях світу; кожна спроба коштує 1 з 30 на добу (`429 daily_cap`), успішна повертає одиницю — «зайняте» не можна перебирати безкоштовно. |
| POST | `/me/look` | Платна зміна вигляду (0029, ADR-038): `{ look }` (той самий формат, що в `/me/character`) → `{ nickname, look }`; нік не змінюється. В одній транзакції під замком рядка гравця списує один `restyle_ticket` (купується за CLASH звичайним `/orders`, $0,99) і записує вигляд. Персонажа нема → `409 no_character`; той самий вигляд → `409 same_look`; поза таблицями → `400 invalid_look`; квитка нема → `409 no_ticket` — у всіх відмовах нічого не списано. `Idempotency-Key` обовʼязковий (повтор віддає першу відповідь, другий квиток не списується); ліміт як у інших діях світу. |
| POST | `/me/nickname` | Платна зміна ніка (0030, ADR-039): `{ nickname }` → `{ nickname, look }`. Ті самі правила, що в `/me/character` (`400 invalid_nickname`, `400 nickname_not_allowed`, `409 nickname_taken` — зайнятий, «двійник» або звільнений іншим гравцем менш ніж 30 днів тому), той самий денний кап спроб (30, `429 daily_cap`, успіх повертає одиницю). В одній транзакції списує один `rename_ticket` (купується за CLASH звичайним `/orders`, $5), міняє нік і пише рядок у `nickname_history`; старе ім'я 30 днів тримається за гравцем. Той самий нік → `409 same_nickname`; інший регістр свого — дозволено (за квиток); квитка нема → `409 no_ticket`; персонажа нема → `409 no_character` — у відмовах нічого не списано. `Idempotency-Key` обовʼязковий. |
| GET | `/me/nickname-check?nick=` | Чи вільне імʼя для цього акаунта: `{ available: true }` або `{ available: false, reason: "invalid"\|"not_allowed"\|"taken", message }`. Кожен виклик — 1 з `200` на добу (`429 daily_cap`), щоб не перебирати чужі імена. З 0030 доступна й гравцю з персонажем (перед перейменуванням); свій нік в іншому регістрі — вільний, звільнений іншим гравцем менш ніж 30 днів тому — `taken`. |

## Світ

| Метод | Шлях | Опис |
|---|---|---|
| GET | `/island` | `{ island: { id, level, xp, expand_n/e/s/w }, plots: [{ id, idx, col, row, bed, state: empty|growing|ready, crop_id, planted_at, ready_at, fertilizer_id }], now }`; створює острів з `PLOTS_START` ділянками і стартовим насінням при першому виклику. `col,row` — клітинка сітки 3 м (центр = `col*3, row*3` м від центру острова), `bed` — номер групи суміжних грядок у цьому знімку. Також `plot_price` (ціна наступної грядки) і `plot_limit` (скільки грядок острів може мати на своєму рівні: поставлені + в сумці; ADR-043). |
| POST | `/island/expand` | `{ side: "n"\|"e"\|"s"\|"w" }` (ADR-015, 0010): списує дозвіл наступного рівня цього боку (`island_expand_<n>`, куплений за CLASH через `/orders`) і збільшує `islands.expand_<side>`; `409 insufficient_items` без дозволу, `409 expand_max` після 4 смуг, `400 invalid_side`. → `{ island, side, level, permit_used }`. Нові смуги одразу доступні для декору й грядок. |
| POST | `/island/clear` | `{ x_dm, z_dm }` (0023, запит власника 2026-09-30): гравець прибирає дикий кущ **руками, безкоштовно й без нагороди** — сервер лише запамʼятовує місце (округлення до 0,5 м, `cf.island_clearings`; повтор — той самий рядок, `Idempotency-Key`), клієнти тримають дику флору подалі. `400 out_of_bounds` поза формою острова, `409 clearing_limit` після 400 місць. → `{ cleared: [[x_dm, z_dm], …] }` (усі місця острова). `GET /island` теж віддає `cleared`. Косметика й місце під грядки: ніщо в економіці не читає таблицю. |
| POST | `/plots/place` | `{ col, row }` → списує предмет `plot` з інвентаря, ставить порожню грядку. Помилки: `400 out_of_bounds` (за верхом острова з розширеннями ADR-015), `409 cell_taken`, `409 plot_limit` (ліміт за рівнем, ECONOMY), `409 bed_too_large` (група суміжних > `BED_MAX_CELLS`), `409 insufficient_items`. `Idempotency-Key` обовʼязковий. |
| POST | `/plots/{id}/remove` | Лише порожня грядка → `{ removed: id }`, предмет `plot` повертається в інвентар. Інакше `409 wrong_state`. |
| POST | `/plots/{id}/bed/plant` | `{ item_id }` → садить у **кожну порожню** грядку групи, до якої належить `{id}`, по одному насінню на клітинку, в одній транзакції (усе або нічого: брак насіння чи денний кап відкочують усе). → `{ plots, count }`. Нема порожніх → `409 wrong_state`. |
| POST | `/plots/{id}/bed/fertilize` | `{ item_id }` → добриво в кожну ростучу неудобрену грядку групи. → `{ plots, count }`. |
| POST | `/resources/{idx}/gather` | Один удар по дереву чи каменю острова (`idx` як у сцені). Перевірки: власність через острів (чужий/невідомий → `404`), вузол не відновлюється (`409 depleted`), дерево ще саджанець (`409 not_ready`, 0017), денний кап `GATHER_MAX_PER_DAY` (`429 daily_cap`). Списує 1 удар, кладе `wood`/`stone` в інвентар, дає `GATHER_XP`. На 0 ударів — `respawn_at = now() + *_respawn_secs` з БД; після нього джоба `regrow_resources` (раз на хвилину) переносить вузол на вільне місце острова як саджанець (стадії 0/1/2 за `economy.tree_grow_secs`, камені цілі одразу), куплені смуги отримують `economy.strip_trees`/`strip_rocks` вузлів. → `{ node: { idx, kind, hits_left, respawn_at?, x_dm, z_dm, stage, variant }, item_id, qty, xp, level }`. `Idempotency-Key` обовʼязковий. `/island` віддає `resources: [node...]`; позиції вузлів обирає лише сервер. |
| POST | `/plots/{id}/bed/harvest` | Збирає кожну готову грядку групи → `{ plots, count, xp, level, items: [{item_id, qty}] }` (продукт сумарно). Нічого готового: `409 not_ready` якщо щось росте, інакше `409 wrong_state`. |
| GET | `/recipes` | `{ recipes: [{ id, output_item, output_qty, inputs: [{item_id, qty}], xp }] }` — увімкнені рецепти (0008). |
| POST | `/market/sell` | `{ item_id, qty }` → продає продукт чи страву за `items.meta.sell_credits` через ледж (`system:mint` → кредити гравця) під `CREDITS_MAX_PER_DAY`: продається стільки цілих одиниць, скільки покриває залишок дня, решта лишається; нуль → `429 daily_cap`. Предмет без ціни продажу → `400 invalid_item`. → `{ item_id, qty_sold, credits_earned, credits }`. `Idempotency-Key`. |
| POST | `/craft` | `{ recipe_id, times? }` → списує всі складники × times, додає результат × times, XP рецепта × times; усе або нічого (`409 insufficient_items` нічого не списує); кап `CRAFT_MAX_PER_DAY` рахує одиниці — робиться стільки, скільки покриває залишок дня (`qty` у відповіді), нуль → `429 daily_cap`. → `{ recipe_id, output_item, qty, xp, level }`. |
| GET | `/weather` | Спільна погода з годинника БД: `{ state: clear\|cloudy\|fog\|rain\|storm, slot, since, until }` — час різано на слоти `WEATHER_PERIOD_MIN`, стан слоту — хеш від `weather.seed`; `weather.force` у settings закріплює стан (демо, тести). Та сама погода є в `/island` як `weather`. |
| POST | `/weather/lightning` | Лише під час `storm` (інакше `409 no_storm`): один удар на острів за слот грози — сервер сам обирає дерево (`resource_idx`) і кладе `LIGHTNING_WOOD` деревини в інвентар; повтор у тому ж слоті → `{ struck: false }`; кап `LIGHTNING_MAX_PER_DAY` → `429 daily_cap`. → `{ struck, slot, resource_idx, item_id, qty }`. |
| POST | `/house/sleep` | Дім: лягти спати → `SLEEP_XP` XP (не множиться «Відпочив») і старт статусу «Відпочив» (0026): відповідь має `rested: { until, bonus_pct }` (немає, коли статус вимкнено в settings), `GET /island` віддає `rested` лише поки статус триває; тоді XP від дій більший на `bonus_pct` %, кап `SLEEP_MAX_PER_DAY` (1 на добу UTC) → `{ xp_gain, xp, level }`; `429 daily_cap`. Готування в домі — звичайний `/craft`; де стоїть персонаж, сервер не знає і не перевіряє. |
| POST | `/items/{id}/eat` | Лише страви (`kind = dish`): списує одну, дає `meta.xp`; кап `EAT_MAX_PER_DAY`. → `{ item_id, xp, level }`. |
| POST | `/plots/{id}/plant` | `{ item_id }` → списує насіння з інвентаря, ставить `growing` (`ready_at = now() + grow_secs` з БД), повертає ділянку. `Idempotency-Key` обовʼязковий (uuid); повтор ключа → та сама відповідь без ефекту, ключ у процесі → `409 in_progress`. |
| POST | `/plots/{id}/fertilize` | `{ item_id }` → скорочує `ready_at` за правилом ECONOMY. |
| POST | `/plots/{id}/harvest` | Якщо `now >= ready_at` (стан `ready` — похідний) → `{ plot, xp, level, item_id, qty }`: продукт культури (`seed.meta.produce`, 1 шт.) в інвентар, ставить `empty`. Кредитів збір не дає з 0008 — вони з продажу. Інакше `409 not_ready`. |
| POST | `/decor/place` | `{ item_id, x_dm, z_dm, yaw_deg }` (0009): точка в дециметрах від центру острова, поворот 0–345 кроком 15. Сервер перевіряє: верх острова з розширеннями ADR-015 і відступом від краю (`400 out_of_bounds`), перекриття з іншим декором за `items.meta.footprint_dm` і з клітинками грядок (`409 overlap`), ліміт `DECOR_SLOTS` на острів (`409 decor_limit`), крок повороту (`400 invalid_rotation`). Списує предмет з інвентаря → `{ id, item_id, x_dm, z_dm, yaw_deg }`. `/island.decor` віддає той самий вигляд. Відомо: дерева й каміння сервер не перевіряє (їх позиції лише в сцені), тому підроблений запит може поставити декор «у дерево» — косметика, економіки не торкається; клієнтський привид це блокує фізикою. |
| POST | `/decor/remove` | `{ id }` → назад в інвентар; чужий чи невідомий id → `404`. |
| GET | `/inventory` | `{ items: [{ item_id, qty }] }` (лише qty > 0). |

Кожен хендлер світу: сесія → `user_id` → `island_id` → перевірка власності в SQL → `FOR UPDATE` → ліміти → дія → ледж → відповідь. Нічого з тіла не використовується як ідентифікатор власника.

## Каталог і покупки за кредити

| Метод | Шлях | Опис |
|---|---|---|
| GET | `/catalog` | (+ `payments: { available, fake }` — чи приймаються замовлення CLASH і чи це локальний тестовий режим `CF_PAYMENTS_FAKE`; у `fake` HUD не кличе гаманець і показує віртуальний баланс)  Увімкнені предмети з `price_usd_cents` і, для кредитних, `price_credits`. |
| POST | `/shop/buy-credits` | `{ item_id, qty }` (1 ≤ qty ≤ `SHOP_MAX_QTY`) → одна транзакція: ledger `credits → system:sink` + інвентар; відповідь `{ item_id, qty, spent_credits, credits_left }`; `409 insufficient_funds`, `400 invalid_qty`, `403 exclusive_locked`. `Idempotency-Key` обовʼязковий. Грядка (`plot`) понад `plot_limit` рівня → `409 plot_limit`, нічого не списано. |

## Payments (CLASH)

| Метод | Шлях | Опис |
|---|---|---|
| POST | `/orders` | Гість без гаманця → `403 wallet_required` (спершу `/auth/link`). `{ item_id, qty }` → `{ order_id, item_id, qty, price_usd_cents, amount_clash, burn_amount, transfer_amount, clash_per_usd, quote_expires_at, memo, mint, decimals, treasury_token_account }` (ADR-017). Лише CLASH-товари (`price_usd_cents`), лише Solana-гаманець (`409 wallet_not_solana`), мін. сума `$0.50`, `Idempotency-Key`. Оракул недоступний → `503 oracle_unavailable`. |
| POST | `/orders/{id}/submit` | `{ signature }` (base58, 64 байти) → синхронно: `200 { status: fulfilled|rejected|needs_review, ... }` або `202 { status: submitted }`, поки RPC ще не бачить транзакцію (повторити пізніше з тією ж сигнатурою). Сигнатура після кінця котирування (і для `expired`) перевіряється як завжди: блок у вікні [created − 60 с, `quote_expires_at` + 120 с] → `fulfilled`, пізніше → `rejected/too_late` (ADR-043). Замовлення приймає лише першу сигнатуру: інша (і в `needs_review`) → `409 wrong_status`. `409 sig_reused`. Вимикачі платежів на `submit` і `GET /orders/{id}` не діють. |
| GET | `/orders/{id}` | Статус: `quoted|submitted|fulfilled|expired|rejected|needs_review` + `reason_code`. |

Правила `submit`: сигнатура — base58, 87–88 символів; ордер належить сесії; статус `quoted` і `now < quote_expires_at + 120s` (запас на підтвердження); далі — 6 перевірок із `SECURITY.md` §4. Відхилення = `rejected` з `reason_code` (`wrong_mint`, `wrong_owner`, `amount_low`, `memo_mismatch`, `tx_failed`, `forbidden_instruction` — включно з переказом не на казначейський рахунок, `too_old`, `too_late` — блок після `quote_expires_at + 120 с`, `malformed` — відповідь RPC без `blockTime`/`meta.err`, `sig_reused`); розбіжність двох RPC, збій RPC або фулфілменту = `needs_review` з фіксованим `reason_code` (`rpc_error`, `fulfil_error`) — деталі лише в логах. Замовлення у статусі `submitted` теж стає `expired` після `quote_expires_at + 120 с`. Ліміт: 30 запитів `/orders*` за хвилину на користувача поверх ліміту світу.

## Admin (`cf-admin`: окремий бінарник, `CF_ADMIN_BIND` лише loopback, доступ SSH-тунелем)

React-застосунок (ADR-022, `web/src/admin`, збірка `web/dist-admin`) над JSON-API під `/api/*`; CSP `default-src 'none'; script-src 'self'; style-src 'self'`, `nosniff`, `Cache-Control: no-store`, тіло ≤ 16 КБ. Сесія — cookie `cf_admin` (HttpOnly, SameSite=Strict, Secure поза local), 30 хв ковзних у Redis, не довше 8 год; кожна мутація несе заголовок `X-CSRF` = токен сесії (сталий час), `Origin` перевіряється, 60 мутацій/хв на сесію, важкі читання (звірка, експорти) — 5/хв. Домен і URI повідомлення для підпису — з `CF_ADMIN_ORIGIN` (або `admin.<CF_SIWS_DOMAIN>`), тому підпис ігрового входу адмінку не відкриває. Ліміт входу: 10/хв на гаманець, 60/хв на бінарник (проксі перед адмінкою немає, IP не враховується). Другий фактор (ADR-023): гаманець із записом у `CF_ADMIN_TOTP_SECRETS` після підпису отримує pending-cookie `cf_admin_2fa` (Path `/login`, 5 хв) і має надіслати TOTP-код; у prod обовʼязково для всіх адмінів. Помилки API — `{ error: { code, message } }`.

| Метод | Шлях | Опис |
|---|---|---|
| GET | `/login`, `/login.js`, `/admin.css` | Сторінка входу: `window.solana` (Phantom) підписує повідомлення сервера. |
| GET | `/login/nonce?wallet=sol:<base58>` | Лише гаманці з `CF_ADMIN_WALLETS` (інакше `403`, лог `admin_login_denied`); 10/хв на IP. → `{ nonce, message }`. |
| POST | `/login/verify` | `{ wallet, signature (hex), message }` → cookie + `{ ok, csrf }`; `audit_log` `admin_login`. |
| POST | `/logout` | Форма з `csrf`; сесію відкликано. |
| POST | `/login/totp` | `{ code }` з pending-cookie `cf_admin_2fa` → cookie сесії + `{ ok, csrf }`; код один раз на 30-с крок, 5/хв і 50/добу на гаманець; відмова → `403` і `audit_log` `admin_totp_rejected` (правило `totp_rejected` при ≥ 5 за годину). |
| GET | `/api/me` | `{ wallet, csrf, env, version, review_queue, open_alerts, totp }`. |
| GET | `/api/alerts?state=open\|resolved\|all&page=&per_page=` | Рядки `cf.alerts` (+`total`), активні `mutes`, `job_last_run_at` джоби `alerts`, allowlist правил. |
| POST | `/api/alerts/{id}/ack` | `{ note? }` → підтверджено, нагадування зупиняються; повторно → `400`; `audit_log` `alert_ack`. |
| POST | `/api/players/{id}/nickname-reset` | `{ nickname, reason }` (`nickname` — ім'я з картки; причина обовʼязкова, ≤ 200) → `{ old, new, repeated }`: нік → `Farmer_<8 hex>`, одна безкоштовна зміна, старе ім'я заблоковане (0031, ADR-040); поки гравець носить заглушку, повтор → `repeated: true` без змін; нік уже інший, ніж `nickname` → `409`; персонажа нема → `409`; `audit_log` `player_nickname_reset`. |
| GET | `/api/name-blocks?q=` | Блоки імен (живі першими, ≤ 200): `{ rows: [{ id, nickname, key, reason, blocked_by, created_at, lifted_at, lifted_by, holder }] }`; `q` — ім'я (`ILIKE`) або його ключ. |
| POST | `/api/name-blocks` | `{ nickname, reason }` → `{ id, nickname, holder }`: блок наперед (і для схожих написань); ім'я поза правилами → `400`/`409`, уже заблоковане → `409`; `audit_log` `nickname_block`. |
| POST | `/api/name-blocks/{id}/lift` | Зняти блок (один раз; інакше `404`); `audit_log` `nickname_unblock`. |
| POST | `/api/alerts/mute`, `/api/alerts/unmute` | `{ rule, hours (1…168), note? }` / `{ rule }`; правило з allowlist (`ledger_mismatch`, `review_queue`, `maintenance_on`, `oracle_paused`, `signup_spike`, `totp_rejected`, `switch_off.<key>`, `job_failed.<job>`, `job_stale.<job>`); `audit_log` `alert_mute`/`alert_unmute`. |
| GET | `/` | Дашборд (2026-09-09, за макетом власника): картки гравців (усього, +7 днів), замовлень за день, черги на перевірку, спалених CLASH (`burns`), стовпчики замовлень за 7 днів і кільце виконано/відхилено/очікують (SVG на сервері, без JS), kill switches, останні дії, стан сервера. Метрики — `metrics.rs`, лише читання, `now()` з БД. |
| POST | `/switches` | `key` ∈ {maintenance, shop_enabled, payments_enabled, registrations_enabled}, `value` bool, `csrf` → `settings` + `audit_log`; cf-api бачить за ≤ 5 с. |
| GET/POST | `/settings` | Параметри `economy.*`/`weather.*` за списком `spec.rs` з межами (`burn_share_bp` не редагується — ADR-017); порожнє = дефолт із коду; кожна зміна → `audit_log`. |
| GET | `/orders?status=needs_review` | Список за статусом. |
| POST | `/orders/{id}/approve` | Лише `csrf`: підпис не вводиться — береться останній, який гравець реально надіслав (`events.order_submitted`, у списку є посилання на експлорер для перевірки); `needs_review → submitted →` звичайний фулфілмент (`burns` UNIQUE, ледж, доставка), після нього `reason_code = manual_review`, щоб звірка відрізняла ручні видачі; без надісланого підпису `400`, повторний/чужий підпис `409` і замовлення лишається в черзі. `audit_log`: `order_approve_attempt` до, `order_approve` / `order_approve_failed` після. |
| POST | `/orders/{id}/reject` | `reason` + `csrf` → `rejected` з `reason_code = admin:<reason>`; лише з `needs_review`. → `audit_log` `order_reject`. |
| GET | `/audit?q=` | Останні 200 рядків `audit_log`; `q` (≤ 64 символів) — пошук по actor/action/target через параметризований `ILIKE`, символи `%`/`_` екрануються. |
| GET | `/healthz` | `ok`. |

Метрики (D1/D7/D30, платящі, burn) — наступна ітерація.

## Cave and storms (печера і гроза, 0016, docs/MAINLAND.md §3–4)

| Метод | Шлях | Опис |
|---|---|---|
| GET | `/cave/{id}` | `{ nodes: [...] }` — вузли печери `id` (поки лише 1) у координатах сцени печери (дм), з живим станом; `404` для іншого id. Ids `99_<печера>_<i>`, вид `crystal` (рідкісний кап, кирка) поруч із рудою. Удар (`/mainland/nodes/{id}/gather`) вимагає позицію presence в зоні `cave`. |

Гроза на материку (`weather.state = storm`) валить 12 дерев у лісах на `economy.storm_fallen_secs` (30 хв) від початку грозового слоту: `GET /mainland/chunks` додає їх у вікно як вид `tree_fallen` (ids `98_<слот>_<i>`, 3 удари, без інструмента, `wood_pine`); удар по дереву неактивного слоту → `404`. Позиції — детермінована функція слоту, стан — у `mainland_nodes`. Presence приймає перший кадр материка і біля причалу, і біля входу в шахту (повернення з печери).

## City (місто M4, docs/MAINLAND.md §7)

| Метод | Шлях | Опис |
|---|---|---|
| GET | `/city` | `{ buildings: [{ id, kind, name, tagline, partner }] }` — будівлі площі; `kind` ∈ `market\|workshop\|hall\|showcase\|monument`, `partner` поки завжди `null`. Список фіксований у `cf_world::city` до міграції `partners`; Godot розставляє ті самі ids на площі. Ринок, майстерня і ратуша відкривають наявні екрани (`/market/sell`, `/craft`, `/tools`, `/mainland/expeditions`) — нових мутацій нема. |

## Exploration (дослідження, 0014/0015, docs/MAINLAND.md §4)

| Метод | Шлях | Опис |
|---|---|---|
| GET | `/mainland/explored` | `{ mask_hex, explored, landmarks: [{ id, name, x_dm, z_dm, radius_dm, xp, discovered }], total_landmarks }` — маска 32 байти (біт `cz*16+cx`), кількість відкритих чанків; у списку лише орієнтири, які вже відкриті або стоять у відкритому чанку (позиції невідкритих не витікають). |
| POST | `/mainland/explore` | Без тіла. Чанк береться з positions presence (зона `mainland`, не старше 10 с; інакше `409 too_far`), відкриваються 3 × 3 навколо → `{ view, revealed: [[cx, cz]], xp, level }`; нові чанки рахуються в експедиції «explore». Повтор на тому ж місці → `revealed: []`. `Idempotency-Key` обовʼязковий. |
| POST | `/mainland/discover` | `{ landmark_id }` (`[a-z0-9_]{2,32}`, інакше `404`). Перше відвідування в `radius_dm` (15 м) від орієнтира за presence → XP орієнтира і предмети з `landmarks.reward` в інвентар → `{ landmark, reward: [{ item_id, qty }], xp, level }`; далі `409 too_far`, повторно → `409 already_discovered` (PK `discoveries`). `Idempotency-Key` обовʼязковий. |
| GET | `/mainland/expeditions` | `{ day, tasks: [{ index, kind: gather\|explore\|discover, target, count, xp, progress, claimed }] }` — три завдання дня з детермінованого сіду (гравець, дата UTC); рядок створюється при першому запиті. Прогрес рахують самі дії (`gather` по виду вузла, `explore` по нових чанках, `discover`) у своїх транзакціях. |
| POST | `/mainland/expeditions/{n}/claim` | `n` ∈ 0..2 (інакше `404`); `progress < count` → `409 not_ready`, повторно → `409 already_claimed`; інакше XP завдання → `{ task, xp, level }`. `Idempotency-Key` обовʼязковий. |

## Tools (інструменти П7, 0013, docs/MAINLAND.md §5)

| Метод | Шлях | Опис |
|---|---|---|
| GET | `/tools` | `{ tools: [{ item_id: tool_axe\|tool_pickaxe, qty, wear, durability, repair: [{ item_id, qty }] }] }` — обидва інструменти, навіть якщо їх немає (`qty` 0). Інструмент — звичайний предмет інвентаря (крафт: `axe` = 4 соснові + 2 мідні, `pickaxe` = 3 дубові + 3 мідні); `wear` — удари поточної одиниці. |
| POST | `/tools/{id}/repair` | Списує матеріали з `items.meta.repair` (сокира 2 соснові, кирка 2 мідні) і скидає `wear` у 0 → `{ tool }`. `404` без інструмента, `409 wrong_state` якщо не зношений, `409 insufficient_items` без матеріалів. `Idempotency-Key` обовʼязковий. |

Удар по вузлу материка з потрібним інструментом (сокира — дерева, кирка — камінь і руда) дає 2 одиниці замість 1 і додає 1 зносу; на `economy.tool_durability` (100) одиниця ламається (`qty` −1, `wear` 0), відповідь `gather` містить `tool: { item_id, wear, durability, broke }` або `null`.

## Mainland (материк M2, docs/MAINLAND.md §5, §10)

| Метод | Шлях | Опис |
|---|---|---|
| GET | `/mainland/map` | `{ chunk_m, chunks, origin_m, rows }` — таблиця зон 16 × 16 (літера на чанк) для панелі карти; туман поки що лише в клієнті (маска `explored` — з міграцією M3). |
| GET | `/mainland/chunks?cx&cz&r` | Вікно чанків 64 м навколо (`cx`, `cz` ∈ 0..15, `r` ≤ 3, за замовчуванням 3) → `{ chunks: [{ cx, cz, biome, nodes: [{ id, kind, x_dm, z_dm, hits_left }] }] }`. Біом — літера з `shared/mainland_zones.json` (C місто, B пляж, M луки, F ліс, D темний ліс, R скелі, S болото, W вода). Вузли — детермінована функція сіду світу (ті самі на кожному інстансі), поверх — живий стан з `mainland_nodes` (0012): `hits_left` = 0, поки вузол відновлюється; `400 out_of_bounds` за межами карти. |
| POST | `/mainland/nodes/{id}/gather` | Один удар по спільному вузлу `id` (`cx_cz_i`). Перевірки: невідомий id → `404`; положення персонажа з presence-сокета (`presence:pos`, не старше 10 с) має бути в зоні `mainland` і не далі `economy.mainland_reach_dm` (6 м) від вузла → інакше `409 too_far` (без відкритого сокета збирати не можна); вузол відновлюється → `409 depleted` (стоячи біля нової точки; біля старої — `409 too_far`, бо вичерпаний вузол переїхав на запасну точку, 0024); денні капи `economy.mainland_hits_per_day` (60) і для рідкісних видів (`ore_iron`, `mushroom_rare`) ще `economy.mainland_rare_hits_per_day` (20) → `429 daily_cap`. Рядок вузла блокується `FOR UPDATE`: з двох одночасних останніх ударів виграє один. Списує удар, кладе предмет виду (`wood_pine`, `wood_oak`, `stone`, `ore_copper`, `ore_iron`, `herb`, `mushroom`, `mushroom_rare`, `reed`) в інвентар, дає `economy.mainland_gather_xp`. → `{ node: { id, kind, x_dm, z_dm, hits_left, respawn_at? }, item_id, qty (2 з інструментом, 1 без), xp, level, tool }` (`x_dm/z_dm` — вже наступна точка вузла; `GET /mainland/chunks` теж віддає поточну). `Idempotency-Key` обовʼязковий. |

## Presence (материк M1, docs/MAINLAND.md §8)

`GET /api/v1/ws/presence` — WebSocket. Апгрейд лише з cookie-сесією (`401` без неї) і з `Origin` з `CF_WS_ORIGINS` (`403` інакше); один сокет на користувача на інстанс (новіший закриває старіший), handshake ≤ 10/хв на користувача (`429`), сесія перевіряється кожні 30 с і при відкликанні сокет закривається. Кадри — JSON-текст ≤ 1 КіБ (більший кадр рве зʼєднання); клієнт, що 5 с не читає, відключається.

| Напрям | Кадр | Правила |
|---|---|---|
| сервер → клієнт | `{ t: "hello", id, nick, look }` | `id` — короткий стабільний хеш (12 hex), ніколи не гаманець; `nick` і `look` — з персонажа (ADR-025); поки його нема — «Farmer #xxxx» і `look: null` |
| клієнт → сервер | `{ t: "pos", zone: "island", x_dm, z_dm, yaw_deg, anim }` | цілі дециметри/градуси; `anim` — назва кліпу `[a-z0-9_]{1,24}`; не частіше 12/с (сплеск 24), швидкість ≤ 6 м/с, межі острова ±60 м; порушення рахуються, 20 → закриття |
| сервер → клієнт | `{ t: "identity" }` | Нік змінився деінде (скидання модератором, 0031, ADR-040): перевіряється раз на 30 с разом із сесією; сокет закривається, клієнт перечитує `/me` і перепідключається з новим `hello` |
| сервер → клієнт | `{ t: "reset", zone, x_dm, z_dm }` | Кадр не прийнято, бо сокет почав не з входу в зону (острів — спавн, материк — причал, радіус 30 м) і немає збереженої позиції за останні 120 с; клієнт ставить персонажа туди (`teleport`) |
| сервер → клієнт | `{ t: "peers", peers: [{ id, nick, look, x_dm, z_dm, yaw_deg, anim }] }` | 5 разів/с при зміні (і раз на секунду завжди): сусіди в клітинках 3 × 3 по 64 м, найближчі, ≤ 50; той, хто мовчав 5 с, зникає |

Кілька інстансів api зустрічаються через Redis pub/sub `presence:<zone>`. Економіки тут немає: позиції ніде не зберігаються (це прийде з материком для перевірки дистанції до вузлів).

## Заголовки відповіді

`Cache-Control: no-store` для всього під `/api`. `X-Request-Id` в кожній відповіді, той самий у логах.
