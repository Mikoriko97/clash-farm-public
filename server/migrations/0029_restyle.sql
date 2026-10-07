-- 0029: paid look change after the character exists (ADR-038, owner ok 2026-10-03, $0.99).
-- One catalog item bought for CLASH through the ordinary verified order flow (no payment code
-- changes); POST /me/look spends one ticket and rewrites the look in the same transaction. The
-- nickname stays (ADR-025). No meta.sell_credits: the NPC market refuses it, so CLASH never turns
-- into credits through a ticket. Hidden from the regular shop tabs; the menu sells it.

INSERT INTO cf.items (id, kind, price_usd_cents, price_credits, exclusive, meta) VALUES
    ('restyle_ticket', 'misc', 99, NULL, false, '{"service": "restyle"}')
ON CONFLICT (id) DO NOTHING;

-- Impact: one catalog row, no schema change, no downtime.
