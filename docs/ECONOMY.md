# ECONOMY — параметри та інваріанти економіки

Параметри з цього файлу живуть у таблиці `settings` і змінюються через адмінку (з `audit_log`), не через реліз. Тут — їх значення на старті та обґрунтування. Змінюєш значення → оновлюєш і цей файл.

> Розрахунок балансу й пропозиція v2 (капи за рівнем, драбина будівель, замки CLASH-товарів за рівнем, живий ринок) — `docs/ECONOMY_LAB.md`, симуляція `docs/economy/sim.py` (2026-09-27, чекає рішень власника).

## 1. Інваріанти (незмінні без ADR)

- CLASH за покупку **спалюється на `BURN_SHARE_BP`** (старт 70%), решта переказується на казначейський мультисиг екосистеми (ADR-017). Гра не емітує, не тримає, не роздає CLASH.
- Каталог: насіння, добрива і простий декор — за кредити; ексклюзивне насіння, добрива з найкращими бустами, преміум-декор — за CLASH (рішення власника 2026-09-04).
- Кредити (`CREDITS`) — внутрішні, невивідні, не торгуються, не конвертуються назад ні в що зовнішнє.
- Ціни фіксуються в USD. CLASH — засіб оплати за ораклом.
- Жодного «стейкай Clash → отримуй Clash» і жодних безкоштовних кранів CLASH ні в цьому, ні в інших продуктах екосистеми (хартія 4 команд).
- Усе, що генерує кредити, має денний кап на акаунт.

## 2. Метрики, які визначають рішення

| Метрика | Формула | Ціль Фази 1 |
|---|---|---|
| **ROR** (return on rewards) | `CREDITS витрачено / CREDITS нараховано` за 30 днів | ≥ 1.0 |
| D1 / D7 / D30 | стандарт | ≥ 40% / ≥ 20% / ≥ 10% (альфа), ≥ 20% D30 для масштабування |
| Платящі | гаманці з ≥ 1 `fulfilled` order за 30 дн / MAU | ≥ 2% |
| ARPPU | $ burn / платящі | орієнтир $8–15 |
| Burn/день | сума `burns.amount` | — |
| Bot-suspect share | акаунти з `flags.suspect` / DAU | < 10% |

Критерій зупинки (з PLAN): 90 днів після беті D30 < 10% і платящі < 2% → материк не будуємо, переробляємо петлю.

## 3. Стартові параметри

