-- 0019: economy v2 (docs/ECONOMY_LAB.md, owner ok 2026-09-27). Numbers only: the daily
-- credits cap grows with the island level, plots unlock every two levels up to a maximum,
-- extra plots cost more each, planting gives XP, the premium fertilizer is sold as a pack
-- and capped per day, long crops and island expansions need a level, and a broke island
-- gets a few wheat seeds a day. Code: cf-world params/plots/shop/market/expand/stipend.

INSERT INTO cf.settings (key, value) VALUES
    ('economy.credits_cap_base',      '500'::jsonb),   -- credits/day at level 0 …
    ('economy.credits_cap_per_level', '150'::jsonb),   -- … + this per level (L1 650, L10 2000, L30 5000)
    ('economy.plots_per_levels',      '2'::jsonb),     -- one more plot every N levels …
    ('economy.plots_max',             '20'::jsonb),    -- … up to this many
    ('economy.plot_price_step',       '100'::jsonb),   -- each plot beyond the starters costs this much more
    ('economy.plant_xp',              '1'::jsonb),
    ('economy.premium_fert_per_day',  '12'::jsonb),
    ('economy.stipend_wheat',         '3'::jsonb)      -- seeds a day for an island with nothing to plant
ON CONFLICT (key) DO NOTHING;
-- The old absolute cap stays as a ceiling the level formula can never exceed.
UPDATE cf.settings SET value = '10000'::jsonb WHERE key = 'economy.credits_max_per_day';

-- Seeds: sale = 2.0–2.6 × seed, profit per hour rising gently with grow time (10 → 17.5);
-- level_min gates the crops from three hours up so the first two weeks keep unlocking.
UPDATE cf.items SET price_credits = v.price, meta = meta || jsonb_build_object('credits', v.sell) || CASE WHEN v.lvl > 1 THEN jsonb_build_object('level_min', v.lvl) ELSE '{}'::jsonb END
FROM (VALUES
    ('seed_wheat',      5,   10,  1), ('seed_barley',    8,   16,  1), ('seed_oats',      10,  21,  1),
    ('seed_onion',      12,  26,  1), ('seed_potato',    14,  30,  1), ('seed_carrot',    16,  38,  1),
    ('seed_beet',       20,  46,  1), ('seed_corn',      26,  62,  3), ('seed_strawberry', 32, 74,  3),
    ('seed_cabbage',    38,  90,  4), ('seed_pepper',    48,  115, 5), ('seed_tomato',    55,  130, 5),
    ('seed_blueberry',  75,  185, 7), ('seed_melon',     85,  210, 8), ('seed_raspberry', 90,  230, 9),
    ('seed_pumpkin',    110, 280, 10), ('seed_watermelon', 160, 415, 12), ('seed_clashberry', 300, 720, 14)
) AS v(id, price, sell, lvl)
WHERE items.id = v.id;

UPDATE cf.items SET meta = meta || jsonb_build_object('sell_credits', v.sell)
FROM (VALUES
    ('crop_oats', 21), ('crop_potato', 30), ('crop_carrot', 38), ('crop_beet', 46), ('crop_corn', 62),
    ('crop_strawberry', 74), ('crop_cabbage', 90), ('crop_pepper', 115), ('crop_tomato', 130),
    ('crop_blueberry', 185), ('crop_melon', 210), ('crop_raspberry', 230), ('crop_pumpkin', 280),
    ('crop_watermelon', 415), ('crop_clashberry', 720)
) AS v(id, sell)
WHERE items.id = v.id;

-- Clashberry: a level instead of the reputation gate (reputation is not earned anywhere yet).
UPDATE cf.items SET exclusive = false WHERE id = 'seed_clashberry';

-- Premium fertilizer: one $0.90 order delivers a pack of ten; uses are capped per day.
UPDATE cf.items SET meta = meta || '{"pack": 10, "premium": true}'::jsonb WHERE id = 'fert_premium';

-- Island expansion permits: money buys the strip, the right to buy it is earned by playing.
UPDATE cf.items SET meta = meta || jsonb_build_object('level_min', v.lvl)
FROM (VALUES ('island_expand_1', 5), ('island_expand_2', 10), ('island_expand_3', 16), ('island_expand_4', 22)) AS v(id, lvl)
WHERE items.id = v.id;

-- Impact: settings inserts and in-place updates of 38 catalog rows; no schema change, no
-- table rewrite, milliseconds. Existing islands keep their XP; the level is re-derived from
-- the new curve (levels.rs) on the next XP gain, so a test island may drop a level or two.
