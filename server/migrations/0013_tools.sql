-- 0013: tools with durability (docs/MAINLAND.md §5 П7, owner ok 2026-09-08).
-- An axe or a pickaxe is an ordinary stackable inventory item; the wear of the
-- one in use is one row per player and tool. At `economy.tool_durability` hits
-- the tool breaks (one unit leaves the stack, wear resets). With the right tool
-- a mainland hit yields two units, without it one. Repair costs materials
-- (items.meta.repair) and resets wear. Crafted from mainland materials.

ALTER TABLE cf.items DROP CONSTRAINT items_kind_check;
ALTER TABLE cf.items ADD CONSTRAINT items_kind_check
    CHECK (kind IN ('seed', 'fertilizer', 'decor', 'misc', 'material', 'produce', 'dish', 'tool'));
UPDATE cf.items SET kind = 'material'
    WHERE kind = 'misc'
      AND id IN ('wood_pine', 'wood_oak', 'ore_copper', 'ore_iron', 'herb', 'mushroom', 'mushroom_rare', 'reed');

INSERT INTO cf.items (id, kind, price_usd_cents, price_credits, exclusive, meta, tier) VALUES
    ('tool_axe',     'tool', NULL, NULL, false, '{"for": "tree", "repair": {"wood_pine": 2}}',  2),
    ('tool_pickaxe', 'tool', NULL, NULL, false, '{"for": "rock", "repair": {"ore_copper": 2}}', 2)
ON CONFLICT (id) DO NOTHING;

CREATE TABLE cf.tool_wear (
    user_id     uuid        NOT NULL REFERENCES cf.users(id) ON DELETE CASCADE,
    item_id     text        NOT NULL REFERENCES cf.items(id),
    wear        int         NOT NULL DEFAULT 0 CHECK (wear >= 0),
    updated_at  timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, item_id)
);

INSERT INTO cf.recipes (id, output_item, output_qty, inputs, xp) VALUES
    ('axe',     'tool_axe',     1, '{"wood_pine": 4, "ore_copper": 2}', 8),
    ('pickaxe', 'tool_pickaxe', 1, '{"wood_oak": 3, "ore_copper": 3}',  8)
ON CONFLICT (id) DO NOTHING;

INSERT INTO cf.settings (key, value) VALUES
    ('economy.tool_durability', '100'::jsonb)
ON CONFLICT (key) DO NOTHING;

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'app') THEN
        GRANT SELECT, INSERT, UPDATE ON cf.tool_wear TO app;
    END IF;
END $$;

-- Impact: constraint swap on the small items table, eight rows re-kinded, two items,
-- one empty table, two recipes, one setting. No player rows touched; no downtime.
