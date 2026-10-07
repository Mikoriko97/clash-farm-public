-- 0024: mainland nodes move when they come back (owner ok 2026-09-30: «материк переносити»).
-- A used-up tree, rock or ore vein vanishes; after its respawn time it stands on a different
-- free spot. The bake (shared/mainland_nodes.json) gives every node its home plus a few
-- reserved alternates; the node's position is `positions[respawns mod (1 + alternates)]`, so the
-- server and the client never disagree and no two nodes compete for a spot. `respawns` counts
-- depletions and is bumped in the same statement that empties the node.

ALTER TABLE cf.mainland_nodes
    ADD COLUMN IF NOT EXISTS respawns int NOT NULL DEFAULT 0 CHECK (respawns >= 0);

-- Impact: one column with a constant default on a small table (rows exist only for nodes somebody
-- has hit): metadata-only in Postgres 11+, instant, no downtime; existing rows read 0 (home spot).
