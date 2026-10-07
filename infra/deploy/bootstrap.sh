#!/usr/bin/env bash
# Одноразова підготовка чистого Ubuntu 22.04/24.04 під Clash Farm (docs/RUNBOOK.md §3, staging
# на одному VPS без Docker). Ідемпотентний: повторний запуск нічого не ламає.
#   sudo DOMAIN=stg.example.com ADMIN_WALLETS=sol:...,sol:... bash bootstrap.sh
# Ставить: Caddy (TLS сам), PostgreSQL 16, Redis, rustup для користувача clash, ufw, fail2ban,
# unattended-upgrades. Секрети генерує на місці у /etc/clash-farm/*.env (root:clash 640) і
# ніколи не друкує. Міграції НЕ запускає (RUNBOOK: staging/prod — власник, `migrate.sh`).
set -euo pipefail

DOMAIN="${DOMAIN:?DOMAIN=<host for TLS and SIWS>, e.g. stg.example.com or 203-0-113-10.sslip.io}"
ADMIN_WALLETS="${ADMIN_WALLETS:?ADMIN_WALLETS=sol:<base58>[,sol:<base58>]}"
CF_ENV="${CF_ENV:-staging}"
APP_DIR=/opt/clash-farm
ETC_DIR=/etc/clash-farm
CLASH_USER=clash

[ "$(id -u)" -eq 0 ] || { echo "run as root"; exit 1; }
export DEBIAN_FRONTEND=noninteractive

echo "== packages"
apt-get update -q
apt-get install -y -q curl ca-certificates gnupg git rsync build-essential pkg-config \
  ufw fail2ban unattended-upgrades postgresql redis-server debian-keyring debian-archive-keyring apt-transport-https
if ! command -v caddy >/dev/null; then
  curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' | gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
  curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' > /etc/apt/sources.list.d/caddy-stable.list
  apt-get update -q && apt-get install -y -q caddy
fi

echo "== user and directories"
id "$CLASH_USER" >/dev/null 2>&1 || useradd --system --create-home --home-dir "$APP_DIR" --shell /usr/sbin/nologin "$CLASH_USER"
install -d -o "$CLASH_USER" -g "$CLASH_USER" -m 750 "$APP_DIR" "$APP_DIR/src" "$APP_DIR/bin" "$APP_DIR/web" "$APP_DIR/admin"
install -d -o root -g "$CLASH_USER" -m 750 "$ETC_DIR"
# Caddy читає статику як користувач caddy.
usermod -a -G "$CLASH_USER" caddy || true
chmod 750 "$APP_DIR"

echo "== rust toolchain for $CLASH_USER (stable, needed for cargo build --release on the box)"
if ! sudo -u "$CLASH_USER" -H bash -c 'test -x ~/.cargo/bin/cargo'; then
  sudo -u "$CLASH_USER" -H bash -c 'curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable'
fi

echo "== firewall"
ufw --force default deny incoming
ufw default allow outgoing
ufw allow OpenSSH
ufw allow 80/tcp
ufw allow 443/tcp
ufw --force enable
systemctl enable --now fail2ban
dpkg-reconfigure -f noninteractive unattended-upgrades

echo "== postgres roles and database (passwords generated once, kept in $ETC_DIR/db.env)"
DB_ENV="$ETC_DIR/db.env"
if [ ! -f "$DB_ENV" ]; then
  MIG_PW="$(openssl rand -hex 24)"
  APP_PW="$(openssl rand -hex 24)"
  umask 027
  printf 'MIGRATOR_PASSWORD=%s\nAPP_PASSWORD=%s\n' "$MIG_PW" "$APP_PW" > "$DB_ENV"
  chown root:"$CLASH_USER" "$DB_ENV"; chmod 640 "$DB_ENV"
fi
# shellcheck disable=SC1090
. "$DB_ENV"
sudo -u postgres psql -v ON_ERROR_STOP=1 -q <<SQL
DO \$\$ BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'migrator') THEN CREATE ROLE migrator LOGIN PASSWORD '${MIGRATOR_PASSWORD}'; END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'app') THEN CREATE ROLE app LOGIN PASSWORD '${APP_PASSWORD}'; END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'readonly') THEN CREATE ROLE readonly LOGIN; END IF;
END \$\$;
ALTER ROLE migrator PASSWORD '${MIGRATOR_PASSWORD}';
ALTER ROLE app PASSWORD '${APP_PASSWORD}';
SQL
sudo -u postgres psql -tAc "SELECT 1 FROM pg_database WHERE datname='cf'" | grep -q 1 || sudo -u postgres createdb -O migrator cf
sudo -u postgres psql -v ON_ERROR_STOP=1 -q -d cf -c "GRANT USAGE ON SCHEMA public TO app, readonly;"
systemctl enable --now postgresql redis-server

