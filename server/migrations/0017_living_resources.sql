-- 0017: living island resources (docs/PLAN.md етап 5, owner ok 2026-09-16).
-- Trees and rocks stop being scene-anchored: every node carries its own point
-- on the island (dm from the centre, like decor) and the moment its current
-- growth started. A used-up node regrows somewhere free (cf_world::regrow picks
-- the spot on the server, never the client), bought strips get their own share,
-- and a tree is gatherable only once grown. Starter nodes keep the exact spots
-- of Island.tscn so the base island still looks hand-made.

ALTER TABLE cf.resource_nodes
    ADD COLUMN x_dm     smallint,
    ADD COLUMN z_dm     smallint,
    -- When the current growth started; stage = age / economy.tree_grow_secs (rocks are whole at once).
    ADD COLUMN grown_at timestamptz NOT NULL DEFAULT now(),
    -- Visual variant for the client (which model of the kind), chosen by the server.
    ADD COLUMN variant  smallint NOT NULL DEFAULT 0 CHECK (variant >= 0 AND variant < 8);

-- Either both coordinates or neither; ±60 m covers the base island plus four strips a side.
ALTER TABLE cf.resource_nodes ADD CONSTRAINT resource_nodes_pos_check
    CHECK ((x_dm IS NULL) = (z_dm IS NULL) AND abs(coalesce(x_dm, 0)) <= 600 AND abs(coalesce(z_dm, 0)) <= 600);

-- Starter layout of Island.tscn (trees idx 0-2, rocks idx 3-7): the same points, in dm.
UPDATE cf.resource_nodes SET x_dm = v.x, z_dm = v.z, grown_at = now() - interval '30 days'
FROM (VALUES (0, -160, -140), (1, 170, 120), (2, 150, -160), (3, 120, -10), (4, -90, 110),
             (5, -180, 40), (6, 60, 170), (7, -50, -190)) AS v(idx, x, z)
WHERE cf.resource_nodes.idx = v.idx AND cf.resource_nodes.x_dm IS NULL;

INSERT INTO cf.settings (key, value) VALUES
    ('economy.tree_grow_secs', '3600'::jsonb),
    ('economy.strip_trees',    '1'::jsonb),
    ('economy.strip_rocks',    '1'::jsonb)
ON CONFLICT (key) DO NOTHING;

-- Impact: four nullable/defaulted columns and one CHECK on cf.resource_nodes
-- (8 rows per island, one UPDATE pass), three settings. No lock on player
-- tables beyond the ALTER (instant: defaults are constants); no downtime. Old
-- servers keep working: they ignore the new columns and never see NULL points
-- for the rows they created before (backfilled above).
