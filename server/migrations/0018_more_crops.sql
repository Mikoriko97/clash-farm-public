-- 0018: thirteen more crops (owner ok 2026-09-24, no level gate): seeds for credits in
-- the shop and their produce for the market. Numbers sit between the existing rungs
-- (docs/ECONOMY.md §3): sell = 2.0–2.6 × seed price; like the existing crops (wheat 10 cr/h
-- of profit … pumpkin 13.3), longer growth earns a little more per hour, for offline players.
-- Seed meta: grow_secs, credits (harvest XP = credits / 10), crop, produce (one unit per
-- plot); produce meta: sell_credits (minted only on sale, under the daily credits cap).
-- Placeholder models: client/blender/crops (PlotNode crop_stages already maps these ids).

INSERT INTO cf.items (id, kind, price_usd_cents, price_credits, exclusive, meta, tier) VALUES
    ('seed_barley',     'seed', NULL, 8,   false, '{"grow_secs": 2700,  "credits": 16,  "crop": "barley",     "produce": "crop_barley"}',     1),
    ('seed_oats',       'seed', NULL, 10,  false, '{"grow_secs": 3600,  "credits": 22,  "crop": "oats",       "produce": "crop_oats"}',       1),
    ('seed_onion',      'seed', NULL, 12,  false, '{"grow_secs": 4500,  "credits": 26,  "crop": "onion",      "produce": "crop_onion"}',      1),
    ('seed_potato',     'seed', NULL, 12,  false, '{"grow_secs": 5400,  "credits": 28,  "crop": "potato",     "produce": "crop_potato"}',     1),
    ('seed_beet',       'seed', NULL, 18,  false, '{"grow_secs": 9000,  "credits": 42,  "crop": "beet",       "produce": "crop_beet"}',       1),
    ('seed_corn',       'seed', NULL, 25,  false, '{"grow_secs": 10800, "credits": 60,  "crop": "corn",       "produce": "crop_corn"}',       1),
    ('seed_strawberry', 'seed', NULL, 30,  false, '{"grow_secs": 10800, "credits": 72,  "crop": "strawberry", "produce": "crop_strawberry"}', 1),
    ('seed_cabbage',    'seed', NULL, 35,  false, '{"grow_secs": 14400, "credits": 85,  "crop": "cabbage",    "produce": "crop_cabbage"}',    2),
    ('seed_pepper',     'seed', NULL, 45,  false, '{"grow_secs": 18000, "credits": 108, "crop": "pepper",     "produce": "crop_pepper"}',     2),
    ('seed_blueberry',  'seed', NULL, 70,  false, '{"grow_secs": 28800, "credits": 175, "crop": "blueberry",  "produce": "crop_blueberry"}',  2),
    ('seed_melon',      'seed', NULL, 80,  false, '{"grow_secs": 32400, "credits": 200, "crop": "melon",      "produce": "crop_melon"}',      2),
    ('seed_raspberry',  'seed', NULL, 85,  false, '{"grow_secs": 36000, "credits": 215, "crop": "raspberry",  "produce": "crop_raspberry"}',  2),
    ('seed_watermelon', 'seed', NULL, 150, false, '{"grow_secs": 57600, "credits": 390, "crop": "watermelon", "produce": "crop_watermelon"}', 2),
    ('crop_barley',     'produce', NULL, NULL, false, '{"sell_credits": 16,  "crop": "barley"}',     1),
    ('crop_oats',       'produce', NULL, NULL, false, '{"sell_credits": 22,  "crop": "oats"}',       1),
    ('crop_onion',      'produce', NULL, NULL, false, '{"sell_credits": 26,  "crop": "onion"}',      1),
    ('crop_potato',     'produce', NULL, NULL, false, '{"sell_credits": 28,  "crop": "potato"}',     1),
    ('crop_beet',       'produce', NULL, NULL, false, '{"sell_credits": 42,  "crop": "beet"}',       1),
    ('crop_corn',       'produce', NULL, NULL, false, '{"sell_credits": 60,  "crop": "corn"}',       1),
    ('crop_strawberry', 'produce', NULL, NULL, false, '{"sell_credits": 72,  "crop": "strawberry"}', 1),
    ('crop_cabbage',    'produce', NULL, NULL, false, '{"sell_credits": 85,  "crop": "cabbage"}',    2),
    ('crop_pepper',     'produce', NULL, NULL, false, '{"sell_credits": 108, "crop": "pepper"}',     2),
    ('crop_blueberry',  'produce', NULL, NULL, false, '{"sell_credits": 175, "crop": "blueberry"}',  2),
    ('crop_melon',      'produce', NULL, NULL, false, '{"sell_credits": 200, "crop": "melon"}',      2),
    ('crop_raspberry',  'produce', NULL, NULL, false, '{"sell_credits": 215, "crop": "raspberry"}',  2),
    ('crop_watermelon', 'produce', NULL, NULL, false, '{"sell_credits": 390, "crop": "watermelon"}', 2)
ON CONFLICT (id) DO NOTHING;

-- Impact: 26 catalog rows inserted, no existing row touched (ON CONFLICT DO NOTHING keeps a
-- re-run and a hand-edited catalog safe). Runs in milliseconds regardless of player count;
-- no downtime, no lock beyond the inserts on cf.items.
