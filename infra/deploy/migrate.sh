#!/usr/bin/env bash
# Міграції на staging/prod запускає власник (docs/RUNBOOK.md §3, rules/migrations.md):
#   sudo bash /opt/clash-farm/src/infra/deploy/migrate.sh
# Роль migrator (пароль у /etc/clash-farm/migrate.env, 600 root). Лише вперед; перед prod — бекап.
# Використовує psql і файли server/migrations/*.sql у порядку номерів, веде власну таблицю
# `_deploy_migrations` (sqlx-cli на сервері не потрібен).
set -euo pipefail
[ "$(id -u)" -eq 0 ] || { echo "run as root"; exit 1; }
# shellcheck disable=SC1091
. /etc/clash-farm/migrate.env
export PGPASSWORD="${DATABASE_URL##*:}"; PGPASSWORD="${PGPASSWORD%%@*}"
DIR=/opt/clash-farm/src/server/migrations
psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -q -c "CREATE TABLE IF NOT EXISTS _deploy_migrations (version text PRIMARY KEY, applied_at timestamptz NOT NULL DEFAULT now());"
for f in "$DIR"/*.sql; do
  v="$(basename "$f" .sql)"
  if psql "$DATABASE_URL" -tAc "SELECT 1 FROM _deploy_migrations WHERE version = '$v'" | grep -q 1; then
    continue
  fi
  echo "== applying $v"
  psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -q -1 -f "$f"
  psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -q -c "INSERT INTO _deploy_migrations (version) VALUES ('$v');"
done
echo "== migrations up to date"
