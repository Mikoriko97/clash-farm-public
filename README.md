# Clash Farm

A cozy 3D farming game in the browser and on Solana phones. You grow crops on your own island, explore a shared
mainland with other players, craft, decorate and level up. The game is free to play; cosmetic and convenience items
are bought with the CLASH token — 70 % of every payment is burned, 30 % goes to the ecosystem treasury.

- Play in a browser: https://clashofstaking.online
- Android / Solana Seeker: the dApp Store app (`mobile/`) — the same game, full screen, with Mobile Wallet Adapter.

## What is inside

| Part | Stack | Role |
|---|---|---|
| `client/` | Godot 4.7 (GDScript), web export | The 3D world only: island, mainland, cave, the character, other players. No wallet code, no economy. |
| `web/` | React + TypeScript, Vite | The shell around the canvas: sign-in, HUD, shop, inventory, wallets, payments UI, admin panel. |
| `server/` | Rust (axum, sqlx, Postgres, Redis) | The single source of truth: game state, the double-entry ledger, on-chain payment verification. |
| `mobile/` | Kotlin, Android WebView shell (Solana Mobile CLI) | The dApp Store app: opens the game full screen, landscape, hands wallet intents to the phone's wallet. |

## Solana

- **Sign-in with Solana.** The wallet signs a plain-text message (domain, nonce, expiry); the server verifies the
  ed25519 signature over the exact bytes. Wallets come through Wallet Standard on desktop and through **Mobile
  Wallet Adapter** on Android — the Seeker's own wallet, Phantom, Solflare. A guest island works without a wallet;
  linking one later keeps the progress.
- **Payments without custody.** The server holds no keys and never holds players' tokens. The player's wallet signs
  one transaction with exactly `BurnChecked` (70 %) + `TransferChecked` to a fixed treasury account (30 %) + `Memo`
  + `ComputeBudget`; nothing else is ever built (no approvals, no authority changes). Prices are in USD, converted
  to CLASH by an oracle quote that is fixed for the order.
- **Verification on chain.** Every payment is confirmed by reading the transaction from RPC: mint, owner, amounts,
  memo, status; idempotent by signature. Only then the ledger moves.

## Design rules the code keeps

- The server is authoritative: the client never reports amounts, timers or results — it asks.
- Every economic effect is a ledger row; balances can never go below zero (database constraints).
- Every handler checks that the session's user owns the resource.
- Strict CSP (`script-src 'self'`), no third-party scripts; secrets only in the environment.
- CI on every push: fmt, clippy, ~300 server tests on Postgres and Redis, web lint/typecheck/tests/builds,
  `npm audit`, `cargo deny`, secret scanning (`.github/workflows/ci.yml`).

## Run it locally

Server (Postgres 16, Redis 7):

```bash
cd server
cargo test                       # integration tests use CF_TEST_DATABASE_URL / CF_TEST_REDIS_URL
cargo run -p cf-api              # needs the CF_* variables from .env.example
```

Web:

```bash
cd web
npm ci --ignore-scripts
npm run dev                      # http://localhost:5173 (the Godot export goes to web/public/godot/)
npm test
```

Android app (JDK 17, Android SDK platform 37):

```bash
read -rs SOLANA_MOBILE_KEYSTORE_PASSWORD; export SOLANA_MOBILE_KEYSTORE_PASSWORD SOLANA_MOBILE_KEY_PASSWORD="$SOLANA_MOBILE_KEYSTORE_PASSWORD"
CF_APK_KEYSTORE=/path/to/keystore bash mobile/build-apk.sh
```

## Art and audio

3D models and textures come partly from licensed asset packs, which may not be redistributed, so binary art is left
out of this repository; the Godot scenes, scripts, shaders and the generators of our own models are here. Sound
effects are CC0 (Freesound) or generated; fonts are under the SIL Open Font License.

## Docs

`docs/ARCHITECTURE.md`, `docs/SECURITY.md`, `docs/API.md`, `docs/ECONOMY.md`, `docs/DATA_MODEL.md`,
`docs/BRIDGE_PROTOCOL.md`, `docs/MAINLAND.md`, `docs/UI.md` (in Ukrainian).
