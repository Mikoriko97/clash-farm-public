# Design preview — перегляд дизайн-варіантів без бекенду і Godot

Лише для дизайну, у продакшн-збірку не потрапляє (`vite build` збирає тільки `index.html`).
Реальний React-HUD запускається на фейкових даних; 3D-світ (Godot-експорту в репо нема) замінює
намальований фон `backdrop.svg`.

## Мок-режим на staging (без терміналів)

Staging-збірка (деплой з `VITE_MOCK_API=1`) має мок-режим: `https://clashofstaking.online/?mock=1` — справжній Godot-світ і весь HUD на фейкових даних, без гаманця й сесії (`?mock=nosession` — екран входу, `?mock=nochar` — створення персонажа). Код — `web/src/mock/` (TS-порт цього `mock-api.mjs`; фікстури тримати синхронно), увімкнено збіркою з `VITE_MOCK_API=1` (`infra/deploy/deploy.sh` за замовчуванням збирає БЕЗ нього — на staging вмикати явно: `VITE_MOCK_API=1 SERVER=… bash infra/deploy/deploy.sh`; без прапорця коду моку в збірці нема, і скрипт це перевіряє). У мок-режимі жовта позначка «Mock data» внизу, API-клієнт не ходить у мережу (читання з RPC гаманця, якщо його підключено, лишаються), сокет присутності не відкривається, а котирування оплати має заглушку замість реального мінта й казначейства, тож підписати його нічим.

## Запуск локально (два термінали, з папки `web/`)

```bash
npm run design:mock   # мок-API на 127.0.0.1:8787 (порт 8080 лишається справжньому серверу)
npm run design        # Vite з vite.design.config.ts → http://localhost:5173
```

Стани сторінки: `/?demo=1` — демо-екрани будівель міста. Мок перемикається кукою `mock=nosession`
(екран входу) або `mock=nochar` (створення персонажа); без куки — гравець із персонажем і островом.
Стан HUD (де персонаж стоїть, рівень) у браузері можна міняти з консолі:

```js
(await import("/src/store/appStore.ts")).useAppStore.setState({ near: { kind: "plot", idx: 0, distance: 1 } })
```

## Знімки екранів

```bash
npm run design:shoot -- --out ../design-shots/aaply              # усі екрани (desktop 1440×900 + mobile 390×844)
npm run design:shoot -- --out ../design-shots/aaply --only shop,menu
node design-preview/gallery.mjs baseline aaply                   # design-shots/gallery.html: варіанти поруч
```

Потрібен Chrome або Edge (шукається у стандартних теках Windows). `design-shots/` у git не потрапляє.

## Як влаштовано теми

- Базовий вигляд — `src/hud/hud.css`, `src/character/creator.css`, `src/city/city.css` (темне скло). Їх не чіпаємо.
- Поточний варіант цієї гілки: Aaply Dark (`data-theme="aaply-dark"` у index.html; адмінка — `src/admin/admin-aaply-dark.css`).
- Варіант = окремий оверлей у `src/themes/<name>.css` з правилами під `:root[data-theme="<name>"]`
  (нативна вкладеність CSS; `@font-face` і `@keyframes` — лише поза блоком) + атрибут
  `data-theme` на `<html>` у `index.html` і імпорт у `src/main.tsx`.
  Прибрати атрибут — повернутись до оригіналу; A/B-порівняння — зміною атрибута в DevTools.
- Шрифти варіанта — у `public/fonts` (самохостинг, CSP `font-src 'self'`, ліцензії OFL поруч).
- Кожен варіант — окрема гілка `design/<name>`.
