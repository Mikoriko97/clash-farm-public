-- 0002: world (docs/DATA_MODEL.md §2, docs/ECONOMY.md §3, ADR-015).
-- islands, plots, inventory, item catalog with seed data, daily caps, events,
-- economy parameters in settings. Ledger/balances/orders come in 0003.

-- Catalog (admin-managed, server-read) --------------------------------------
CREATE TABLE cf.items (
    id               text    PRIMARY KEY,
    kind             text    NOT NULL CHECK (kind IN ('seed', 'fertilizer', 'decor', 'misc')),
    price_usd_cents  int     CHECK (price_usd_cents IS NULL OR price_usd_cents >= 0),
    price_credits    bigint  CHECK (price_credits IS NULL OR price_credits >= 0),
    exclusive        boolean NOT NULL DEFAULT false,
    enabled          boolean NOT NULL DEFAULT true,
    meta             jsonb   NOT NULL DEFAULT '{}'::jsonb
);

-- Seeds: meta.grow_secs, meta.credits (harvest reward, ledger in 0003), meta.xp.
INSERT INTO cf.items (id, kind, price_usd_cents, price_credits, exclusive, meta) VALUES
    ('seed_wheat',      'seed', NULL, 5,    false, '{"grow_secs": 1800,  "credits": 10,  "crop": "wheat"}'),
    ('seed_carrot',     'seed', NULL, 15,   false, '{"grow_secs": 7200,  "credits": 35,  "crop": "carrot"}'),
    ('seed_tomato',     'seed', NULL, 50,   false, '{"grow_secs": 21600, "credits": 120, "crop": "tomato"}'),
    ('seed_pumpkin',    'seed', NULL, 100,  false, '{"grow_secs": 43200, "credits": 260, "crop": "pumpkin"}'),
    ('seed_clashberry', 'seed', NULL, 300,  true,  '{"grow_secs": 86400, "credits": 700, "crop": "clashberry"}'),
    -- Fertilizers: meta.speedup_bp = share of the remaining time removed, in basis points.
    ('fert_basic',      'fertilizer', NULL, 15, false, '{"speedup_bp": 2500}'),
    ('fert_premium',    'fertilizer', 90,   NULL, true, '{"speedup_bp": 5000}');

-- World -----------------------------------------------------------------------
CREATE TABLE cf.islands (
    id          uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     uuid        NOT NULL UNIQUE REFERENCES cf.users(id) ON DELETE CASCADE,
    level       int         NOT NULL DEFAULT 1 CHECK (level >= 1),
    xp          bigint      NOT NULL DEFAULT 0 CHECK (xp >= 0),
    expand_n    smallint    NOT NULL DEFAULT 0 CHECK (expand_n BETWEEN 0 AND 4),
    expand_e    smallint    NOT NULL DEFAULT 0 CHECK (expand_e BETWEEN 0 AND 4),
    expand_s    smallint    NOT NULL DEFAULT 0 CHECK (expand_s BETWEEN 0 AND 4),
    expand_w    smallint    NOT NULL DEFAULT 0 CHECK (expand_w BETWEEN 0 AND 4),
    created_at  timestamptz NOT NULL DEFAULT now()
);

-- state is stored as empty|growing; "ready" is derived (growing AND ready_at <= now()).
CREATE TABLE cf.plots (
    id             uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    island_id      uuid        NOT NULL REFERENCES cf.islands(id) ON DELETE CASCADE,
    idx            smallint    NOT NULL CHECK (idx >= 0),
    state          text        NOT NULL DEFAULT 'empty' CHECK (state IN ('empty', 'growing')),
    crop_id        text        REFERENCES cf.items(id),
    planted_at     timestamptz,
    ready_at       timestamptz,
    fertilizer_id  text        REFERENCES cf.items(id),
    UNIQUE (island_id, idx),
    CHECK ((state = 'empty' AND crop_id IS NULL AND ready_at IS NULL)
        OR (state = 'growing' AND crop_id IS NOT NULL AND planted_at IS NOT NULL AND ready_at IS NOT NULL))
);
CREATE INDEX plots_island_id_idx ON cf.plots (island_id);

CREATE TABLE cf.inventory (
    user_id  uuid   NOT NULL REFERENCES cf.users(id) ON DELETE CASCADE,
    item_id  text   NOT NULL REFERENCES cf.items(id),
    qty      int    NOT NULL CHECK (qty >= 0),
    PRIMARY KEY (user_id, item_id)
);

-- Daily caps per account, one row per (user, action, day). Atomic upsert in code.
CREATE TABLE cf.rate_events (
    user_id  uuid  NOT NULL REFERENCES cf.users(id) ON DELETE CASCADE,
    action   text  NOT NULL,
    day      date  NOT NULL,
    count    int   NOT NULL DEFAULT 0 CHECK (count >= 0),
    PRIMARY KEY (user_id, action, day)
);

-- Product events (ECONOMY §5). Retention 90 days (job later).
CREATE TABLE cf.events (
    id          bigserial   PRIMARY KEY,
    user_id     uuid        REFERENCES cf.users(id) ON DELETE SET NULL,
    name        text        NOT NULL,
    props       jsonb       NOT NULL DEFAULT '{}'::jsonb,
    created_at  timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX events_created_at_idx ON cf.events (created_at);
CREATE INDEX events_user_id_idx ON cf.events (user_id);

-- Economy parameters (ECONOMY §3), editable via admin, read with a 30 s cache.
INSERT INTO cf.settings (key, value) VALUES
    ('economy.plots_start',          '6'::jsonb),
    ('economy.plots_max_l1',         '6'::jsonb),
    ('economy.plots_max_l5',         '12'::jsonb),
    ('economy.harvest_max_per_day',  '40'::jsonb),
    ('economy.plant_max_per_day',    '40'::jsonb),
    ('economy.credits_max_per_day',  '3000'::jsonb),
    ('economy.starter_wheat_seeds',  '6'::jsonb),
    ('economy.exclusive_min_reputation', '20'::jsonb);

-- Grants -----------------------------------------------------------------------
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'app') THEN
        GRANT SELECT ON cf.items TO app;
        GRANT SELECT, INSERT, UPDATE ON cf.islands, cf.plots, cf.inventory, cf.rate_events TO app;
        GRANT SELECT, INSERT ON cf.events TO app;
        GRANT USAGE, SELECT ON SEQUENCE cf.events_id_seq TO app;
    END IF;
END $$;

-- Impact: new tables + 7 catalog rows + 7 settings rows, no existing data touched.
-- Runtime on 1M rows: n/a (fresh). Downtime: none.
