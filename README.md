# PolyFollow

<p align="center">
  <strong>Self-custody, paper-first Polymarket copy-trading engine.</strong>
</p>

<p align="center">
  <a href="https://github.com/okloorcl/polyfollow/actions/workflows/ci.yml"><img alt="ci" src="https://github.com/okloorcl/polyfollow/actions/workflows/ci.yml/badge.svg"></a>
  <a href="https://github.com/okloorcl/polyfollow/actions/workflows/release.yml"><img alt="release" src="https://github.com/okloorcl/polyfollow/actions/workflows/release.yml/badge.svg"></a>
  <a href="https://github.com/okloorcl/polyfollow"><img alt="repo" src="https://img.shields.io/badge/repo-okloorcl%2Fpolyfollow-24292f"></a>
  <img alt="rust" src="https://img.shields.io/badge/Rust-2024-b7410e">
  <img alt="polymarket" src="https://img.shields.io/badge/Polymarket-CLOB-0f766e">
  <img alt="mode" src="https://img.shields.io/badge/default-paper%20trading-2563eb">
  <img alt="storage" src="https://img.shields.io/badge/storage-SQLite-003b57">
  <img alt="output" src="https://img.shields.io/badge/output-CLI%20%2B%20JSON%20%2B%20HTTP-111827">
  <img alt="license" src="https://img.shields.io/badge/license-MIT-green">
</p>

<p align="center">
  <a href="README.md">English</a> |
  <a href="README.zh-CN.md">简体中文</a>
</p>

PolyFollow watches one or more Polymarket leader wallets, turns their new
trades into copy intents, applies strict per-leader and account-level risk
controls, and executes in paper mode by default. Live trading exists, but it is
guarded by explicit flags, environment-only private keys, and risk checks.

It is intentionally separate from research tools such as PolyAlpha. PolyAlpha
can help discover who may be worth following; PolyFollow is the execution and
audit engine.

## What You Can Do

| Need | Command |
| --- | --- |
| Create local config and SQLite database | `polyfollow setup` |
| Add smart-money wallets with per-wallet limits | `polyfollow leader add ...` |
| Import PolyAlpha follow candidates | `polyfollow leader import-polyalpha ...` |
| Run one safe paper cycle | `polyfollow run --paper --once` |
| Run continuous paper following | `polyfollow run --paper` |
| Opt in to live CLOB execution | `polyfollow run --live --confirm-live` |
| Inspect orders, live attempts, logs, PnL, and status | `orders`, `live-attempts`, `logs`, `pnl`, `status` |
| Expose local read-only API | `polyfollow serve` |
| Render a static dashboard | `polyfollow dashboard` |
| Watch CLOB websocket events | `polyfollow watch-clob` |
| Poll Polygon logs as backup | `polyfollow watch-chain` |
| Replay normalized trades offline | `polyfollow backtest` |
| Suggest portfolio-level caps | `polyfollow allocate` |
| Disable noisy leaders after repeated risk blocks | `polyfollow cooldown` |
| Fetch MarketBridge context for agents | `polyfollow marketbridge-context` |

## Install

PolyFollow is designed to be used as a single binary. For normal use, download
the matching asset from the latest GitHub Release:

```text
https://github.com/okloorcl/polyfollow/releases/latest
```

Release assets are built for:

| Platform | Assets |
| --- | --- |
| Linux | `x86_64-unknown-linux-gnu`, `i686-unknown-linux-gnu`, `armv7-unknown-linux-gnueabihf`, `aarch64-unknown-linux-gnu` |
| macOS | `aarch64-apple-darwin` |
| Windows | `x86_64-pc-windows-msvc`, `i686-pc-windows-msvc` |

Linux/macOS example:

```bash
tar -xzf polyfollow-v0.1.0-aarch64-apple-darwin.tar.gz
cd polyfollow-v0.1.0-aarch64-apple-darwin
chmod +x polyfollow
./polyfollow --help
```

Windows users can download the matching `.zip`, extract it, and run
`polyfollow.exe`.

Build from source when developing:

```bash
git clone https://github.com/okloorcl/polyfollow.git
cd polyfollow
cargo build --release
./target/release/polyfollow --help
```

