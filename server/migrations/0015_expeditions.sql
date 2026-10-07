-- 0015: daily expeditions (docs/MAINLAND.md §4, owner ok 2026-09-09).
-- Three tasks a day per player, rolled from a seed of (user, day) so every
-- instance agrees; progress is counted inside the same transactions that gather,
-- explore and discover, and each task pays its XP once (claimed flag under the
-- row lock). Rewards are XP only until city credits arrive with M4.

CREATE TABLE cf.expeditions (
    user_id     uuid        NOT NULL REFERENCES cf.users(id) ON DELETE CASCADE,
    day         date        NOT NULL,
    -- [{ kind: "gather"|"explore"|"discover", target?, count, xp }]
    tasks       jsonb       NOT NULL,
    -- { "<index>": { progress, claimed } }
    done        jsonb       NOT NULL DEFAULT '{}'::jsonb,
    updated_at  timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, day)
);

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'app') THEN
        GRANT SELECT, INSERT, UPDATE ON cf.expeditions TO app;
    END IF;
END $$;

-- Impact: one new table; no player rows touched; no downtime. Old rows can be
-- dropped by retention after 30 days (job later).
