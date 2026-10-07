-- 0003: economy (docs/DATA_MODEL.md §2–3, ADR-006): append-only double-entry
-- ledger, materialised balances, orders/burns for CLASH payments (used from
-- week 4), audit_log, decor placements, credit-priced decor items.

-- Append-only guard --------------------------------------------------------
CREATE OR REPLACE FUNCTION cf.forbid_update_delete() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'table % is append-only (% not allowed)', TG_TABLE_NAME, TG_OP
        USING ERRCODE = 'integrity_constraint_violation';
END $$;

-- Ledger -------------------------------------------------------------------
-- user_id NULL = system account (system:mint, system:sink, system:burn_clash).
CREATE TABLE cf.ledger (
    id          bigserial   PRIMARY KEY,
    user_id     uuid        REFERENCES cf.users(id),
    account     text        NOT NULL,
    delta       bigint      NOT NULL CHECK (delta <> 0),
    currency    text        NOT NULL CHECK (currency IN ('CREDITS', 'XP', 'CLASH')),
    ref_type    text        NOT NULL,
    ref_id      uuid        NOT NULL,
    created_at  timestamptz NOT NULL DEFAULT now(),
    CHECK ((user_id IS NULL) = (account LIKE 'system:%'))
);
CREATE INDEX ledger_user_id_idx ON cf.ledger (user_id, created_at);
CREATE INDEX ledger_ref_id_idx ON cf.ledger (ref_id);
CREATE TRIGGER ledger_append_only BEFORE UPDATE OR DELETE ON cf.ledger
    FOR EACH ROW EXECUTE FUNCTION cf.forbid_update_delete();

-- Materialised balances, updated in the same transaction as the ledger rows.
CREATE TABLE cf.balances (
    user_id     uuid        NOT NULL REFERENCES cf.users(id) ON DELETE CASCADE,
    account     text        NOT NULL,
    amount      bigint      NOT NULL CHECK (amount >= 0),
    updated_at  timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, account)
);

-- Orders and burns (CLASH payments, SECURITY §4) -----------------------------
CREATE TABLE cf.orders (
    id               uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id          uuid        NOT NULL REFERENCES cf.users(id),
    item_id          text        NOT NULL REFERENCES cf.items(id),
    qty              int         NOT NULL CHECK (qty > 0),
    price_usd_cents  int         NOT NULL CHECK (price_usd_cents >= 0),
    clash_per_usd    numeric(30, 12) NOT NULL CHECK (clash_per_usd > 0),
    amount_clash     bigint      NOT NULL CHECK (amount_clash > 0),
    quote_expires_at timestamptz NOT NULL,
    status           text        NOT NULL DEFAULT 'quoted'
                     CHECK (status IN ('quoted', 'submitted', 'fulfilled', 'expired', 'rejected', 'needs_review')),
    reason_code      text,
    created_at       timestamptz NOT NULL DEFAULT now(),
    updated_at       timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX orders_user_id_idx ON cf.orders (user_id, created_at);
CREATE INDEX orders_status_idx ON cf.orders (status) WHERE status IN ('submitted', 'needs_review');

CREATE TABLE cf.burns (
    id           uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    order_id     uuid        NOT NULL UNIQUE REFERENCES cf.orders(id),
    signature    text        NOT NULL UNIQUE,
    slot         bigint,
    block_time   timestamptz,
    amount       bigint      NOT NULL CHECK (amount > 0),
    commitment   text        NOT NULL CHECK (commitment IN ('confirmed', 'finalized')),
    verified_at  timestamptz NOT NULL DEFAULT now(),
    raw_summary  jsonb       NOT NULL DEFAULT '{}'::jsonb
);
CREATE TRIGGER burns_append_only BEFORE UPDATE OR DELETE ON cf.burns
    FOR EACH ROW EXECUTE FUNCTION cf.forbid_update_delete();

-- Audit log (admin actions, SECURITY §8) --------------------------------------
CREATE TABLE cf.audit_log (
    id          bigserial   PRIMARY KEY,
    actor       text        NOT NULL,
    action      text        NOT NULL,
    target      text,
    before      jsonb,
    after       jsonb,
    ip_hash     bytea,
    created_at  timestamptz NOT NULL DEFAULT now()
);
CREATE TRIGGER audit_log_append_only BEFORE UPDATE OR DELETE ON cf.audit_log
    FOR EACH ROW EXECUTE FUNCTION cf.forbid_update_delete();

-- Decor ----------------------------------------------------------------------
CREATE TABLE cf.decor_placements (
    id         uuid      PRIMARY KEY DEFAULT gen_random_uuid(),
    island_id  uuid      NOT NULL REFERENCES cf.islands(id) ON DELETE CASCADE,
    item_id    text      NOT NULL REFERENCES cf.items(id),
    slot       smallint  NOT NULL CHECK (slot >= 0),
    placed_at  timestamptz NOT NULL DEFAULT now(),
    UNIQUE (island_id, slot)
);

-- Credit-priced decor (ECONOMY §3: 4 items). meta.piece = farm_pack scene name for Godot.
INSERT INTO cf.items (id, kind, price_usd_cents, price_credits, exclusive, meta) VALUES
    ('decor_fence',   'decor', NULL, 120, false, '{"piece": "fence_c"}'),
    ('decor_bench',   'decor', NULL, 200, false, '{"piece": "bench_a"}'),
    ('decor_barrel',  'decor', NULL, 150, false, '{"piece": "barrel_a"}'),
    ('decor_lantern', 'decor', NULL, 260, false, '{"piece": "streetlight_a"}');

INSERT INTO cf.settings (key, value) VALUES
    ('economy.decor_slots',  '8'::jsonb),
    ('economy.shop_max_qty', '100'::jsonb);

-- Grants: ledger/burns/audit_log are INSERT+SELECT only for app; triggers back it up.
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'app') THEN
        GRANT SELECT, INSERT ON cf.ledger, cf.burns, cf.audit_log TO app;
        GRANT SELECT, INSERT, UPDATE ON cf.balances, cf.orders, cf.decor_placements TO app;
        GRANT DELETE ON cf.decor_placements TO app;
        GRANT USAGE, SELECT ON SEQUENCE cf.ledger_id_seq, cf.audit_log_id_seq TO app;
    END IF;
END $$;

-- Impact: new tables + 4 catalog rows + 2 settings rows; no existing data touched.
-- Runtime on 1M rows: n/a (fresh). Downtime: none.
