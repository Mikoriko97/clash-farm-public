# Деплой на один VPS без Docker (staging, 2026-09-29)

Поки нема CI й образів (RUNBOOK §3 описує майбутнє), гра живе на одному сервері:
Caddy (TLS сам) → статика `web/dist` + `/api/*` → `cf-api` на 127.0.0.1:8080; PostgreSQL 16 і
Redis з пакетів; `cf-admin` лише на 127.0.0.1:9100 (SSH-тунель `ssh -L localhost:9100:127.0.0.1:9100`).
Секрети генеруються на сервері у `/etc/clash-farm/*.env` (root:clash 640) і не залишають його.

| Крок | Де | Команда |
|---|---|---|
| 1. Підготувати сервер (раз) | сервер, root | `DOMAIN=<host> ADMIN_WALLETS=sol:<base58> bash bootstrap.sh` |
| 2. Деплой коду | машина розробника | `SERVER=root@<ip> bash infra/deploy/deploy.sh` (перед цим — експорт Godot-паків) |
| 3. Міграції | сервер, root (власник) | `bash /opt/clash-farm/src/infra/deploy/migrate.sh` |
| 4. Старт/рестарт | сервер | `systemctl restart cf-api cf-admin` (deploy.sh робить сам) |
| 5. Смоук | будь-де | `https://<host>/healthz`, `/readyz`, вхід гаманцем, посадка → збір |

`DOMAIN` — хост із DNS на сервер; без свого домену тимчасово годиться `<ip з дефісами>.sslip.io`
(TLS від Let's Encrypt працює, Phantom підписує лише на https). Він же йде в `CF_SIWS_DOMAIN`.

Що НЕ вмикається без ручного кроку: платежі CLASH (`CF_RPC_*`, казначейство — RUNBOOK §2),
Telegram для сигналів (`CF_ALERT_TELEGRAM_*`), TOTP адмінів (`cf-admin totp-new`, у prod обовʼязково).
Логи: `journalctl -u cf-api -f`, `journalctl -u cf-admin -f`, `journalctl -u caddy -f`.
Бекапи (RUNBOOK §4) і моніторинг (§5) — окремо, поки staging.
