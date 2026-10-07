-- 0001: identity, sessions, kill switches (docs/DATA_MODEL.md §2, ADR-007, ADR-012).
-- Runs as role `migrator`. Role `app` gets DML only; DDL never.

CREATE SCHEMA IF NOT EXISTS cf;
-- No SET search_path: it would stick to the pooled connection and hide public._sqlx_migrations.

-- Identity ---------------------------------------------------------------
-- wallet: canonical identity `sol:<base58>` or `evm:<0x lowercase hex>` (ADR-012/013).
CREATE TABLE cf.users (
    id            uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    wallet        text        NOT NULL UNIQUE
                              CHECK (wallet ~ '^sol:[1-9A-HJ-NP-Za-km-z]{32,44}$'
                                  OR wallet ~ '^evm:0x[0-9a-f]{40}$'),
    nickname      text        CHECK (nickname IS NULL OR char_length(nickname) BETWEEN 2 AND 24),
    created_at    timestamptz NOT NULL DEFAULT now(),
    last_seen_at  timestamptz NOT NULL DEFAULT now(),
    reputation    int         NOT NULL DEFAULT 0,
    flags         jsonb       NOT NULL DEFAULT '{}'::jsonb
);

-- Sessions ---------------------------------------------------------------
-- One row per refresh token; `family` groups rotations so reuse of an old
-- token revokes the whole family (SECURITY §3).
CREATE TABLE cf.sessions (
    id            uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id       uuid        NOT NULL REFERENCES cf.users(id) ON DELETE CASCADE,
    family        uuid        NOT NULL,
    refresh_hash  bytea       NOT NULL UNIQUE,
    expires_at    timestamptz NOT NULL,
    revoked_at    timestamptz,
    created_at    timestamptz NOT NULL DEFAULT now(),
    ip_hash       bytea,
    ua_hash       bytea
);
CREATE INDEX sessions_user_id_idx ON cf.sessions (user_id);
CREATE INDEX sessions_family_idx  ON cf.sessions (family);
CREATE INDEX sessions_expires_at_idx ON cf.sessions (expires_at) WHERE revoked_at IS NULL;

-- Settings / kill switches ----------------------------------------------
CREATE TABLE cf.settings (
    key         text        PRIMARY KEY,
    value       jsonb       NOT NULL,
    updated_at  timestamptz NOT NULL DEFAULT now(),
    updated_by  text        NOT NULL DEFAULT 'migration'
);
INSERT INTO cf.settings (key, value) VALUES
    ('maintenance',           'false'::jsonb),
    ('shop_enabled',          'true'::jsonb),
    ('payments_enabled',      'true'::jsonb),
    ('registrations_enabled', 'true'::jsonb);

-- Grants (role `app` may not exist in local test DBs; skip silently) -----
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'app') THEN
        GRANT USAGE ON SCHEMA cf TO app;
        GRANT SELECT, INSERT, UPDATE ON cf.users, cf.sessions, cf.settings TO app;
    END IF;
END $$;

-- Impact: new schema + 3 tables, no existing data. Runtime on 1M rows: n/a (fresh).
-- Downtime: none.