```
# Острів
PLOTS_START=6            PLOTS_PER_LEVELS=2   PLOTS_MAX=20   # 0019: ліміт грядок = 6 + ⌊рівень/2⌋, до 20 на L28+ (було 6…12 до L5)
PLOT_PRICE_CREDITS=200   PLOT_PRICE_STEP=100  PLOT_CELL_M=3    BED_MAX_CELLS=9    # грядка = предмет `plot`; кожна понад стартові дорожча на 100 (7-ма 200, 8-ма 300 …); суміжні по боках зливаються в групу ≤ 9 (3×3); дії групи рахуються на клітинку
# Відомо: стартові 6 грядок можна прибрати і отримати 6 предметів `plot` (номінал 1200 кр.); продажу/повернення кредитів за предмети нема, тож емісії немає — при появі sell/trade виключити `plot` або зробити стартові неповоротними
ISLAND_BASE_M=48         ISLAND_EXPAND_STRIP_M=8   ISLAND_EXPANSIONS_MAX_PER_SIDE=4
ISLAND_EXPAND_USD_CENTS: 1=290 2=490 3=790 4=1290 (за один бік, CLASH burn; ADR-015); дозвіл тиру n використовується лише з рівня 5/10/16/22 (0019, `meta.level_min`) — гроші купують смугу, право на неї заробляється грою. З 0010 це предмети `island_expand_1..4` у каталозі (лише CLASH); n-те розширення боку списує дозвіл рівня n через `POST /island/expand`. Ділянок розширення не додає — лише землю (рішення 2026-09-06, ліміт грядок за рівнем)
STARTER_WHEAT_SEEDS=6  STARTER_CARROT_SEEDS=2  STARTER_CREDITS=30   # онбординг (міграція 0005): один цикл посадка → збір → покупка без буста
STARTER_RADISH_SEEDS=4   # 0027: навчальна культура — редиска 45 с (насіння 2 кр., продаж 3 кр.), перший урожай за хвилину; settings economy.starter_radish_seeds; кнопка посадки пропонує її першою, поки є
SEED_PRICE_CREDITS (0019, «ок» 2026-09-27; замок `meta.level_min` у дужках): wheat=5 barley=8 oats=10 onion=12 potato=14 carrot=16 beet=20 corn=26(L3) strawberry=32(L3) cabbage=38(L4) pepper=48(L5) tomato=55(L5) blueberry=75(L7) melon=85(L8) raspberry=90(L9) pumpkin=110(L10) watermelon=160(L12) clashberry=300(L14, замість репутації)
XP_PER_HARVEST=crop_credits/10; PLANT_XP=1 (0019); рівень n+1 з xp ≥ 120*n^1.7 (0019: L10=5 027, L20=19 540, L30=38 930, L40=63 486; таблиця цілих у `cf-world/levels.rs`)
# Ключі в `settings`: economy.plots_start, economy.plots_max_l1, economy.plots_max_l5, economy.harvest_max_per_day,
#   economy.plant_max_per_day, economy.credits_max_per_day, economy.starter_wheat_seeds (міграція 0002), economy.starter_credits, economy.starter_carrot_seeds (0005),
#   economy.plots_max_l1, economy.plots_max_l5, economy.bed_max_cells (0006); кеш 30 с
CROPS (продаж урожаю, 0019): wheat(30m, 10cr), barley(45m, 16), oats(1h, 21), onion(75m, 26), potato(90m, 30), carrot(2h, 38), beet(2.5h, 46), corn(3h, 62), strawberry(3h, 74), cabbage(4h, 90), pepper(5h, 115), tomato(6h, 130), blueberry(8h, 185), melon(9h, 210), raspberry(10h, 230), pumpkin(12h, 280), watermelon(16h, 415), clashberry(24h, 720) — продаж = 2.0–2.6 × ціна насіння; прибуток за годину плавно росте з часом росту (пшениця 10 → кавун 15.9 → clashberry 17.5): гравцю, що заходить рідко, вигідніші довгі; кукурудза — дешева 3-годинна, полуниця — дорога. Тест `crops_catalog.rs` тримає відношення, замки й те, що урожай не купується, а насіння не продається. Обґрунтування — `ECONOMY_LAB.md` §4.2
# 0008: збір дає 1 продукт (crop_*) в інвентар; «cr» вище — це meta.sell_credits продукту, кредити мінтяться лише продажем (/market/sell) під CREDITS_MAX_PER_DAY
DISHES (sell_credits / xp при з'їданні): bread=40/4 (3 wheat), salad=250/25 (2 carrot + tomato), pumpkin_soup=420/42 (pumpkin + 2 carrot), berry_pie=900/90 (clashberry + wheat)
DISHES v2 (0027, продаж ≈ інгредієнти +20–30 %, xp ≈ продаж/10): porridge=80/8 (3 oats), baked_potato=75/7 (2 potato), veggie_stew=120/12 (potato+carrot+onion), corn_bread=165/16 (2 corn+wheat), borscht=265/26 (2 beet+cabbage+potato), stuffed_pepper=470/45 (2 pepper+onion+tomato), blueberry_muffin=460/45 (2 blueberry+wheat), berry_jam=460/45 (2 strawberry+raspberry), fruit_salad=850/80 (melon+watermelon+strawberry); з ресурсів материка: herb_tea=60/10 (2 herb), mushroom_soup=160/20 (2 mushroom+onion+potato), mushroom_pie=180/25 (mushroom_rare+2 wheat). Гриби й трави не мають ціни продажу, тож ці страви — кран кредитів (не стік): обмежені денними капами збору материка (60, рідкісних 20), крафту (100) і кредитів за рівнем; пиріг тримаємо поруч із супом за гриб, щоб не було швидкої доріжки до капу для ботів
BUILD (декор з матеріалів, xp 5): fence = 10 wood + 4 stone, bench = 8 wood, lantern = 4 wood + 6 stone
BUILD v2 (0027): crate = 5 wood, firewood = 6 wood, chair = 3 wood_oak, table = 6 wood_oak, signpost = 6 wood_pine, bonfire = 4 wood + 8 stone, wheelbarrow = 6 wood + 2 ore_iron, well_windlass (велика ціль) = 12 stone + 6 wood_oak + 2 ore_iron; firewood/signpost/bonfire/well_windlass лише будуються
WEATHER_PERIOD_MIN=12  WEATHER_SEED=12345  # спільна погода: слоти по 12 хв, стан = хеш(слот, seed): 10% гроза, 18% дощ, 10% туман, 22% хмарно, 40% ясно; settings weather.period_min, weather.seed, weather.force
LIGHTNING_WOOD=3   LIGHTNING_MAX_PER_DAY=3   # гроза: одна блискавка на острів за слот, лише якщо гравець онлайн (клієнт просить), settings economy.lightning_wood, economy.lightning_max_per_day
RESTED_MINUTES=180   RESTED_BONUS_PCT=10   # 0026, «Відпочив» (власник 2026-10-01): сон ще й ставить статус на RESTED_MINUTES; поки він триває, КОЖЕН приріст XP від дій (посадка, збір, видобуток, крафт, їжа, експедиції, пам'ятки) збільшується на RESTED_BONUS_PCT % — точно: частки XP, що не склали цілого, переносяться в `islands.rested_carry` (цілі числа). XP самого сну не множиться. Відсоток копіюється на острів у момент сну, повторний сон перезапускає (не складає) статус; settings: economy.rested_minutes (0..1440), economy.rested_bonus_pct (0..100), 0 — вимкнено. Це лише XP: ні кредитів, ні CLASH, ні ледж.
SLEEP_MAX_PER_DAY=1   SLEEP_XP=10   # дім: сон раз на добу UTC дає XP; settings: economy.sleep_max_per_day, economy.sleep_xp
CRAFT_MAX_PER_DAY=100   EAT_MAX_PER_DAY=20   # settings: economy.craft_max_per_day, economy.eat_max_per_day; рецепти в таблиці recipes
FERTILIZER: basic(-25% часу, credits 15), premium(-50%, CLASH $0.90 за пакет ×10 (`meta.pack`), exclusive, ≤ PREMIUM_FERT_PER_DAY=12 застосувань на добу — 0019)
RENAME (0030, ADR-039, власник 2026-10-03): `rename_ticket` $5.00 (лише CLASH, burn 70 % + 30 % казначейству) — одна зміна ніка через `POST /me/nickname`; старе ім'я 30 днів за гравцем. Не продається на ринку, у вкладках магазину не показується (продає меню «Change name»).
RESTYLE (0029, ADR-038, власник 2026-10-03): `restyle_ticket` $0.99 (лише CLASH, burn 70 % + 30 % казначейству як усі CLASH-товари) — одна зміна вигляду через `POST /me/look`; нік не змінюється. Лише косметика: на ринку не продається (нема `meta.sell_credits`), у вкладках магазину не показується (продає меню «Change look»).
DECOR (0027, правило власника 2026-10-02: популярне й найпропрацьованіше — за CLASH, просте — за кредити; лише косметика): CLASH — farm_sign $1.90, well $2.90 (модель B136, була вітряним насосом), coop/pergola $3.90, gazebo/canopy $4.90, shed/tractor/water_tower $5.90, gazebo_large/lookout_tower $6.90, greenhouse $7.90, windmill $9.90 (footprint 50 дм); рівні 1–10 у meta.level_min. Кредити — 16 простих за 40–180 кр. (bucket, watering_can, fences, stool, milk_can, barrel_small, feeder, crate, haybale, chair, bench_stone, mailbox, lamp_short, wheelbarrow, table) + наявні fence/bench/barrel/lantern. Повний список і моделі — docs/CONTENT_V2.md
DECOR_CREDITS: fence=120 bench=200 barrel=150 lantern=260   DECOR_SLOTS=16 (0027; було 8 з 0009 — ліміт предметів декору на острові, ставляться будь-де)   SHOP_MAX_QTY=100
DECOR_FOOTPRINT_DM (items.meta.footprint_dm, радіус для перекриття): fence=14 bench=12 barrel=6 lantern=5 windmill=25 well=15, інші 10; поворот кроком 15°, прилипання 0.5 м у клієнті
HARVEST_CREDITS: сума за врожай іде в ледж (`system:mint` → `credits`) у тій самій транзакції; понад `CREDITS_MAX_PER_DAY` — врожай збирається, але платиться лише залишок дня
XP: harvest=crop_credits/10; рівень n потребує 100*n^1.6 xp

# Денні капи (на акаунт)
HARVEST_MAX_PER_DAY=40   PLANT_MAX_PER_DAY=40   GATHER_MAX_PER_DAY=60
CREDITS_CAP = CREDITS_CAP_BASE=500 + CREDITS_CAP_PER_LEVEL=150 × рівень острова (L1 650, L10 2 000, L30 5 000), не вище CREDITS_MAX_PER_DAY=10000 (0019; було 3 000 для всіх — впиралось на 2–3 день, casual = hardcore)
STIPEND_WHEAT=3   # 0019: острів без насіння, без посівів і з < 5 кр. отримує 3 пшениці раз на добу при `GET /island` (event `stipend`) — банкрутство неможливе

# Добування (0007): дерева → wood, каміння → stone; матеріали лише в інвентарі, не гроші
TREE_HITS=5   ROCK_HITS=8   TREE_RESPAWN_SECS=21600   ROCK_RESPAWN_SECS=14400   GATHER_XP=1   STARTER_TREES=3   STARTER_ROCKS=5
# settings: economy.gather_max_per_day, economy.tree_hits, economy.rock_hits, economy.tree_respawn_secs, economy.rock_respawn_secs, economy.gather_xp, economy.starter_trees, economy.starter_rocks

# Платежі
BURN_SHARE_BP=7000        # частка burn, решта → казначейство (ADR-017)
ORDER_MIN_USD_CENTS=50   QUOTE_TTL_SEC=90   PAY_CONFIRMED_MAX_USD_CENTS=2000
PRICE_CACHE_SEC=15       PRICE_MAX_DEVIATION_PCT=30   PRICE_STALE_SEC=60

# Стрік
STREAK_BONUS_PCT: d2=5 d3=10 d7=25 (до кредитів за збір), скидається після 48 год без входу

# Репутація v0
rep += 1 за день з ≥ 1 збором; rep += 10 за fulfilled order; rep -= 50 за флаг бота
Ексклюзивне насіння доступне з rep ≥ 20 (або 1 покупка)
```

## 4. Правила, що з них випливають

- Кап кредитів/день росте з рівнем (650 → 5 000), а рівень — лише з гри: стеля однакова для всіх, CLASH купує зручність, не стелю. Бот на L1 заробляє ≤ 650. Драбина будівель-цілей, індекс цін і ринок гравців — `ECONOMY_LAB.md` §4.3–4.6 (наступні кроки).
- Усі CLASH-товари — косметика, прискорення або ексклюзив, **не** підвищують заробіток кредитів більше ніж на 25% (щоб не було pay-to-earn-петлі).
- Сезон 8–12 тижнів: 5 нових CLASH-предметів, 1 ексклюзивна культура, лідерборд за xp (нагороди — косметика, не кредити).

## 5. Що міряємо з першого дня

`events`: `session_start`, `plant`, `harvest`, `order_quoted`, `order_fulfilled`, `order_rejected(reason)`, `decor_placed`, `level_up`, `streak_broken`. Без цього ROR і retention не порахувати. Це частина DoD Фази 1, не «потім».
