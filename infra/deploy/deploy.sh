#!/usr/bin/env bash
# Деплой з машини розробника на VPS (docs/RUNBOOK.md §3, без Docker/CI поки що):
#   SERVER=<user>@<host> bash infra/deploy/deploy.sh
# 1) web і адмін-застосунок збираються тут (dist містить і Godot-паки з web/public/godot),
# 2) джерела сервера їдуть як `git archive HEAD` (лише закомічене) через tar|ssh|tar (rsync на
#    Windows нема) у /opt/clash-farm/src,
# 3) на сервері `cargo build --release` під користувачем clash, бінарники в /opt/clash-farm/bin,
# 4) сервіси перезапускаються; міграції НЕ запускаються (migrate.sh — власник).
# Ідемпотентний; помилка на будь-якому кроці зупиняє все (set -e).
set -euo pipefail

SERVER="${SERVER:?SERVER=user@host}"
SSH_OPTS="${SSH_OPTS:--o StrictHostKeyChecking=accept-new}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
APP_DIR=/opt/clash-farm
STAMP="$(git -C "$ROOT" rev-parse --short HEAD)"

# Мок-режим для перегляду дизайну (`?mock=1`, docs/UI.md): за замовчуванням ВИМКНЕНО (fail-closed), на staging вмикається явно:
#   VITE_MOCK_API=1 SERVER=root@<ip> bash infra/deploy/deploy.sh
export VITE_MOCK_API="${VITE_MOCK_API:-0}"
echo "== build web ($STAMP, mock mode: $VITE_MOCK_API)"
# Не --silent: на Windows dev-сервер Vite тримає node_modules, і `npm ci` падає — має бути видно чому.
(cd "$ROOT/web" && npm ci --ignore-scripts && npm run build --silent && npm run build:admin --silent) || { echo "web build failed (is a Vite dev server still running?)"; exit 1; }
# Страховка: збірка без прапорця не має містити коду моку (інакше prod віддавав би фейковий API за ?mock=1).
[ -d "$ROOT/web/dist/assets" ] || { echo "web/dist/assets missing: the web build produced nothing"; exit 1; }
if [ "$VITE_MOCK_API" != "1" ] && grep -rlq "mock_not_found" "$ROOT/web/dist/assets"; then echo "mock code leaked into a build without VITE_MOCK_API"; exit 1; fi
[ -f "$ROOT/web/dist/godot/index.pck" ] || { echo "web/public/godot/index.pck missing: export the Godot packs first (godot-verify skill)"; exit 1; }

echo "== ship sources (git archive HEAD) and builds"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
git -C "$ROOT" archive --format=tar HEAD server shared infra docs/API.md | tar -x -C "$TMP"
echo "$STAMP" > "$TMP/BUILD"
# Заміна теки цілком (як rsync --delete): розпакувати поруч, потім атомарно підмінити.
ship() { # ship <local dir> <remote dir>
  # shellcheck disable=SC2086
  tar -C "$1" -cf - . | ssh $SSH_OPTS "$SERVER" "set -e; rm -rf '$2.new'; mkdir -p '$2.new'; tar -xf - -C '$2.new'; chown -R clash:clash '$2.new'; chmod 750 '$2.new'; rm -rf '$2'; mv '$2.new' '$2'"
}
ship "$TMP" "$APP_DIR/src"
ship "$ROOT/web/dist" "$APP_DIR/web"
ship "$ROOT/web/dist-admin" "$APP_DIR/admin"

echo "== build server binaries on the box (release; first time takes a while)"
# shellcheck disable=SC2086
ssh $SSH_OPTS "$SERVER" bash -s <<'REMOTE'
set -euo pipefail
sudo -u clash -H bash -c '
  set -euo pipefail
  export PATH="$HOME/.cargo/bin:$PATH"
  cd /opt/clash-farm/src/server
  cargo build --release -p cf-api -p cf-admin 2>&1 | tail -3
  install -m 755 target/release/cf-api /opt/clash-farm/bin/cf-api.new
  install -m 755 target/release/cf-admin /opt/clash-farm/bin/cf-admin.new
  mv -f /opt/clash-farm/bin/cf-api.new /opt/clash-farm/bin/cf-api
  mv -f /opt/clash-farm/bin/cf-admin.new /opt/clash-farm/bin/cf-admin
'
chmod 750 /opt/clash-farm/web /opt/clash-farm/admin
systemctl restart cf-api cf-admin
sleep 3
systemctl --no-pager --lines=0 status cf-api cf-admin | grep -E "Active:" || true
echo "healthz: $(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:8080/healthz)  readyz: $(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:8080/readyz)  admin: $(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:9100/)"
REMOTE
echo "== deployed $STAMP. If migrations are pending: on the box, sudo bash /opt/clash-farm/src/infra/deploy/migrate.sh"
