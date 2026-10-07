# DATA_MODEL — PostgreSQL

Принципи: цілі числа для грошей; `CHECK`-обмеження замість «перевіримо в коді»; append-only там, де це гроші; кожна таблиця з даними гравця має `user_id` і індекс по ньому.

## 1. Ролі

| Роль | Права | Використовує |
|---|---|---|
| `migrator` | DDL на схемі `cf` | тільки міграції, запускає власник |
| `app` | `SELECT/INSERT/UPDATE` на ігрових таблицях; на `ledger`, `burns`, `audit_log` — лише `SELECT/INSERT`; `alerts` — `SELECT/INSERT/UPDATE`, `alert_mutes` — і `DELETE` (0021) | сервер |
| `readonly` | `SELECT` | аналітика, дашборди |

Тригери `forbid_update_delete()` на `ledger`, `burns`, `audit_log` — навіть якщо права помилково розширять.

## 2. Таблиці (Фаза 1)

```sql
-- Ідентичність
users(id uuid pk, wallet text unique (NULL — гостьовий острів до прив'язки, 0028/ADR-031), nickname text, created_at, last_seen_at,
      reputation int not null default 0, flags jsonb not null default '{}',
      -- 0025 (ADR-025): персонаж, обраний один раз; три колонки є разом або жодної
      nickname_key text unique-when-set (нік у нижньому регістрі зі згорнутими схожими символами), look jsonb {skin,shirt,pants,shoes,hair_color,hair,face,beard,hat,glasses,outfit,outfit_color,body,build} (індекси таблиць клієнта; `body` 0 чоловіче / 1 жіноче, ADR-027; `build` 0 середня / 1 струнка / 2 міцна, ADR-029; рядки до етапу 3 мають лише перші чотири, решта читається як 0);
      CHECK формату ніка ^[A-Za-z](_?[A-Za-z0-9])+$ 3..16)
nickname_history(id bigserial pk, user_id fk, old_nickname, old_key, new_nickname, new_key, changed_at)  -- 0030 (ADR-039): платні зміни ніка, append-only;
      -- old_key, змінений менш ніж 30 днів тому, тримається за user_id (ніхто інший не займе); індекси (old_key, changed_at desc), (user_id, changed_at desc)
      -- 0031 (ADR-040): kind 'rename' | 'moderation'; останній рядок 'moderation' = наступна зміна безкоштовна
nickname_blocks(id bigserial pk, key, nickname, reason 1..200, blocked_by, created_at, lifted_at, lifted_by)  -- 0031 (ADR-040): заблоковані імена;
      -- unique (key) where lifted_at is null; рядки не видаляються, лише одне зняття (тригер nickname_block_lift_only)
sessions(id uuid pk, user_id fk, family uuid not null, refresh_hash bytea unique,
         expires_at, revoked_at, created_at, ip_hash bytea, ua_hash bytea)

-- Світ
islands(id uuid pk, user_id fk unique, level int not null default 1, xp bigint not null default 0,
        rested_until timestamptz,                                                -- 0026: «Відпочив» після сну, годинник БД
        rested_bonus_pct smallint not null default 0 check (rested_bonus_pct between 0 and 100),  -- % до XP від дій, копія з settings на момент сну
        rested_carry smallint not null default 0 check (rested_carry between 0 and 99),           -- частки XP (соті), що не склали цілого
        expand_n smallint not null default 0 check (expand_n between 0 and 4),   -- ADR-015: платні розширення
        expand_e smallint not null default 0 check (expand_e between 0 and 4),   -- по кожному боку
        expand_s smallint not null default 0 check (expand_s between 0 and 4),
        expand_w smallint not null default 0 check (expand_w between 0 and 4),
        created_at)
plots(id uuid pk, island_id fk, idx smallint not null, state text not null
        check (state in ('empty','growing','ready')),
      crop_id text, planted_at, ready_at, fertilizer_id text, unique(island_id, idx),
      col smallint not null, row smallint not null, unique(island_id, col, row))   -- 0006: клітинка сітки 3 м; група суміжних = похідна
decor_placements(id uuid pk, island_id fk, item_id text not null, x_dm smallint not null, z_dm smallint not null,
        yaw_deg smallint not null check (0..345, крок 15), slot smallint null (спадок до 0009), placed_at)   -- 0009: вільна точка, перекриття за items.meta.footprint_dm
mainland_nodes.id — після 0016 також `99_<печера>_<i>` (печера) і `98_<слот>_<i>` (грозові дерева); items + `crystal` (tier 4).
explored(user_id pk, mask bytea ≤ 32, updated_at) — туман карти материка (0014), біт cz*16+cx. landmarks(id text pk, name, x_dm, z_dm, radius_dm 10..1000, xp, reward jsonb, enabled) — орієнтири; discoveries(user_id, landmark_id; pk обидва) — одне відкриття на гравця. expeditions(user_id, day; pk обидва; tasks jsonb, done jsonb {idx: {progress, claimed}}) — щоденні завдання (0015).
tool_wear(user_id fk, item_id fk, wear int >= 0, updated_at; pk (user_id, item_id)) — знос поточної одиниці інструмента (0013); сам інструмент — предмет в inventory. items.kind + 'tool'.
mainland_nodes(id text pk '^cx_cz_i$', kind text, hits_left int >= 0, respawn_at timestamptz?, respawns int >= 0 default 0 (0024: скільки разів вичерпано — за ним вузол стоїть на `[дім, alts…][respawns mod (1 + alts)]`), updated_at) — спільні вузли материка (0012); рядок лише для торкнутих вузлів, позиції/види/`alts` — із запеченої таблиці `shared/mainland_nodes.json` (cf_world::mainland, пише client/tools/flora_baker.gd). items.tier smallint 1..4 (0012, MAINLAND §6).
resource_nodes(id uuid pk, island_id fk, idx smallint not null, kind text check (kind in ('tree','rock')),
        hits_left int not null check (>= 0), respawn_at timestamptz null, unique(island_id, idx),
        x_dm smallint, z_dm smallint (обидва або жоден, |·| ≤ 600), grown_at timestamptz not null default now(), variant smallint 0..7)
        -- 0007: стан; 0017: живі ресурси — точка на острові обирається сервером (cf_world::regrow: вільне місце поза хатою, грядками, декором, іншими вузлами, у формі з купленими смугами), вичерпаний вузол після respawn_at переноситься джобою на нове місце і росте від grown_at (стадія 0/1/2 за economy.tree_grow_secs; камені одразу цілі); куплена смуга додає economy.strip_trees/strip_rocks; стартові 8 тримають точки Island.tscn
recipes(id text pk, output_item fk items, output_qty int > 0, inputs jsonb {item_id: qty}, xp int >= 0, enabled bool)   -- 0008: адмін пише, сервер читає
inventory(user_id fk, item_id text not null, qty int not null check (qty >= 0),
        primary key(user_id, item_id))

-- Каталог (керується адміном, читається сервером)
items(id text pk, kind text not null check (kind in ('seed','fertilizer','decor','misc','material','produce','dish')),   -- 0008: produce/dish мають meta.sell_credits, dish — meta.xp, seed — meta.produce
      price_usd_cents int check (price_usd_cents >= 0), exclusive bool not null default false,
      enabled bool not null default true, meta jsonb not null default '{}')

-- Економіка
ledger(id bigserial pk, user_id fk null, account text not null, delta bigint not null,
       currency text not null, ref_type text not null, ref_id uuid not null, created_at)
balances(user_id fk, account text, amount bigint not null check (amount >= 0),
         primary key(user_id, account))
orders(id uuid pk, user_id fk, item_id fk, qty int check (qty > 0),
       price_usd_cents int not null, clash_per_usd numeric(30,12) not null,
       amount_clash bigint not null, quote_expires_at not null,
       status text not null check (status in ('quoted','submitted','fulfilled','expired','rejected','needs_review')),
       created_at, updated_at)
burns(id uuid pk, order_id fk unique, signature text unique not null, slot bigint, block_time,
burn_finality(order_id uuid pk fk orders, signature text, outcome text in (finalized, missing), slot bigint, checked_at)   -- 0011: вердикт джоби finalize_burns для confirmed-burns; append-only
      amount bigint not null, commitment text not null, verified_at not null,
      raw_summary jsonb not null)

-- Захист і аудит
rate_events(user_id fk, action text, day date, count int, primary key(user_id, action, day))
audit_log(id bigserial pk, actor text not null, action text not null, target text,
          before jsonb, after jsonb, ip_hash bytea, created_at)
events(id bigserial pk, user_id fk null, name text not null, props jsonb, created_at)
settings(key text pk, value jsonb not null, updated_at, updated_by text)   -- kill switches, economy.*, alerts.* (пороги правил)

-- Сигнали (0021, ADR-023): один відкритий рядок на правило (unique (rule) where resolved_at is null)
alerts(id bigserial pk, rule text, level text check in ('warn','crit'), title text, detail jsonb,
       opened_at, last_seen_at, resolved_at null, notified_at null, acked_by text null, acked_at null, ack_note null)
alert_mutes(rule text pk, until timestamptz, muted_by text, note text null, created_at)

-- Розчищені дикі кущі (0023, косметика): позиція на сітці 0,5 м, ≤ 400 на острів (код)
island_clearings(island_id fk cascade, x_dm smallint, z_dm smallint, cleared_at, pk(island_id, x_dm, z_dm), check x%5=0 and z%5=0)
```

