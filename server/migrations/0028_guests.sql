-- 0028: guest islands (owner ok 2026-10-02, docs/PRELAUNCH_REVIEW.md §2.1): a visitor plays at once, without a wallet,
-- and keeps the progress by linking a wallet later with a signature in the same session (POST /auth/link).
-- A guest is a user without a wallet (`wallet IS NULL`); the UNIQUE constraint allows many NULLs. Guests cannot pay
-- (orders need a wallet) and never receive anything of value, so a flood of guests prints nothing withdrawable;
-- creation is still capped per IP and has its own switch next to `registrations_enabled`.

ALTER TABLE cf.users ALTER COLUMN wallet DROP NOT NULL;

INSERT INTO cf.settings (key, value) VALUES
    ('guests_enabled', 'true'::jsonb)   -- kill switch for POST /auth/guest (registrations_enabled still applies)
ON CONFLICT (key) DO NOTHING;

-- Impact: dropping NOT NULL is a catalog-only change in Postgres (no scan, no rewrite, a brief ACCESS EXCLUSIVE lock on
-- cf.users); existing rows all keep their wallet. Roll out BEFORE the new binary; the old binary keeps working (it
-- never creates a guest). Code that reads `users.wallet` must treat it as optional from this migration on.