During development you can run the binary through Cargo:

```bash
cargo run -- --help
cargo run -- setup
cargo run -- run --paper --once
```

The important Cargo syntax is the double dash. `cargo run -- run --paper` runs
the `polyfollow run --paper` command. Do not write `cargo run polyfollow run`.

## Quick Start

Create a config and database. For current Polymarket deposit-wallet accounts,
`wallet` is the public profile/proxy address and `funder` is the deposit address
shown in the recharge dialog:

```bash
polyfollow setup \
  --wallet 0x1111111111111111111111111111111111111111 \
  --funder 0x2222222222222222222222222222222222222222
polyfollow doctor
polyfollow config path
```

Add a leader using ratio sizing:

```bash
polyfollow leader add 0x2222222222222222222222222222222222222222 \
  --label "weather specialist" \
  --copy-ratio 0.10 \
  --max-order 20 \
  --max-daily 100 \
  --max-position 250
```

Add a fixed-size leader and ignore sells:

```bash
polyfollow leader add 0x3333333333333333333333333333333333333333 \
  --label "small fixed" \
  --fixed-order 10 \
  --max-order 10 \
  --max-daily 50 \
  --no-sell
```

Run a single paper cycle:

```bash
polyfollow run --paper --once
polyfollow run --paper
polyfollow orders
polyfollow logs
polyfollow pnl
polyfollow status
```

Print machine-readable JSON for agents:

```bash
polyfollow --json leader list
polyfollow --json run --paper --once --limit 50
polyfollow --json orders --limit 50
polyfollow --json live-attempts --limit 20
polyfollow --json pnl
```

## Live Trading

Live mode is deliberately hard to trigger by accident. All of these are
required:

```bash
export POLYFOLLOW_PRIVATE_KEY="0x..."
polyfollow run --live --confirm-live
```

Account-specific keys are supported. If a leader is assigned to account
`research`, PolyFollow checks `POLYFOLLOW_PRIVATE_KEY_RESEARCH` first, then
falls back to `POLYFOLLOW_PRIVATE_KEY`, then `POLYMARKET_PRIVATE_KEY`.

```toml
[[accounts]]
name = "research"
wallet = "0x1111111111111111111111111111111111111111" # public profile/proxy address
funder = "0x2222222222222222222222222222222222222222" # deposit wallet / recharge address
signature_type = "poly-1271"
```

`signature_type` is validated strictly. Use `poly-1271` for current Polymarket
deposit wallets, or `proxy`, `gnosis-safe`, `eoa` for older account types.
Unknown values are rejected instead of silently falling back to another signing
mode. `poly-1271`, `proxy`, and `gnosis-safe` live orders require a funder
address; PolyFollow uses `account.funder` first and falls back to
`account.wallet` for backward compatibility.

```bash
export POLYFOLLOW_PRIVATE_KEY_RESEARCH="0x..."
polyfollow leader update 0x2222222222222222222222222222222222222222 \
  --account research
polyfollow run --live --confirm-live --once
```

Private keys are read from the environment. They are not written to TOML or the
SQLite audit database.

Before live mode, run:

```bash
polyfollow doctor
```

`doctor` reports missing live wallets, missing private-key environment
variables, disabled leaders, and kill-switch state without printing secrets.
After a live run, inspect exchange responses with:

```bash
polyfollow live-attempts --limit 20
polyfollow --json live-attempts --limit 20
```

Successful live responses include the exchange order id, exchange status,
success flag, trade ids, and transaction hashes when Polymarket returns them.
Rejected exchange responses are recorded as `rejected`, not `submitted`, so
they do not inflate open live exposure.

## Command Reference

### Global Options

| Option | Meaning |
| --- | --- |
| `--config <path>` | Use a specific config file instead of `~/.config/polyfollow/config.toml` |
| `--db <path>` | Override `global.db_path` for the SQLite database |
| `--json` | Print machine-readable JSON instead of human text |

### Setup And Config

```bash
polyfollow setup
polyfollow setup \
  --wallet 0x1111111111111111111111111111111111111111 \
  --funder 0x2222222222222222222222222222222222222222
polyfollow setup --force
polyfollow config show
polyfollow config path
polyfollow doctor
```

