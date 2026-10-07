-- 0026: the «Rested» status after sleeping (owner ok 2026-10-01: «статус Відпочив» as the purpose of sleep).
-- Sleeping in the house (once per UTC day, 0008/house) now also rests the island's owner for
-- `economy.rested_minutes`; while `rested_until` is in the future every XP gain from actions is raised by
-- `rested_bonus_pct` percent. The percent is copied here at sleep time (a later settings change does not
-- touch a running status) and the sub-XP remainder is carried in `rested_carry` (0..99 hundredths of an XP), so
-- the bonus is exact on average even when single gains are 1-7 XP. XP is not money: no ledger rows, no credits, no CLASH.

ALTER TABLE cf.islands
    ADD COLUMN IF NOT EXISTS rested_until timestamptz,
    ADD COLUMN IF NOT EXISTS rested_bonus_pct smallint NOT NULL DEFAULT 0 CHECK (rested_bonus_pct BETWEEN 0 AND 100),
    ADD COLUMN IF NOT EXISTS rested_carry smallint NOT NULL DEFAULT 0 CHECK (rested_carry BETWEEN 0 AND 99);

-- Impact: three columns on cf.islands (one row per player). The columns are added without a table rewrite (constant defaults,
-- nullable `rested_until`: metadata-only in Postgres 11+); the CHECK constraints are validated against the existing rows
-- (a read-only scan, well under a second at 1M rows) while ACCESS EXCLUSIVE is held briefly; no downtime is needed. `app` already
-- has table-level UPDATE (0002). Roll out the migration BEFORE the new binary: the new code selects the columns, the old binary
-- ignores them. Emergency stop of a running bonus (settings only stop NEW statuses; a running one lasts at most 24 h):
--   UPDATE cf.islands SET rested_until = NULL WHERE rested_until > now();   -- run by the operator (migrator role), see RUNBOOK
