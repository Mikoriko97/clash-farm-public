-- Local development / test database bootstrap (run once as the postgres superuser):
--   psql -U postgres -h 127.0.0.1 -f infra/db/init_local.sql
-- Roles mirror docs/DATA_MODEL.md §1. No passwords: local pg_hba uses `trust` on 127.0.0.1 only.
-- Never run this against staging/prod (RUNBOOK: roles there are created by the owner with passwords).

CREATE ROLE migrator LOGIN;
CREATE ROLE app LOGIN;
CREATE ROLE readonly LOGIN;

CREATE DATABASE cf_test OWNER migrator;
CREATE DATABASE cf_dev  OWNER migrator;

\connect cf_test
GRANT USAGE ON SCHEMA public TO app, readonly;
\connect cf_dev
GRANT USAGE ON SCHEMA public TO app, readonly;
