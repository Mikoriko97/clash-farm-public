-- 0008: produce, dishes, recipes, selling (docs/PLAN.md «Крафт v1», owner ok 2026-09-06).
-- A harvest now yields produce into the inventory instead of credits; credits come
-- from selling produce or dishes (same daily credits cap). Recipes turn produce and
-- materials into dishes (sell higher, or eat for XP) and into decor.

ALTER TABLE cf.items DROP CONSTRAINT items_kind_check;
ALTER TABLE cf.items ADD CONSTRAINT items_kind_check
    CHECK (kind IN ('seed', 'fertilizer', 'decor', 'misc', 'material', 'produce', 'dish'));
UPDATE cf.items SET kind = 'material' WHERE id IN ('wood', 'stone');

-- Produce: meta.sell_credits equals the pre-0008 harvest reward, so "sell right away"
-- pays exactly what a harvest paid before.
INSERT INTO cf.items (id, kind, price_usd_cents, price_credits, exclusive, meta) VALUES
    ('crop_wheat',      'produce', NULL, NULL, false, '{"sell_credits": 10,  "crop": "wheat"}'),
    ('crop_carrot',     'produce', NULL, NULL, false, '{"sell_credits": 35,  "crop": "carrot"}'),
    ('crop_tomato',     'produce', NULL, NULL, false, '{"sell_credits": 120, "crop": "tomato"}'),
    ('crop_pumpkin',    'produce', NULL, NULL, false, '{"sell_credits": 260, "crop": "pumpkin"}'),
    ('crop_clashberry', 'produce', NULL, NULL, false, '{"sell_credits": 700, "crop": "clashberry"}'),
    -- Dishes: sell above the sum of their inputs, or eat for meta.xp.
    ('dish_bread',        'dish', NULL, NULL, false, '{"sell_credits": 40,  "xp": 4}'),
    ('dish_salad',        'dish', NULL, NULL, false, '{"sell_credits": 250, "xp": 25}'),
    ('dish_pumpkin_soup', 'dish', NULL, NULL, false, '{"sell_credits": 420, "xp": 42}'),
    ('dish_berry_pie',    'dish', NULL, NULL, false, '{"sell_credits": 900, "xp": 90}')
ON CONFLICT (id) DO NOTHING;

-- Which produce a seed yields (one unit per plot).
UPDATE cf.items SET meta = meta || '{"produce": "crop_wheat"}'::jsonb      WHERE id = 'seed_wheat';
UPDATE cf.items SET meta = meta || '{"produce": "crop_carrot"}'::jsonb     WHERE id = 'seed_carrot';
UPDATE cf.items SET meta = meta || '{"produce": "crop_tomato"}'::jsonb     WHERE id = 'seed_tomato';
UPDATE cf.items SET meta = meta || '{"produce": "crop_pumpkin"}'::jsonb    WHERE id = 'seed_pumpkin';
UPDATE cf.items SET meta = meta || '{"produce": "crop_clashberry"}'::jsonb WHERE id = 'seed_clashberry';

-- Recipes: inputs = {item_id: qty}; admin-managed, read-only for the server.
CREATE TABLE cf.recipes (
    id           text    PRIMARY KEY,
    output_item  text    NOT NULL REFERENCES cf.items(id),
    output_qty   int     NOT NULL DEFAULT 1 CHECK (output_qty > 0),
    inputs       jsonb   NOT NULL,
    xp           int     NOT NULL DEFAULT 0 CHECK (xp >= 0),
    enabled      boolean NOT NULL DEFAULT true
);
INSERT INTO cf.recipes (id, output_item, output_qty, inputs, xp) VALUES
    ('bread',        'dish_bread',        1, '{"crop_wheat": 3}',                   3),
    ('salad',        'dish_salad',        1, '{"crop_carrot": 2, "crop_tomato": 1}', 10),
    ('pumpkin_soup', 'dish_pumpkin_soup', 1, '{"crop_pumpkin": 1, "crop_carrot": 2}', 20),
    ('berry_pie',    'dish_berry_pie',    1, '{"crop_clashberry": 1, "crop_wheat": 1}', 40),
    -- Building from materials: the same decor the shop sells, made instead of bought.
    ('fence',        'decor_fence',       1, '{"wood": 10, "stone": 4}',            5),
    ('bench',        'decor_bench',       1, '{"wood": 8}',                          5),
    ('lantern',      'decor_lantern',     1, '{"wood": 4, "stone": 6}',             5)
ON CONFLICT (id) DO NOTHING;

INSERT INTO cf.settings (key, value) VALUES
    ('economy.craft_max_per_day', '100'::jsonb),
    ('economy.eat_max_per_day',   '20'::jsonb)
ON CONFLICT (key) DO NOTHING;

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'app') THEN
        GRANT SELECT ON cf.recipes TO app;
    END IF;
END $$;

-- Impact: constraint swap on cf.items (small table, instant), nine catalog rows,
-- five meta updates, new read-only table with seven rows. No player-table locks,
-- no downtime. Servers older than 0008 keep paying harvests in credits until
-- redeployed; deploy the server together with this migration.
