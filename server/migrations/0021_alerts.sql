-- 0021: alerts (ADR-023, owner ok 2026-09-28). The `alerts` job in cf-api evaluates its
-- rules every minute and keeps one open row per rule here (opened → seen → resolved); the
-- admin reads, acknowledges and mutes them, every action audited. Telegram delivery is the
-- job's side effect, never a page view's.

CREATE TABLE cf.alerts (
    id           bigserial   PRIMARY KEY,
    rule         text        NOT NULL CHECK (char_length(rule) <= 64),
    level        text        NOT NULL CHECK (level IN ('warn', 'crit')),
    title        text        NOT NULL CHECK (char_length(title) <= 200),
    detail       jsonb       NOT NULL DEFAULT '{}'::jsonb,
    opened_at    timestamptz NOT NULL DEFAULT now(),
    last_seen_at timestamptz NOT NULL DEFAULT now(),
    resolved_at  timestamptz,
    -- last delivery to the operator (open, or the six-hourly reminder of an unread crit)
    notified_at  timestamptz,
    acked_by     text,
    acked_at     timestamptz,
    ack_note     text        CHECK (ack_note IS NULL OR char_length(ack_note) <= 200)
);
-- one open row per rule; the job upserts against this
CREATE UNIQUE INDEX alerts_open_rule_idx ON cf.alerts (rule) WHERE resolved_at IS NULL;
CREATE INDEX alerts_opened_at_idx ON cf.alerts (opened_at DESC);

-- a muted rule still opens rows, but nothing is sent until `until`
CREATE TABLE cf.alert_mutes (
    rule       text        PRIMARY KEY CHECK (char_length(rule) <= 64),
    until      timestamptz NOT NULL,
    muted_by   text        NOT NULL,
    note       text        CHECK (note IS NULL OR char_length(note) <= 200),
    created_at timestamptz NOT NULL DEFAULT now()
);

INSERT INTO cf.settings (key, value) VALUES
    ('alerts.review_queue_max',     '5'::jsonb),   -- needs_review orders before a signal …
    ('alerts.review_age_max_min',   '60'::jsonb),  -- … or the oldest one waiting this long
    ('alerts.switch_off_max_min',   '30'::jsonb),  -- a kill switch flipped for this long
    ('alerts.signups_per_hour_max', '200'::jsonb)  -- new wallets per hour (bot wave)
ON CONFLICT (key) DO NOTHING;

-- the rules read audit_log by action/actor and users by created_at every minute
CREATE INDEX audit_log_action_created_at_idx ON cf.audit_log (action, created_at);
CREATE INDEX audit_log_actor_created_at_idx ON cf.audit_log (actor, created_at);
CREATE INDEX users_created_at_idx ON cf.users (created_at);

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'app') THEN
        GRANT SELECT, INSERT, UPDATE ON cf.alerts TO app;
        GRANT SELECT, INSERT, UPDATE, DELETE ON cf.alert_mutes TO app;
        GRANT USAGE, SELECT ON SEQUENCE cf.alerts_id_seq TO app;
    END IF;
END $$;

-- Impact: two new tables and four settings rows; three indexes on existing tables
-- (audit_log, users: seconds at 1M rows, writers wait for the build). No data changes,
-- no downtime.
