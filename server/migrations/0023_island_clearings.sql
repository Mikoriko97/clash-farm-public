-- 0023: cleared wild bushes (owner ok 2026-09-30). The island's wild flora (bushes, grass,
-- flowers) is scattered by the client from a fixed seed, so the server knows nothing about
-- individual bushes. A player clears one with their hands (no tool, no cost, no reward):
-- the server only remembers *where*, and every client keeps the flora away from those spots.
-- Positions sit on a 0.5 m grid (5 dm) so a repeat click is the same row; the per-island cap
-- lives in code (world::clearings). Cosmetic and space-making only: nothing in the economy,
-- ledger or inventory reads this table.

CREATE TABLE cf.island_clearings (
    island_id  uuid        NOT NULL REFERENCES cf.islands(id) ON DELETE CASCADE,
    x_dm       smallint    NOT NULL,
    z_dm       smallint    NOT NULL,
    cleared_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (island_id, x_dm, z_dm),
    CHECK (x_dm % 5 = 0 AND z_dm % 5 = 0)
);

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'app') THEN
        GRANT SELECT, INSERT ON cf.island_clearings TO app;
    END IF;
END $$;

-- Impact: one new table, no change to existing data; instant, no downtime. Rows are tiny
-- (≤ 400 per island by the API), so 1M islands would be ≈ 400M rows worst case — well past
-- the alpha; the primary key (island_id, x, z) serves the only query (list by island).
