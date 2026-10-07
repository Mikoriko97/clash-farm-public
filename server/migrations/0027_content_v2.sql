-- 0027: content v2 from the models the project already has (docs/CONTENT_V2.md; owner ok 2026-10-02).
-- Owner's rule: only what is seen is sold; popular, detailed pieces for CLASH (USD price, 70 % burn),
-- cheap and simple ones for credits; part of the simple decor is also built from wood, stone and ore.
-- Twelve dishes from the existing crops and mainland goods (sale ≈ inputs + 20–30 %, xp ≈ sale / 10,
-- like the four dishes of 0008), and a tutorial crop (radish, 45 s) so the first harvest comes in a minute.
-- Fixes: the paid «well» showed a 15 m wind pump and the windmill reserved 2.5 m for a 10 m model.
-- Decor meta: piece (the model the client shows, documentation only), footprint_dm (radius for overlap),
-- level_min (CLASH decor, checked by orders). No ledger, balance or order row is touched.

INSERT INTO cf.items (id, kind, price_usd_cents, price_credits, exclusive, meta, tier) VALUES
    -- Decor for credits: simple pieces (models F/B in docs/models/catalog.tsv).
    ('decor_bucket',        'decor', NULL, 40,  false, '{"piece": "F014", "footprint_dm": 3}', 1),
    ('decor_watering_can',  'decor', NULL, 40,  false, '{"piece": "F144", "footprint_dm": 3}', 1),
    ('decor_fence_cross',   'decor', NULL, 50,  false, '{"piece": "F054", "footprint_dm": 6}', 1),
    ('decor_fence_picket',  'decor', NULL, 60,  false, '{"piece": "F064", "footprint_dm": 7}', 1),
    ('decor_stool',         'decor', NULL, 70,  false, '{"piece": "F023", "footprint_dm": 3}', 1),
    ('decor_milk_can',      'decor', NULL, 80,  false, '{"piece": "F098", "footprint_dm": 3}', 1),
    ('decor_barrel_small',  'decor', NULL, 90,  false, '{"piece": "F002", "footprint_dm": 3}', 1),
    ('decor_feeder',        'decor', NULL, 90,  false, '{"piece": "F071", "footprint_dm": 5}', 1),
    ('decor_crate',         'decor', NULL, 110, false, '{"piece": "F027", "footprint_dm": 7}', 1),
    ('decor_haybale',       'decor', NULL, 120, false, '{"piece": "F115", "footprint_dm": 7}', 1),
    ('decor_chair',         'decor', NULL, 140, false, '{"piece": "F022", "footprint_dm": 5}', 1),
    ('decor_bench_stone',   'decor', NULL, 150, false, '{"piece": "F005", "footprint_dm": 7}', 1),
    ('decor_mailbox',       'decor', NULL, 150, false, '{"piece": "F097", "footprint_dm": 3}', 1),
    ('decor_lamp_short',    'decor', NULL, 160, false, '{"piece": "B188", "footprint_dm": 3}', 1),
    ('decor_wheelbarrow',   'decor', NULL, 170, false, '{"piece": "B166", "footprint_dm": 8}', 1),
    ('decor_table',         'decor', NULL, 180, false, '{"piece": "B192", "footprint_dm": 6}', 1),
    -- Decor that is only built (recipes below), never sold.
    ('decor_firewood',      'decor', NULL, NULL, false, '{"piece": "B142", "footprint_dm": 6}', 1),
    ('decor_signpost',      'decor', NULL, NULL, false, '{"piece": "B148", "footprint_dm": 7}', 1),
    ('decor_bonfire',       'decor', NULL, NULL, false, '{"piece": "B157", "footprint_dm": 8}', 1),
    ('decor_well_windlass', 'decor', NULL, NULL, false, '{"piece": "B137", "footprint_dm": 17}', 2),
    -- Decor for CLASH: the popular, most detailed pieces; cosmetic only, the level is earned by playing.
    ('decor_farm_sign',     'decor', 190, NULL, true, '{"piece": "F041", "footprint_dm": 15}', 1),
    ('decor_coop',          'decor', 390, NULL, true, '{"piece": "F024", "footprint_dm": 15, "level_min": 3}', 2),
    ('decor_pergola',       'decor', 390, NULL, true, '{"piece": "F122", "footprint_dm": 25, "level_min": 4}', 2),
    ('decor_gazebo',        'decor', 490, NULL, true, '{"piece": "B025", "footprint_dm": 22, "level_min": 4}', 2),
    ('decor_canopy',        'decor', 490, NULL, true, '{"piece": "F118", "footprint_dm": 32, "level_min": 5}', 2),
    ('decor_shed',          'decor', 590, NULL, true, '{"piece": "F094", "footprint_dm": 19, "level_min": 5}', 2),
    ('decor_tractor',       'decor', 590, NULL, true, '{"piece": "F128", "footprint_dm": 24, "level_min": 6}', 3),
    ('decor_water_tower',   'decor', 590, NULL, true, '{"piece": "B030", "footprint_dm": 15, "level_min": 7}', 3),
    ('decor_gazebo_large',  'decor', 690, NULL, true, '{"piece": "B026", "footprint_dm": 40, "level_min": 8}', 3),
    ('decor_lookout_tower', 'decor', 690, NULL, true, '{"piece": "B022", "footprint_dm": 27, "level_min": 9}', 3),
    ('decor_greenhouse',    'decor', 790, NULL, true, '{"piece": "F082", "footprint_dm": 36, "level_min": 10}', 3),
    -- Dishes: eaten for XP or sold (meta.sell_credits, minted only on sale under the daily cap).
    ('dish_porridge',         'dish', NULL, NULL, false, '{"xp": 8,  "sell_credits": 80}',  1),
    ('dish_baked_potato',     'dish', NULL, NULL, false, '{"xp": 7,  "sell_credits": 75}',  1),
    ('dish_veggie_stew',      'dish', NULL, NULL, false, '{"xp": 12, "sell_credits": 120}', 1),
    ('dish_herb_tea',         'dish', NULL, NULL, false, '{"xp": 10, "sell_credits": 60}',  1),
    ('dish_corn_bread',       'dish', NULL, NULL, false, '{"xp": 16, "sell_credits": 165}', 1),
    ('dish_mushroom_soup',    'dish', NULL, NULL, false, '{"xp": 20, "sell_credits": 160}', 2),
    ('dish_borscht',          'dish', NULL, NULL, false, '{"xp": 26, "sell_credits": 265}', 2),
    ('dish_stuffed_pepper',   'dish', NULL, NULL, false, '{"xp": 45, "sell_credits": 470}', 2),
    ('dish_blueberry_muffin', 'dish', NULL, NULL, false, '{"xp": 45, "sell_credits": 460}', 2),
    ('dish_berry_jam',        'dish', NULL, NULL, false, '{"xp": 45, "sell_credits": 460}', 2),
    ('dish_fruit_salad',      'dish', NULL, NULL, false, '{"xp": 80, "sell_credits": 850}', 3),
    ('dish_mushroom_pie',     'dish', NULL, NULL, false, '{"xp": 25, "sell_credits": 180}', 3),
    -- Tutorial crop: the first harvest within a minute; worth almost nothing, so not worth farming.
    ('seed_radish', 'seed',    NULL, 2,    false, '{"grow_secs": 45, "credits": 3, "crop": "radish", "produce": "crop_radish"}', 1),
    ('crop_radish', 'produce', NULL, NULL, false, '{"sell_credits": 3, "crop": "radish"}', 1)
