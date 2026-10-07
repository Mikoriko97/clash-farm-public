-- 0014: exploration (docs/MAINLAND.md §4, owner ok 2026-09-09).
-- The map lifts its fog per 64 m chunk the player has stood in: one 32-byte mask
-- (256 bits, bit = cz * 16 + cx) per player. Landmarks are fixed points with a
-- discovery reward; a discovery is one row per player and landmark, so it can
-- never pay twice. Positions come from the presence socket, never from the client.

CREATE TABLE cf.explored (
    user_id     uuid        PRIMARY KEY REFERENCES cf.users(id) ON DELETE CASCADE,
    mask        bytea       NOT NULL DEFAULT '\x'::bytea CHECK (octet_length(mask) <= 32),
    updated_at  timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE cf.landmarks (
    id          text    PRIMARY KEY CHECK (id ~ '^[a-z0-9_]{2,32}$'),
    name        text    NOT NULL,
    x_dm        int     NOT NULL,
    z_dm        int     NOT NULL,
    -- How close (dm) presence must put the character for the discovery to count.
    radius_dm   int     NOT NULL DEFAULT 150 CHECK (radius_dm BETWEEN 10 AND 1000),
    xp          int     NOT NULL DEFAULT 0 CHECK (xp >= 0),
    -- Items handed over on the first visit: {item_id: qty}.
    reward      jsonb   NOT NULL DEFAULT '{}'::jsonb,
    enabled     boolean NOT NULL DEFAULT true
);

CREATE TABLE cf.discoveries (
    user_id     uuid        NOT NULL REFERENCES cf.users(id) ON DELETE CASCADE,
    landmark_id text        NOT NULL REFERENCES cf.landmarks(id),
    at          timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, landmark_id)
);

-- Placeholder landmarks on the baked map (tools/make_mainland.gd); positions move
-- with the owner's assets, ids stay.
INSERT INTO cf.landmarks (id, name, x_dm, z_dm, radius_dm, xp, reward) VALUES
    ('dock',        'Причал',                 0,    4000, 200, 10, '{}'),
    ('shipwreck',   'Кістяк корабля',        -2200, 3900, 150, 30, '{"reed": 3}'),
    ('old_mill',    'Старий млин',           -3800,  600, 150, 30, '{"herb": 3}'),
    ('swamp_hut',   'Хатина на болоті',      -3800, 3300, 150, 40, '{"mushroom": 2}'),
    ('lighthouse',  'Руїни маяка',           -1200, -4200, 150, 50, '{"mushroom_rare": 1}'),
    ('dark_altar',  'Вівтар темного лісу',   -3600, -4300, 150, 60, '{"mushroom_rare": 1}'),
    ('stone_circle','Камʼяне коло',           1000, -3800, 150, 40, '{"wood_oak": 3}'),
    ('waterfall',   'Водоспад',               3300, -1200, 150, 50, '{"ore_copper": 2}'),
    ('lookout',     'Оглядовий майданчик',    4200,   400, 150, 40, '{"stone": 3}'),
    ('mine_gate',   'Вхід у шахту',           2800, -2000, 150, 60, '{"ore_iron": 1}'),
    ('old_bridge',  'Старий міст',            1200,  1500, 150, 30, '{"wood_pine": 3}'),
    ('lake_pier',   'Пірс на озері',          2600,  -300, 150, 30, '{"herb": 2}')
ON CONFLICT (id) DO NOTHING;

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'app') THEN
        GRANT SELECT, INSERT, UPDATE ON cf.explored TO app;
        GRANT SELECT ON cf.landmarks TO app;
        GRANT SELECT, INSERT ON cf.discoveries TO app;
    END IF;
END $$;

-- Impact: three new tables, twelve catalog-like rows; no player rows touched; no downtime.