echo "== service env files (created once; edit by hand later, never commit)"
API_ENV="$ETC_DIR/api.env"
if [ ! -f "$API_ENV" ]; then
  umask 027
  cat > "$API_ENV" <<EOF
CF_ENV=${CF_ENV}
CF_BIND=127.0.0.1:8080
CF_DATABASE_URL=postgres://app:${APP_PASSWORD}@127.0.0.1:5432/cf
CF_REDIS_URL=redis://127.0.0.1:6379/0
CF_COOKIE_SECRET=$(openssl rand -hex 32)
CF_CSRF_SECRET=$(openssl rand -hex 32)
CF_SIWS_DOMAIN=${DOMAIN}
CF_WS_ORIGINS=https://${DOMAIN}
CF_CLASH_MINT=9mM1Mc4Ta9UJJ32v5qsHef91PiXi7EWyiSsqF5WXpump
CF_LOG_LEVEL=info
# Платежі CLASH вмикаються, коли задано RPC і казначейство (RUNBOOK §2); без них /orders → 409.
# CF_RPC_PRIMARY_URL=
# CF_RPC_SECONDARY_URL=
# CF_JUPITER_PRICE_URL=
# CF_TREASURY_WALLET=
# CF_TREASURY_TOKEN_ACCOUNT=
# Telegram для джоби alerts (ADR-023):
# CF_ALERT_TELEGRAM_BOT_TOKEN=
# CF_ALERT_TELEGRAM_CHAT_ID=
EOF
  chown root:"$CLASH_USER" "$API_ENV"; chmod 640 "$API_ENV"
fi
ADMIN_ENV="$ETC_DIR/admin.env"
if [ ! -f "$ADMIN_ENV" ]; then
  umask 027
  COOKIE_SECRET="$(grep '^CF_COOKIE_SECRET=' "$API_ENV" | cut -d= -f2-)"
  cat > "$ADMIN_ENV" <<EOF
CF_ENV=${CF_ENV}
CF_ADMIN_BIND=127.0.0.1:9100
CF_DATABASE_URL=postgres://app:${APP_PASSWORD}@127.0.0.1:5432/cf
CF_REDIS_URL=redis://127.0.0.1:6379/0
CF_COOKIE_SECRET=${COOKIE_SECRET}
CF_SIWS_DOMAIN=${DOMAIN}
CF_ADMIN_ORIGIN=http://localhost:9100
CF_ADMIN_WALLETS=${ADMIN_WALLETS}
CF_ADMIN_STATIC_DIR=${APP_DIR}/admin
CF_LOG_LEVEL=info
# Другий фактор (ADR-023): cf-admin totp-new sol:<wallet> → рядок сюди; у prod обовʼязково.
# CF_ADMIN_TOTP_SECRETS=
EOF
  chown root:"$CLASH_USER" "$ADMIN_ENV"; chmod 640 "$ADMIN_ENV"
fi
MIG_ENV="$ETC_DIR/migrate.env"
if [ ! -f "$MIG_ENV" ]; then
  umask 077
  printf 'DATABASE_URL=postgres://migrator:%s@127.0.0.1:5432/cf\n' "${MIGRATOR_PASSWORD}" > "$MIG_ENV"
  chmod 600 "$MIG_ENV"
fi

echo "== systemd units and Caddy"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
install -m 644 "$HERE/cf-api.service" /etc/systemd/system/cf-api.service
install -m 644 "$HERE/cf-admin.service" /etc/systemd/system/cf-admin.service
sed "s/{\$DOMAIN}/${DOMAIN}/g" "$HERE/Caddyfile" > /etc/caddy/Caddyfile
systemctl daemon-reload
systemctl enable cf-api cf-admin caddy
systemctl restart caddy

echo "== done. Next: deploy.sh from the dev machine, then migrate.sh here (owner), then start the services."
