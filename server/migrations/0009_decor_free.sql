-- 0009: decor placed anywhere on the island with a rotation (owner ok 2026-09-06).
-- Fixed editor slots go away: a placement is a point in decimetres from the island
-- centre plus a yaw in 15° steps. Existing placements move to the coordinates of
-- the slots they occupied in DecorSlots.tscn. `slot` stays nullable for one release.

ALTER TABLE cf.decor_placements
    ADD COLUMN x_dm    smallint,
    ADD COLUMN z_dm    smallint,
    ADD COLUMN yaw_deg smallint NOT NULL DEFAULT 0;

UPDATE cf.decor_placements SET (x_dm, z_dm) = (
    CASE slot WHEN 0 THEN -80 WHEN 1 THEN -80 WHEN 2 THEN 80 WHEN 3 THEN 80
              WHEN 4 THEN -40 WHEN 5 THEN 40 WHEN 6 THEN 110 ELSE 0 END,
    CASE slot WHEN 0 THEN -10 WHEN 1 THEN 20 WHEN 2 THEN -40 WHEN 3 THEN 0
              WHEN 4 THEN 80 WHEN 5 THEN 80 WHEN 6 THEN -90 ELSE 120 END)
WHERE x_dm IS NULL;

ALTER TABLE cf.decor_placements
    ALTER COLUMN x_dm SET NOT NULL,
    ALTER COLUMN z_dm SET NOT NULL,
    ALTER COLUMN slot DROP NOT NULL,
    DROP CONSTRAINT decor_placements_island_id_slot_key,
    ADD CONSTRAINT decor_placements_point_range CHECK (x_dm BETWEEN -560 AND 560 AND z_dm BETWEEN -560 AND 560),
    ADD CONSTRAINT decor_placements_yaw CHECK (yaw_deg >= 0 AND yaw_deg < 360 AND yaw_deg % 15 = 0);
CREATE INDEX IF NOT EXISTS decor_placements_island_id_idx ON cf.decor_placements (island_id);

-- Footprint radius (dm) used for overlap checks; unknown decor gets 10 dm.
UPDATE cf.items SET meta = meta || '{"footprint_dm": 10}'::jsonb WHERE kind = 'decor' AND NOT (meta ? 'footprint_dm');
UPDATE cf.items SET meta = meta || '{"footprint_dm": 14}'::jsonb WHERE id = 'decor_fence';
UPDATE cf.items SET meta = meta || '{"footprint_dm": 12}'::jsonb WHERE id = 'decor_bench';
UPDATE cf.items SET meta = meta || '{"footprint_dm": 6}'::jsonb  WHERE id = 'decor_barrel';
UPDATE cf.items SET meta = meta || '{"footprint_dm": 5}'::jsonb  WHERE id = 'decor_lantern';
UPDATE cf.items SET meta = meta || '{"footprint_dm": 25}'::jsonb WHERE id = 'decor_windmill';
UPDATE cf.items SET meta = meta || '{"footprint_dm": 15}'::jsonb WHERE id = 'decor_well';

-- Impact: three columns on a small table, backfill by slot, one unique index dropped
-- and two CHECKs added; catalog meta touched for decor rows only. No downtime.
-- Servers older than 0009 would insert rows without x_dm: deploy together.
