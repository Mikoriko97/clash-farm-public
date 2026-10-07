# Локальні Postgres і Redis для тестів

Рішення власника (2026-09-04): замість Docker/testcontainers — Windows-сервіси.

| Сервіс | Пакет | Порт | Авторизація |
|---|---|---|---|
| PostgreSQL 16 | winget `PostgreSQL.PostgreSQL.16` | 5432 | `trust` лише для `127.0.0.1/32` і `::1/128` у `pg_hba.conf` |
| Memurai Developer (Redis 7-сумісний) | winget `Memurai.MemuraiDeveloper` | 6379 | без пароля, bind 127.0.0.1 |

Ініціалізація ролей і баз: `infra/db/init_local.sql` (виконати один раз).

Змінні для тестів (лише в оточенні процесу, ніколи у файлах репо):
```
CF_TEST_DATABASE_URL=postgres://migrator@127.0.0.1:5432/cf_test
CF_TEST_REDIS_URL=redis://127.0.0.1:6379/1
```
Тести самі застосовують міграції з `server/migrations` (`sqlx::migrate!`) і чистять дані між кейсами.