`doctor` validates config shape, wallet addresses, database access, and live
safety conditions.

### Leader Management

```bash
polyfollow leader add <wallet> [options]
polyfollow leader update <wallet> [options]
polyfollow leader remove <wallet>
polyfollow leader list
```

Common leader options:

| Option | Meaning |
| --- | --- |
| `--label <text>` | Human label for the wallet |
| `--account <name>` | Route this leader to a configured account |
| `--copy-ratio <decimal>` | Copy leader notional by ratio, for example `0.10` |
| `--fixed-order <usdc>` | Use a fixed USDC order size per buy |
| `--max-order <usdc>` | Per-copy order cap |
| `--max-daily <usdc>` | Per-leader daily notional cap |
| `--max-position <usdc>` | Per-leader open position cap |
| `--market-allow <text>` | Only allow matching market text, repeatable |
| `--market-block <text>` | Block matching market text, repeatable |
| `--no-buy` | Do not copy buys |
| `--no-sell` | Do not copy sells |

Update-only toggles:

```bash
polyfollow leader update <wallet> --enabled false
polyfollow leader update <wallet> --support-buy false
polyfollow leader update <wallet> --support-sell true
```

### PolyAlpha Import

PolyFollow can import leader candidates from PolyAlpha JSON exports or from a
SQLite database containing `wallet_follow_scores`.

```bash
polyfollow leader import-polyalpha /path/to/polyalpha.sqlite \
  --min-score 0.80 \
  --copy-ratio 0.05 \
  --max-order 15 \
  --max-daily 75 \
  --dry-run
```

Use `--dry-run` first. Remove it only after the candidate set looks right.

### Follow Loop

```bash
polyfollow run --paper --once
polyfollow run --paper --limit 100
polyfollow run --paper --max-consecutive-errors 10
polyfollow run --mode paper
polyfollow run --live --confirm-live --once
```

The loop does this for every enabled leader:

1. Poll Polymarket activity.
2. Normalize leader trades.
3. Dedupe already processed events.
4. Enrich with CLOB order-book checks when token ids are available.
5. Apply sizing, market filters, latency, spread, depth, drift, daily, order,
   and position limits.
6. Record a copy intent.
7. Execute through paper ledger or live CLOB adapter.
8. Persist audit rows and optional notifications.

Continuous mode is daemon-friendly. A failed polling cycle is logged and the
next cycle continues. `--max-consecutive-errors 0` means never stop because of
consecutive polling errors; this is also the default through
`global.max_consecutive_errors = 0`. Use a positive value when you prefer the
process to exit after repeated failures:

```bash
polyfollow run --paper --max-consecutive-errors 10
```

Press `Ctrl-C` to stop gracefully and print the final run summary.

### State And Reports

```bash
polyfollow status
polyfollow orders --limit 50
polyfollow live-attempts --limit 20
polyfollow logs --limit 50
polyfollow pnl
```

Paper sells are matched against tracked paper buys using FIFO, so `pnl` can
show realized PnL instead of only open notional.

### Local HTTP API

Run a read-only local API for dashboards and agents:

```bash
polyfollow serve --addr 127.0.0.1:8787
```

Endpoints:

```bash
curl http://127.0.0.1:8787/health
curl http://127.0.0.1:8787/status
curl http://127.0.0.1:8787/leaders
curl 'http://127.0.0.1:8787/orders?limit=20'
curl 'http://127.0.0.1:8787/live-attempts?limit=20'
curl 'http://127.0.0.1:8787/logs?limit=20'
curl http://127.0.0.1:8787/pnl
```

The server is read-only. It does not expose HTTP trading endpoints.

### Static Dashboard

```bash
polyfollow dashboard --out polyfollow-dashboard.html --limit 50
```

This renders a local HTML snapshot from SQLite. It is useful for quick reviews
without keeping a server running.

### CLOB Websocket Watcher

```bash
polyfollow watch-clob --asset 123456789 --once
polyfollow watch-clob --asset 123 --asset 456 --json
polyfollow watch-clob --assets-file token_ids.txt --chunk-size 500
```

