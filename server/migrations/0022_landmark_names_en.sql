-- 0022: English landmark names (owner decision 2026-09-28: the game UI is English-only for
-- now; the admin stays Ukrainian). Names are display text on the mainland signs and the
-- map; ids, positions, rewards and discoveries do not change.

UPDATE cf.landmarks SET name = v.name
FROM (VALUES
    ('dock',         'Dock'),
    ('shipwreck',    'Shipwreck'),
    ('old_mill',     'Old mill'),
    ('swamp_hut',    'Swamp hut'),
    ('lighthouse',   'Lighthouse ruins'),
    ('dark_altar',   'Dark forest altar'),
    ('stone_circle', 'Stone circle'),
    ('waterfall',    'Waterfall'),
    ('lookout',      'Lookout point'),
    ('mine_gate',    'Mine entrance'),
    ('old_bridge',   'Old bridge'),
    ('lake_pier',    'Lake pier')
) AS v(id, name)
WHERE cf.landmarks.id = v.id;

-- Impact: 12 rows of a reference table updated; instant; no downtime.