ON CONFLICT (id) DO NOTHING;

INSERT INTO cf.recipes (id, output_item, output_qty, inputs, xp) VALUES
    -- Building from materials (same pieces the shop sells, or build-only ones).
    ('crate',          'decor_crate',          1, '{"wood": 5}',                                    3),
    ('firewood',       'decor_firewood',       1, '{"wood": 6}',                                    3),
    ('chair',          'decor_chair',          1, '{"wood_oak": 3}',                                4),
    ('table',          'decor_table',          1, '{"wood_oak": 6}',                                6),
    ('signpost',       'decor_signpost',       1, '{"wood_pine": 6}',                               6),
    ('bonfire',        'decor_bonfire',        1, '{"wood": 4, "stone": 8}',                        8),
    ('wheelbarrow',    'decor_wheelbarrow',    1, '{"wood": 6, "ore_iron": 2}',                     10),
    ('well_windlass',  'decor_well_windlass',  1, '{"stone": 12, "wood_oak": 6, "ore_iron": 2}',    25),
    -- Cooking.
    ('porridge',         'dish_porridge',         1, '{"crop_oats": 3}',                                        3),
    ('baked_potato',     'dish_baked_potato',     1, '{"crop_potato": 2}',                                      3),
    ('veggie_stew',      'dish_veggie_stew',      1, '{"crop_potato": 1, "crop_carrot": 1, "crop_onion": 1}',  5),
    ('herb_tea',         'dish_herb_tea',         1, '{"herb": 2}',                                             4),
    ('corn_bread',       'dish_corn_bread',       1, '{"crop_corn": 2, "crop_wheat": 1}',                       6),
    ('mushroom_soup',    'dish_mushroom_soup',    1, '{"mushroom": 2, "crop_onion": 1, "crop_potato": 1}',      8),
    ('borscht',          'dish_borscht',          1, '{"crop_beet": 2, "crop_cabbage": 1, "crop_potato": 1}',   10),
    ('stuffed_pepper',   'dish_stuffed_pepper',   1, '{"crop_pepper": 2, "crop_onion": 1, "crop_tomato": 1}',   15),
    ('blueberry_muffin', 'dish_blueberry_muffin', 1, '{"crop_blueberry": 2, "crop_wheat": 1}',                  15),
    ('berry_jam',        'dish_berry_jam',        1, '{"crop_strawberry": 2, "crop_raspberry": 1}',             15),
    ('fruit_salad',      'dish_fruit_salad',      1, '{"crop_melon": 1, "crop_watermelon": 1, "crop_strawberry": 1}', 25),
    ('mushroom_pie',     'dish_mushroom_pie',     1, '{"mushroom_rare": 1, "crop_wheat": 2}',                   10)
