-- 0020: admin panel stage 2 (ADR-022, owner ok 2026-09-28). A player can be banned from
-- the admin: sign-in and refresh refuse a banned wallet, every live session is revoked at
-- ban time. The catalog becomes editable from the admin (prices, gates, enabled), so the
-- app role needs UPDATE on the columns the admin may change — never on `id`/`kind`.

ALTER TABLE cf.users
    ADD COLUMN IF NOT EXISTS banned_at  timestamptz,
    ADD COLUMN IF NOT EXISTS ban_reason text CHECK (ban_reason IS NULL OR char_length(ban_reason) <= 200);

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'app') THEN
        GRANT UPDATE (price_usd_cents, price_credits, enabled, exclusive, tier, meta) ON cf.items TO app;
    END IF;
END $$;

-- Analytics reads events by name and day (docs/ECONOMY.md §5): the admin's funnel and
-- top-crops queries walk the last 30 days by name.
CREATE INDEX IF NOT EXISTS events_name_created_at_idx ON cf.events (name, created_at);

-- Impact: two nullable columns on cf.users (no rewrite), one column grant, one index
-- (built in seconds at this size; on a large table it would need CONCURRENTLY outside a
-- transaction). No data changes.
