# ARCHITECTURE

## 1. Компоненти

```
┌──────────────────────────── браузер ────────────────────────────┐
│  /web  React-шелл                                                │
│   ├─ wallet (wallet-adapter: Phantom/Solflare/Backpack)          │
│   ├─ api client (fetch, cookie session, CSRF)                    │
│   ├─ UI: магазин, інвентар, профіль, сезони, материк (партнери)  │
│   └─ bridge ─── JavaScriptBridge ───┐                            │
│                                     ▼                            │
│  /client Godot 4.7 (canvas)  ─ острів, місто, персонаж, кліки    │
└───────────────────────────────┬──────────────────────────────────┘
                                │ HTTPS (REST), пізніше WS
┌───────────────────────────────▼──────────────────────────────────┐
│  /server Rust (axum)                                              │
│   api ── auth ── world ── catalog ── ledger ── payments ── admin  │
│                    │                             │                │
│               Postgres                     chain (RPC ×2,        │
│               Redis                        Jupiter Price)        │
└───────────────────────────────────────────────────────────────────┘
                                                 │
                                    Solana (читання транзакцій)
```

Ланцюг використовується **лише для читання**. Запис у ланцюг робить гравець своїм гаманцем (burn). Сервер не має keypair.

## 2. Потоки

**Логін.** web → `GET /auth/nonce` → підпис SIWS у гаманці → `POST /auth/verify` → cookie-сесія. Godot про це не знає; отримує `session_ready` через міст.

**Ігрова дія (посадити).** Godot: клік по ділянці → міст `plot_selected(id)` → React: показує вибір насіння → `POST /plots/{id}/plant` → сервер перевіряє власність, ліміти, інвентар → відповідь зі снапшотом ділянки → React → міст `world_state(patch)` → Godot рендерить.

**Покупка за CLASH.** React: `POST /orders` (quote) → будує tx (Burn+Memo) → симуляція → екран підтвердження → підпис → `POST /orders/{id}/submit` → сервер верифікує ончейн → фулфілмент → React оновлює інвентар → міст `inventory_changed`.

**Материк (Фаза 2).** Будівля = React-екран партнера (Sanctum/Jupiter/перп-DEX через їх SDK). Дії гравця там → сервер отримує підтвердження (webhook/polling по гаманцю) → нараховує міські кредити в ледж.

## 3. Репозиторій

```
clash-farm/
├── CHANGELOG.md
├── docs/                   # цей набір
├── web/                    # React + TypeScript + Vite
│   └── src/{wallet,api,payments,ui,bridge,mainland}
├── client/                 # Godot 4.7 проєкт
│   └── {scenes,scripts,assets,export_presets.cfg}
├── server/                 # Rust workspace
│   ├── Cargo.toml
│   ├── crates/{api,auth,world,catalog,ledger,payments,chain,admin,common}
│   ├── migrations/         # sqlx migrations, тільки вперед
│   └── tests/              # інтеграційні (testcontainers: Postgres, Redis)
├── infra/                  # docker-compose, Caddyfile, deploy-скрипти (без секретів)
└── shared/
    └── bridge-protocol.json   # схема повідомлень мосту (джерело для TS і GDScript)
```

## 4. Стек і версії

| Шар | Технологія | Примітка |
|---|---|---|
| Web | React 18+, TypeScript strict, Vite, `@wallet-standard/app` (Solana, ADR-013), `viem` (EVM-логін, ADR-012), `@solana/web3.js` лише для платежів (тиждень 4) | Жодних UI-кітів із рантайм-скриптами з CDN |
| Godot | 4.7.x, Compatibility renderer, веб-експорт **single-thread** | Forward+/Mobile на вебі недоступні; SharedArrayBuffer-режим ламає інтеграцію гаманців |
| Server | Rust stable, axum, tokio, sqlx (offline mode у CI), serde, jsonwebtoken, ed25519-dalek, solana-client/solana-sdk, spl-token, reqwest, tracing, redis | `#![deny(unsafe_code)]` у всіх крейтах |
| DB | PostgreSQL 16, Redis 7 | Ролі `app`/`migrator`, див. DATA_MODEL |
| Infra | Docker Compose на 1 VPS (Hetzner/аналог), Caddy (TLS, HSTS), Uptime-моніторинг | Prod і staging — окремі VPS або окремі compose-стеки з окремими БД |

