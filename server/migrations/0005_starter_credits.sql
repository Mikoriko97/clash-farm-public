-- Onboarding (рішення власника 2026-09-04, docs/UI.md §5.3): новачок отримує
-- стартові культури і трохи кредитів, щоб пройти цикл посадка → збір → покупка.
-- Числа регулюються через settings без релізу (ECONOMY §3).
INSERT INTO cf.settings (key, value) VALUES
    ('economy.starter_credits',      '30'::jsonb),
    ('economy.starter_carrot_seeds', '2'::jsonb)
ON CONFLICT (key) DO NOTHING;
