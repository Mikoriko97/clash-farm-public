-- 0007: gatherable resource nodes (docs/PLAN.md «Добування ресурсів v1», owner ok 2026-09-05).
-- Trees and rocks placed in the island scene get server state: hits left and a
-- respawn time. Positions stay in the scene (idx matches Island.tscn); only the
-- state lives here. Wood and stone are inventory items, never money.

CREATE TABLE cf.resource_nodes (
    id          uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    island_id   uuid        NOT NULL REFERENCES cf.islands(id) ON DELETE CASCADE,
    idx         smallint    NOT NULL CHECK (idx >= 0),
    kind        text        NOT NULL CHECK (kind IN ('tree', 'rock')),
    hits_left   int         NOT NULL CHECK (hits_left >= 0),
    -- Set when hits_left reaches 0; the node is back once now() passes it.
    respawn_at  timestamptz,
    UNIQUE (island_id, idx)
);
CREATE INDEX resource_nodes_island_id_idx ON cf.resource_nodes (island_id);

INSERT INTO cf.items (id, kind, price_usd_cents, price_credits, exclusive, meta) VALUES
    ('wood',  'misc', NULL, NULL, false, '{"source": "tree"}'),
    ('stone', 'misc', NULL, NULL, false, '{"source": "rock"}')
ON CONFLICT (id) DO NOTHING;

INSERT INTO cf.settings (key, value) VALUES
    ('economy.gather_max_per_day', '60'::jsonb),
    ('economy.tree_hits',          '5'::jsonb),
    ('economy.rock_hits',          '8'::jsonb),
    ('economy.tree_respawn_secs',  '21600'::jsonb),
    ('economy.rock_respawn_secs',  '14400'::jsonb),
    ('economy.gather_xp',          '1'::jsonb),
    ('economy.starter_trees',      '3'::jsonb),
    ('economy.starter_rocks',      '5'::jsonb)
ON CONFLICT (key) DO NOTHING;

-- Existing islands get the starting layout of Island.tscn: trees idx 0-2, rocks idx 3-7.
INSERT INTO cf.resource_nodes (island_id, idx, kind, hits_left)
SELECT i.id, g.idx,
       CASE WHEN g.idx < 3 THEN 'tree' ELSE 'rock' END,
       CASE WHEN g.idx < 3 THEN 5 ELSE 8 END
FROM cf.islands i CROSS JOIN generate_series(0, 7) AS g(idx)
ON CONFLICT (island_id, idx) DO NOTHING;

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'app') THEN
        GRANT SELECT, INSERT, UPDATE ON cf.resource_nodes TO app;
    END IF;
END $$;

-- Impact: new table, 8 rows per existing island (≤ 1 s per 100k islands), two
-- catalog rows, eight settings. No locks on player tables beyond the inserts;
-- no downtime. Old servers ignore the table.