## 5. Середовища

| Env | Ланцюг | Дані | Хто деплоїть |
|---|---|---|---|
| `staging` | mainnet, тестовий mint CLASH-DEV **або** mainnet CLASH з `$0.50` товарами | окрема БД | власник |
| `prod` | mainnet | prod БД | власник вручну |

Конфіг лише через env (`CF_*`). Список у `docs/RUNBOOK.md` §2. Сервер відмовляється стартувати, якщо `CF_CLASH_MINT` не збігається з ончейн-даними (decimals, mint authority = none).

## 6. Godot-специфіка

- Сцена `Island.tscn`: грід ділянок (`PlotNode` з `plot_id`), персонаж (`CharacterBody3D`), камера, декор-слоти. Стан приходить із мосту, локальних таймерів немає (лише візуальні, не логічні).
- Клієнт не знає цін, балансів у CLASH, правил економіки. Знає лише: що де стоїть, який стан у ділянки (`empty|growing|ready`), куди можна поставити декор.
- Ассети: glTF, текстури ≤ 1024, запечене освітлення, бюджет острова ≤ 150K трикутників, місто — LOD.
- Експорт: `index.html` з кастомного шаблону в `/web/public/godot/`; React інстанціює `Engine` на власному canvas.
- Два паки під формат текстур (2026-09-22): пресет «Web» — повний експорт (`--export-release Web …/index.html`), текстури лише S3TC; пресет «Web Mobile» — лише пак (`--export-pack "Web Mobile" …/web/public/godot/index.mobile.pck`), текстури лише ETC2. `web/src/bridge/host.ts` (`packFor`) дає рушію `mainPack` за `WEBGL_compressed_texture_s3tc` — тим самим тестом Godot обирає формат; без мобільного паку вантажить `index.pck`. Фільтри файлів у двох пресетах мають збігатися (змінюєш один — зміни й другий); `tools/check_export.gd` ганяти проти обох паків. Кожен пак ≈ 25 МБ замість 28,7 з обома форматами.
- Анімації персонажа: усе через `SkeletonProfileHumanoid` — модель `HumanM_Model.fbx` (bone map `bone_map_rig.tres`), кліпи з пакета/Mixamo (`bone_map_mixamo.tres`) і бібліотека Quaternius Universal Animation Library (CC0, `assets/character/ual2.glb`, `bone_map_ual.tres`; сирі паки лежать у `client/Motion/`, поза git). `tools/extract_anims.gd` збирає `character_anims.tres`: одиночні FBX + таблиця `LIBRARIES` (кліп бібліотеки → наша назва); треки перевішуються на `Model/Skeleton3D`. Інструменти в руці — `assets/tools/*.fbx` (власник, PolyPack) під `BoneAttachment3D` `RightHand` у `Player.tscn`; дія показує інструмент з таблиці `FX_CLIPS` (`island_input.gd`), `play_fx.tool` перекриває.

## 7. Web-специфіка

- Стан застосунку — один стор (Zustand або аналог), джерело — відповіді сервера. Ніякого «локального» розрахунку балансів.
- Оверлей над canvas: `pointer-events: none` на контейнері, `auto` на інтерактивних елементах. Модалка відкрита → міст `set_mode(ui)` → Godot ігнорує ввід.
- Транзакції будуються у `web/src/payments/build.ts` — єдине місце. Allowlist інструкцій — константа з тестом.
- Перший бандл без web3 (2026-09-22): бекенди гаманців (`wallet/create.ts`: viem, wallet-standard) і Solana-частина платежів (`payments/solana.ts`: web3.js, spl-token, `build.ts`, `rpc.ts`) — окремі чанки того самого бандла, вантажаться `loadWalletManager()` / `loadSolana()` (`lazyOnce`: одне завантаження на всіх, збій — повтор). Екран входу стартує пошук гаманців при монтуванні, підключення — лише за кліком. Перший бандл 764 → 331 КБ; тест `src/__tests__/entry-imports.test.ts` не дає повернути ці бібліотеки в статичні імпорти. CSP не змінювалась (`script-src 'self'`).

## 8. Що явно НЕ робимо (Фаза 1)

Власні програми Solana. Серверний гаманець. Депозити. Вивід кредитів. NFT. Мультиплеєр у реальному часі. Мобільні нативні збірки. Сторонні скрипти аналітики.
