-- 0030: paid nickname change (ADR-039, owner ok 2026-10-03: $5, the old name held 30 days).
-- One catalog item bought for CLASH through the ordinary verified order flow (no payment code
-- changes); POST /me/nickname spends one ticket, rewrites users.nickname/nickname_key and appends
-- a row here in the same transaction. The history is append-only: moderation reads it (admin
-- player card), and a name given up within the last 30 days stays with its former owner — nobody
-- else can claim it (character creation or another rename), so a fresh account cannot pose as a
-- player who just renamed. No meta.sell_credits: the market refuses it.

INSERT INTO cf.items (id, kind, price_usd_cents, price_credits, exclusive, meta) VALUES
    ('rename_ticket', 'misc', 500, NULL, false, '{"service": "rename"}')
ON CONFLICT (id) DO NOTHING;

-- Kill switch for POST /me/nickname (a wave of abusive renames: tickets already bought stop working too).
INSERT INTO cf.settings (key, value) VALUES ('renames_enabled', 'true'::jsonb)
ON CONFLICT (key) DO NOTHING;

CREATE TABLE cf.nickname_history (
    id            bigserial   PRIMARY KEY,
    user_id       uuid        NOT NULL REFERENCES cf.users(id),
    old_nickname  text        NOT NULL,
    old_key       text        NOT NULL,
    new_nickname  text        NOT NULL,
    new_key       text        NOT NULL,
    changed_at    timestamptz NOT NULL DEFAULT now()
);
-- "Is this name held by someone else?" — by key, newest first; and a player's own history.
CREATE INDEX nickname_history_old_key_idx ON cf.nickname_history (old_key, changed_at DESC);
CREATE INDEX nickname_history_user_idx ON cf.nickname_history (user_id, changed_at DESC);
CREATE TRIGGER nickname_history_append_only BEFORE UPDATE OR DELETE ON cf.nickname_history
    FOR EACH ROW EXECUTE FUNCTION cf.forbid_update_delete();

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'app') THEN
        REVOKE UPDATE, DELETE, TRUNCATE ON cf.nickname_history FROM app;
        GRANT SELECT, INSERT ON cf.nickname_history TO app;
        GRANT USAGE, SELECT ON SEQUENCE cf.nickname_history_id_seq TO app;
    END IF;
END $$;

-- Impact: one catalog row, one settings row and a new empty table with two indexes; no rewrite of
-- existing data, instant at any size (no scan of cf.users), no downtime. Roll out before the new
-- binary (the old one never reads it). Rolling the binary back to before 0030 after the first
-- renames silently drops the 30-day hold (the old creation path does not read the history).
