-- 0031: nickname moderation (ADR-040, owner ok 2026-10-03). An admin resets an offending name:
-- the player gets a placeholder `Farmer_<8 hex>` (look kept) and one free rename, the old name
-- is blocked for everybody — lookalikes too, by `nickname_key` — until an admin lifts the block.
-- Admins can also block a name ahead of time. A block is never deleted, only lifted (once): the
-- table is the moderation record next to cf.audit_log. A moderation reset is a row of
-- cf.nickname_history with kind 'moderation'; the free rename lasts while it is the player's
-- latest history row (derived, no ticket, no ledger entry).

CREATE TABLE cf.nickname_blocks (
    id          bigserial   PRIMARY KEY,
    key         text        NOT NULL CHECK (char_length(key) BETWEEN 1 AND 32),
    nickname    text        NOT NULL CHECK (char_length(nickname) BETWEEN 1 AND 32),
    reason      text        NOT NULL CHECK (char_length(reason) BETWEEN 1 AND 200),
    blocked_by  text        NOT NULL,
    created_at  timestamptz NOT NULL DEFAULT now(),
    lifted_at   timestamptz,
    lifted_by   text,
    CONSTRAINT nickname_blocks_lift_together CHECK ((lifted_at IS NULL) = (lifted_by IS NULL))
);
-- One live block per key ("is this name blocked?" is an index lookup); lifted rows stay as history.
CREATE UNIQUE INDEX nickname_blocks_live_key_uidx ON cf.nickname_blocks (key) WHERE lifted_at IS NULL;
CREATE INDEX nickname_blocks_created_idx ON cf.nickname_blocks (created_at DESC);

-- No DELETE; an UPDATE may only lift a live block (lifted_at/lifted_by), nothing else changes.
CREATE OR REPLACE FUNCTION cf.nickname_block_lift_only() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'table % is lift-only (DELETE not allowed)', TG_TABLE_NAME
            USING ERRCODE = 'integrity_constraint_violation';
    END IF;
    IF OLD.lifted_at IS NOT NULL OR NEW.lifted_at IS NULL
       OR (NEW.id, NEW.key, NEW.nickname, NEW.reason, NEW.blocked_by, NEW.created_at)
          IS DISTINCT FROM (OLD.id, OLD.key, OLD.nickname, OLD.reason, OLD.blocked_by, OLD.created_at) THEN
        RAISE EXCEPTION 'table % is lift-only (a live block can be lifted once)', TG_TABLE_NAME
            USING ERRCODE = 'integrity_constraint_violation';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER nickname_blocks_lift_only BEFORE UPDATE OR DELETE ON cf.nickname_blocks
    FOR EACH ROW EXECUTE FUNCTION cf.nickname_block_lift_only();

-- What changed the name: the player's paid rename, or a moderator's reset.
ALTER TABLE cf.nickname_history
    ADD COLUMN kind text NOT NULL DEFAULT 'rename'
        CONSTRAINT nickname_history_kind CHECK (kind IN ('rename', 'moderation'));

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'app') THEN
        REVOKE UPDATE, DELETE, TRUNCATE ON cf.nickname_blocks FROM app;
        GRANT SELECT, INSERT ON cf.nickname_blocks TO app;
        GRANT UPDATE (lifted_at, lifted_by) ON cf.nickname_blocks TO app;
        GRANT USAGE, SELECT ON SEQUENCE cf.nickname_blocks_id_seq TO app;
    END IF;
END $$;

-- Impact: a new empty table with two indexes and a trigger; one column on cf.nickname_history
-- with a constant default (no rewrite; the CHECK scans the table once — milliseconds, it holds
-- only renames since 0030). No downtime. Roll out before the new binary (the old one never reads
-- either). Rolling the binary back below 0031 silently stops enforcing the blocks and the free
-- rename (old code ignores both); the placeholders stay valid names.
