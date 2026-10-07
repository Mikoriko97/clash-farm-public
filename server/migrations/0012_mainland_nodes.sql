-- 0012: mainland shared resource nodes (docs/MAINLAND.md §5, owner ok 2026-09-08).
-- Node positions and kinds are a pure function of the world seed (cf_world::mainland);
-- only the state of nodes somebody has hit lives here, keyed by the generated id
-- ("cx_cz_i"). Rows appear on the first hit and stay; a node with respawn_at in the
-- past is full again. Materials are inventory items with a tier (MAINLAND §6).

CREATE TABLE cf.mainland_nodes (
    id          text        PRIMARY KEY CHECK (id ~ '^[0-9]{1,2}_[0-9]{1,2}_[0-9]{1,3}$'),
    kind        text        NOT NULL,
    hits_left   int         NOT NULL CHECK (hits_left >= 0),
    -- Set when hits_left reaches 0; the node is back once now() passes it.
    respawn_at  timestamptz,
    updated_at  timestamptz NOT NULL DEFAULT now()
);

ALTER TABLE cf.items ADD COLUMN IF NOT EXISTS tier smallint NOT NULL DEFAULT 1 CHECK (tier BETWEEN 1 AND 4);

INSERT INTO cf.items (id, kind, price_usd_cents, price_credits, exclusive, meta, tier) VALUES
    ('wood_pine',     'misc', NULL, NULL, false, '{"source": "tree_pine"}',     2),
    ('wood_oak',      'misc', NULL, NULL, false, '{"source": "tree_oak"}',      2),
    ('ore_copper',    'misc', NULL, NULL, false, '{"source": "ore_copper"}',    2),
    ('ore_iron',      'misc', NULL, NULL, false, '{"source": "ore_iron"}',      3),
    ('herb',          'misc', NULL, NULL, false, '{"source": "herb"}',          2),
    ('mushroom',      'misc', NULL, NULL, false, '{"source": "mushroom"}',      2),
    ('mushroom_rare', 'misc', NULL, NULL, false, '{"source": "mushroom_rare"}', 3),
    ('reed',          'misc', NULL, NULL, false, '{"source": "reed"}',          1)
ON CONFLICT (id) DO NOTHING;

UPDATE cf.items SET tier = 2 WHERE id IN ('seed_tomato', 'seed_pumpkin') AND tier = 1;
UPDATE cf.items SET tier = 3 WHERE id = 'seed_clashberry' AND tier = 1;

INSERT INTO cf.settings (key, value) VALUES
    ('economy.mainland_hits_per_day',         '60'::jsonb),
    ('economy.mainland_rare_hits_per_day',    '20'::jsonb),
    ('economy.mainland_respawn_secs',         '1800'::jsonb),
    ('economy.mainland_rare_respawn_secs',    '3600'::jsonb),
    ('economy.mainland_gather_xp',            '2'::jsonb),
    -- How close (dm) the presence position must be to the node for a hit.
    ('economy.mainland_reach_dm',             '60'::jsonb)
ON CONFLICT (key) DO NOTHING;

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'app') THEN
        GRANT SELECT, INSERT, UPDATE ON cf.mainland_nodes TO app;
    END IF;
END $$;

-- Impact: one new empty table, one column with a default on the small items table
-- (rewrite ≤ 1 s), eight catalog rows, six settings. No player rows touched; no
-- downtime. Servers older than 0012 ignore all of it.