This subscribes to Polymarket market websocket events for token/asset ids.

### On-Chain Backup Watcher

```bash
polyfollow --json watch-chain \
  --rpc-url https://polygon-rpc.com \
  --contract 0x0000000000000000000000000000000000000000 \
  --from-block 72100000 \
  --once
```

Use this as a raw backup feed from Polygon logs when you want an independent
view of exchange activity.

### Backtesting

Replay normalized `LeaderTrade[]` JSON through the paper engine:

```bash
polyfollow --json backtest trades.json \
  --leader 0x2222222222222222222222222222222222222222
```

The leader must exist in config so the same sizing and risk rules are used.

### Allocation Optimizer

Preview or apply portfolio-level caps:

```bash
polyfollow allocate --capital 1000 --order-fraction 0.02 --daily-fraction 0.10
polyfollow allocate --capital 1000 --apply
```

This helps spread account capital across enabled leaders and keeps per-leader
risk budgets consistent.

### Cooldown Audit

Audit leaders that repeatedly fail risk checks:

```bash
polyfollow cooldown --blocked-threshold 5
polyfollow cooldown --blocked-threshold 5 --apply
```

Without `--apply`, the command only suggests changes. With `--apply`, noisy
leaders can be disabled in config.

### MarketBridge Context

Fetch market context from a local MarketBridge instance:

```bash
polyfollow --json marketbridge-context \
  --base-url http://127.0.0.1:8080 \
  --symbol BTCUSDT \
  --symbol ETHUSDT \
  --market perp
```

This is for agent workflows that combine Polymarket copy signals with broader
market context.

## Configuration

Default config path:

```bash
polyfollow config path
```

Minimal shape:

```toml
[global]
mode = "paper"
db_path = "/Users/you/.local/share/polyfollow/polyfollow.sqlite"
data_api_base_url = "https://data-api.polymarket.com"
clob_base_url = "https://clob.polymarket.com"
poll_interval_secs = 10
max_consecutive_errors = 0
max_daily_loss_usdc = "100"
max_open_positions = 50
kill_switch = false

[account]
name = "main"
wallet = "0x1111111111111111111111111111111111111111"
funder = "0x2222222222222222222222222222222222222222"
max_capital_usdc = "1000"
max_daily_loss_usdc = "50"
signature_type = "poly-1271"

[[leaders]]
address = "0x2222222222222222222222222222222222222222"
label = "weather specialist"
enabled = true

[leaders.copy]
mode = "ratio"
ratio = "0.10"
fixed_order_usdc = "10"

[leaders.risk]
max_order_usdc = "20"
max_daily_usdc = "100"
max_position_usdc = "250"
max_latency_secs = 120
max_price_drift_bps = "100"
max_spread_bps = "250"
min_depth_usdc = "50"
support_buy = true
support_sell = true

[leaders.filters]
allow = []
block = []

[notifications]
webhook_url = "https://example.com/polyfollow"
telegram_bot_token = "123456:bot-token"
telegram_chat_id = "123456789"
notify_blocked = false
```

## Safety Model

| Guard | Behavior |
| --- | --- |
| Paper default | `setup` and normal `run` behavior stay in paper mode unless changed |
| Live confirmation | Live requires `--live --confirm-live` |
| Daemon-friendly loop | Continuous `run` logs failed cycles and keeps polling by default |
| Environment keys | Private keys are read from env vars only |
| Kill switch | `global.kill_switch = true` blocks new copy execution |
| Global caps | `max_open_positions` and the stricter global/account daily-loss cap block new buys |
| Per-leader caps | Max order, daily notional, and open position caps |
| Market filters | Allow/block text filters per leader |
| Market quality checks | Price drift, spread, and depth checks when order-book data exists |
| FIFO sells | Paper sell PnL is matched against tracked paper buys |
| Full audit | Observed trades, intents, fills, live attempts, and blocks go to SQLite |

## Architecture

