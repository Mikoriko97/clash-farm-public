-- 0025: the character a player creates once at the first login (ADR-025, docs/DATA_MODEL.md §2).
-- `users.nickname` existed but nothing set it (every row is NULL); from now on only
-- POST /me/character writes it, together with a lookalike-proof key and the look. Additive:
-- rows that never created a character stay NULL/NULL/NULL and are sent to the creator screen.
-- Runs as role `migrator`.

ALTER TABLE cf.users
    ADD COLUMN IF NOT EXISTS nickname_key text,
    ADD COLUMN IF NOT EXISTS look         jsonb;

-- Defence in depth for the rules in cf_world::nickname (the code is the first line):
-- 3..16 chars, Latin letters/digits/underscore, starts with a letter, no leading, trailing
-- or doubled underscore; nickname, key and look exist together or not at all.
ALTER TABLE cf.users
    ADD CONSTRAINT users_nickname_format CHECK (
        nickname IS NULL
        OR (nickname ~ '^[A-Za-z](?:_?[A-Za-z0-9])+$' AND char_length(nickname) BETWEEN 3 AND 16)
    ),
    ADD CONSTRAINT users_character_together CHECK (
        (nickname IS NULL) = (nickname_key IS NULL) AND (nickname IS NULL) = (look IS NULL)
    ),
    ADD CONSTRAINT users_look_object CHECK (look IS NULL OR jsonb_typeof(look) = 'object');

-- One name per lookalike class ("Adm1n" and "admin" share a key): the final barrier against
-- two players claiming the same name in parallel.
CREATE UNIQUE INDEX IF NOT EXISTS users_nickname_key_uidx
    ON cf.users (nickname_key) WHERE nickname_key IS NOT NULL;

-- Impact: two nullable columns on cf.users (no rewrite), three CHECK constraints and one
-- partial UNIQUE index. The CHECKs are validated against existing rows: every nickname is NULL
-- so far, so they pass; a hand-written nickname that breaks the format would abort the whole
-- migration (it runs in a transaction) — count `nickname IS NOT NULL` before staging/prod.
-- ADD CONSTRAINT takes ACCESS EXCLUSIVE and scans the table, CREATE UNIQUE INDEX is not
-- CONCURRENTLY (sqlx migrates in a transaction): milliseconds at this size, seconds at 1M
-- rows. No downtime; the previous release runs unchanged on the new schema (it never reads
-- the columns), so migrate first, then deploy.