ON CONFLICT (id) DO NOTHING;

-- The paid well now shows the roofed well B136 (it was a 15 m wind pump), and both big pieces reserve
-- the room their models actually take. Only over the values of 0004/0009: a catalog edited by hand stays.
UPDATE cf.items SET meta = meta || '{"piece": "B136", "footprint_dm": 16}'::jsonb
    WHERE id = 'decor_well' AND meta->>'piece' = 'brick_b' AND meta->>'footprint_dm' = '15';
UPDATE cf.items SET meta = meta || '{"footprint_dm": 50}'::jsonb
    WHERE id = 'decor_windmill' AND meta->>'footprint_dm' = '25';

INSERT INTO cf.settings (key, value) VALUES
    ('economy.starter_radish_seeds', '4'::jsonb)   -- tutorial: plant, wait 45 s, harvest
ON CONFLICT (key) DO NOTHING;
-- With 47 decor pieces in the catalog, 8 on an island is too few to make it one's own.
UPDATE cf.settings SET value = '16'::jsonb WHERE key = 'economy.decor_slots' AND value = '8'::jsonb;

-- Mushroom pie: the rare mushroom has no price of its own (mainland only, shared nodes), so the pie is a
-- credit tap, not a sink; 180 keeps it next to the soup per mushroom and leaves no fast lane to the cap.
-- Impact: 45 item and 20 recipe inserts (ON CONFLICT DO NOTHING: a re-run or a hand-edited catalog is
-- safe), two meta updates and two settings rows; no schema change, milliseconds, no downtime. Existing
-- wells keep their placement; on the next snapshot the client shows the new model. A windmill already
-- placed keeps its spot; only neighbours placed later must keep 5 m away.