```text
Polymarket Data API
Polymarket CLOB websocket
Polygon RPC logs
PolyAlpha exports
MarketBridge context
        |
        v
monitor + watchers + importers
        |
        v
normalization + dedupe
        |
        v
CLOB market enrichment
        |
        v
sizing engine + risk engine
        |
        v
copy intent
        |
        +--> paper executor -> FIFO ledger -> PnL
        |
        +--> live executor  -> Polymarket CLOB SDK
        |
        v
SQLite audit database
        |
        +--> CLI text/JSON
        +--> read-only HTTP API
        +--> static dashboard
        +--> webhook/Telegram notifications
```

## Project Layout

```text
src/main.rs         binary entrypoint
src/cli/            focused Clap command modules
src/app/            command orchestration, reports, and follow loop
src/config/         TOML config model, defaults, storage, and validation
src/engine/         sizing and risk decision modules
src/execution.rs    paper/live execution adapters
src/market.rs       CLOB order-book enrichment
src/monitor.rs      Polymarket Data API polling
src/watch.rs        CLOB websocket watcher
src/chain.rs        Polygon log polling
src/storage/        SQLite schema, rows, paper ledger, risk, and audit access
src/server.rs       local read-only HTTP API
src/dashboard.rs    static HTML dashboard rendering
src/backtest.rs     offline replay engine
src/allocation.rs   portfolio-level cap suggestions
src/cooldown.rs     blocked-leader audit
src/polyalpha.rs    PolyAlpha import support
src/marketbridge.rs MarketBridge context fetcher
```

## Tech Stack

| Layer | Technology |
| --- | --- |
| Language | Rust 2024 |
| Async runtime | Tokio |
| CLI | Clap derive |
| Config | TOML, Serde |
| Storage | SQLite via Rusqlite bundled |
| HTTP client | Reqwest with Rustls |
| HTTP server | Axum |
| Websocket | tokio-tungstenite |
| Money math | rust_decimal |
| Polymarket execution | official `polymarket-client-sdk` with CLOB feature |
| Hashing / chain helpers | tiny-keccak |
| Agent output | stable JSON via `--json` |

## Data Sources

| Source | Used For |
| --- | --- |
| Polymarket Data API | Leader activity polling |
| Polymarket CLOB REST/order book | Spread, depth, drift checks and live execution |
| Polymarket CLOB websocket | Token/asset event watcher |
| Polygon JSON-RPC logs | On-chain backup monitoring |
| PolyAlpha JSON/SQLite | Candidate leader import |
| MarketBridge HTTP | Optional market context enrichment |

## Output Philosophy

PolyFollow is built for both humans and agents:

- Human commands default to readable text.
- `--json` prints stable machine-readable responses.
- The local HTTP API is read-only and agent-safe.
- Every trade decision keeps enough context to audit later.
- Live execution is never hidden behind an HTTP endpoint.

## Development

Run the verification suite:

```bash
cargo fmt --check
cargo check
cargo test
```

Build an optimized binary:

```bash
cargo build --release
./target/release/polyfollow --help
```

The release profile enables thin LTO, one codegen unit, symbol stripping, and
`panic = "abort"` for smaller optimized binaries.

CI runs formatting, clippy, tests, and multi-target builds on every push and
pull request. GitHub Releases are created from version tags:

```bash
git tag v0.1.0
git push origin v0.1.0
```

The release workflow uploads `.tar.gz` archives for Linux/macOS, `.zip`
archives for Windows, and a `.sha256` checksum for every artifact.

## Roadmap

See [PLAN.md](PLAN.md). The current roadmap has completed:

- P0: config, leaders, SQLite audit, polling, dedupe, sizing, risk, paper loop,
  guarded live execution.
- P1: CLOB websocket watcher, on-chain backup watcher, FIFO PnL, multi-account
  keys, notifications, PolyAlpha import, local HTTP API.
- P2: static dashboard, backtesting, cooldown audit, allocation optimizer,
  MarketBridge context, daemon-friendly run loop.

Future work can focus on release automation, richer dashboards, more exchange
telemetry, and deeper strategy evaluation.

## Disclaimer

PolyFollow is software for research and automation. Prediction markets are
risky, liquidity can vanish, copied traders can be wrong, and live execution can
lose money. Start in paper mode, use tiny caps, and understand every configured
leader before enabling live trading.
