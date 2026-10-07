-- 0006: plots placed by the player (docs/PLAN.md, owner decision 2026-09-05).
-- Every plot gets a cell on the island grid (3 m pitch, cell centre = col*3, row*3
-- relative to the island centre; bounds grow with ADR-015 expansions). Plots that
-- touch by a side form a bed the client acts on as one; the bed size limit lives
-- in settings. A plot is an inventory item bought for credits.

ALTER TABLE cf.plots
    ADD COLUMN col smallint,
    ADD COLUMN row smallint;

-- Backfill the starting layout: two rows of three, north of the island centre,
-- matching the pre-0006 PlotGrid scene (x = -3, 0, 3; z = -9, -6).
UPDATE cf.plots SET col = (idx % 3) - 1, row = (idx / 3) - 3 WHERE col IS NULL;

ALTER TABLE cf.plots
    ALTER COLUMN col SET NOT NULL,
    ALTER COLUMN row SET NOT NULL,
    ADD CONSTRAINT plots_cell_range CHECK (col BETWEEN -16 AND 16 AND row BETWEEN -16 AND 16),
    ADD CONSTRAINT plots_cell_unique UNIQUE (island_id, col, row);

-- Plot as a purchasable item (credits). meta.cell_m documents the pitch for clients.
INSERT INTO cf.items (id, kind, price_usd_cents, price_credits, exclusive, meta) VALUES
    ('plot', 'misc', NULL, 200, false, '{"cell_m": 3}')
ON CONFLICT (id) DO NOTHING;

INSERT INTO cf.settings (key, value) VALUES
    ('economy.plots_max_l1',  '6'::jsonb),
    ('economy.plots_max_l5',  '12'::jsonb),
    ('economy.bed_max_cells', '9'::jsonb)
ON CONFLICT (key) DO NOTHING;

-- Removing an empty plot deletes its row (the item goes back to the inventory).
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'app') THEN
        GRANT DELETE ON cf.plots TO app;
    END IF;
END $$;

-- Impact: adds two NOT NULL columns to cf.plots with a backfill from idx
-- (≤ 12 rows per island). On 1M rows the UPDATE + constraint build takes ~10 s
-- and holds an ACCESS EXCLUSIVE lock on cf.plots: run at low traffic. No downtime
-- otherwise; old servers keep working (they ignore the new columns).
