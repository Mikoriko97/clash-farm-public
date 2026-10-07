-- 0011: background jobs (week 5, docs/RUNBOOK.md §Jobs).
-- burns is append-only, so the confirmed → finalized verdict lives in its own
-- append-only table; retention of events/rate_events needs DELETE for role app.
-- Impact: one new table, two grants; no existing rows touched.

CREATE TABLE cf.burn_finality (
    order_id     uuid        PRIMARY KEY REFERENCES cf.orders(id),
    signature    text        NOT NULL,
    outcome      text        NOT NULL CHECK (outcome IN ('finalized', 'missing')),
    slot         bigint,
    checked_at   timestamptz NOT NULL DEFAULT now()
);
CREATE TRIGGER burn_finality_append_only BEFORE UPDATE OR DELETE ON cf.burn_finality
    FOR EACH ROW EXECUTE FUNCTION cf.forbid_update_delete();

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'app') THEN
        GRANT SELECT, INSERT ON cf.burn_finality TO app;
        -- Retention job (docs/DATA_MODEL.md §retention): events 90 days, rate_events 30 days.
        GRANT DELETE ON cf.events, cf.rate_events TO app;
    END IF;
END $$;
