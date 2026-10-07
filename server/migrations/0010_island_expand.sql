-- 0010: island expansion permits (ADR-015, owner ok 2026-09-06).
-- One permit per tier, bought for CLASH through the ordinary order flow; the
-- n-th expansion of a side consumes the tier-n permit (POST /island/expand).
-- Expansion counters already live in cf.islands.expand_* (0002, max 4 per side).

INSERT INTO cf.items (id, kind, price_usd_cents, price_credits, exclusive, meta) VALUES
    ('island_expand_1', 'misc', 290,  NULL, false, '{"tier": 1, "strip_m": 8}'),
    ('island_expand_2', 'misc', 490,  NULL, false, '{"tier": 2, "strip_m": 8}'),
    ('island_expand_3', 'misc', 790,  NULL, false, '{"tier": 3, "strip_m": 8}'),
    ('island_expand_4', 'misc', 1290, NULL, false, '{"tier": 4, "strip_m": 8}')
ON CONFLICT (id) DO NOTHING;

-- Impact: four catalog rows, no schema change, no downtime.
