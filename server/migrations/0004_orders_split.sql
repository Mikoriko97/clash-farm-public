-- 0004: ADR-017 — an order's CLASH amount is split into a burn part and a
-- treasury transfer part, both fixed at quote time so a later change of
-- economy.burn_share_bp never re-prices an open order.

ALTER TABLE cf.orders
    ADD COLUMN burn_amount     bigint NOT NULL DEFAULT 0 CHECK (burn_amount >= 0),
    ADD COLUMN transfer_amount bigint NOT NULL DEFAULT 0 CHECK (transfer_amount >= 0),
    ADD COLUMN wallet          text   NOT NULL DEFAULT '' ,
    ADD CONSTRAINT orders_split_sums CHECK (burn_amount + transfer_amount = amount_clash);

ALTER TABLE cf.burns
    ADD COLUMN treasury_amount bigint NOT NULL DEFAULT 0 CHECK (treasury_amount >= 0);

INSERT INTO cf.settings (key, value) VALUES
    ('economy.burn_share_bp',            '7000'::jsonb),
    ('economy.order_min_usd_cents',      '50'::jsonb),
    ('economy.quote_ttl_sec',            '90'::jsonb),
    ('economy.pay_confirmed_max_usd_cents', '2000'::jsonb);

-- CLASH-priced catalog entries (ECONOMY §3; exclusive = CLASH only).
UPDATE cf.items SET price_usd_cents = 90, price_credits = NULL WHERE id = 'fert_premium';
INSERT INTO cf.items (id, kind, price_usd_cents, price_credits, exclusive, meta) VALUES
    ('decor_windmill', 'decor', 990, NULL, true, '{"piece": "farmstructure_a"}'),
    ('decor_well',     'decor', 290, NULL, true, '{"piece": "brick_b"}');

-- Impact: adds columns with defaults (rewrite of orders/burns — both empty), 4 settings rows, 2 catalog rows.
-- Runtime on 1M rows: ~seconds (DEFAULT is non-volatile, no table rewrite in PG16). Downtime: none.
