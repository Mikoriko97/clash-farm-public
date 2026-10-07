-- 0016: cave crystals and storm-felled trees (docs/MAINLAND.md §3–4, owner ok 2026-09-09).
-- The cave is zone "cave" with fixed nodes (cf_world::cave, ids 99_<cave>_<i>);
-- a mainland storm fells trees for a while (cf_world::storm, ids 98_<slot>_<i>).
-- Both keep their live state in cf.mainland_nodes, so the id check widens to
-- eight digits in the middle for the weather slot number.

ALTER TABLE cf.mainland_nodes DROP CONSTRAINT IF EXISTS mainland_nodes_id_check;
ALTER TABLE cf.mainland_nodes ADD CONSTRAINT mainland_nodes_id_check
    CHECK (id ~ '^[0-9]{1,2}_[0-9]{1,8}_[0-9]{1,3}$');

INSERT INTO cf.items (id, kind, price_usd_cents, price_credits, exclusive, meta, tier) VALUES
    ('crystal', 'material', NULL, NULL, false, '{"source": "crystal"}', 4)
ON CONFLICT (id) DO NOTHING;

INSERT INTO cf.settings (key, value) VALUES
    -- How long felled trees stay after a storm slot began.
    ('economy.storm_fallen_secs', '1800'::jsonb)
ON CONFLICT (key) DO NOTHING;

-- Impact: constraint swap on a small table, one catalog row, one setting; no player rows touched.