## 3. Ледж: правила

- Кожна економічна подія — **одна** транзакція БД, що пише ≥ 2 рядки в `ledger` (сума `delta` по `ref_id` та `currency` = 0) і оновлює `balances` тим самим `ref_id`.
- Системні рахунки: `system:mint` (джерело кредитів за ігрові дії), `system:sink` (куди йдуть витрачені), `system:burn_clash` (облік спалених CLASH у atomic units, currency `CLASH`).
- Валюти: `CREDITS` (внутрішня, невивідна), `XP`, `CLASH` (тільки облік burn, ніколи не баланс гравця).
- Ніколи `UPDATE ledger`. Помилка = компенсуючий запис із `ref_type='reversal'` і посиланням на оригінал.
- Нічна джоба: `SUM(delta) GROUP BY user_id, account` порівнюється з `balances`; розбіжність = алерт + `settings.shop_enabled=false`.

## 4. Конкурентність

- Дія над ділянкою: `SELECT ... FROM plots WHERE id=$1 AND island_id IN (SELECT id FROM islands WHERE user_id=$2) FOR UPDATE`. Нема рядка → 404 (не 403, щоб не підтверджувати існування).
- Покупка: `SELECT ... FROM orders WHERE id=$1 AND user_id=$2 AND status='quoted' FOR UPDATE`.
- Унікальність `burns.signature` — фінальний бар'єр від подвійного фулфілменту.
- Денні ліміти: `INSERT ... ON CONFLICT (user_id, action, day) DO UPDATE SET count = rate_events.count + 1 WHERE rate_events.count < $limit RETURNING count` — атомарно.

## 5. Міграції

- `sqlx migrate` — тільки вперед. Файл `NNNN_<опис>.sql`. Ніколи не редагувати вже застосовану міграцію.
- Кожна міграція — окремий коміт `chore(server/migrations): ...` з описом впливу на дані.
- Деструктивні зміни (drop/alter type) — двофазно: додати нове → перенести → видалити старе через ≥ 1 реліз.
- Локально: testcontainers застосовує всі міграції з нуля в CI; це тест на консистентність.
- На prod міграції запускає **власник** руками з бекапом перед цим.

## 6. Ретенція

`events` — 90 днів; `sessions` (revoked/expired) — 30 днів; `rate_events` — 30 днів; `ledger`, `burns`, `burn_finality`, `audit_log`, `orders` — назавжди. Джоба `retention` у cf-api (0011: `GRANT DELETE` на `events`, `rate_events`) видаляє батчами; ретенція `sessions` — пізніше.
