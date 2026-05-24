# PolyFollow

Self-custody Polymarket copy-trading engine.

PolyFollow tracks one or more leader wallets, converts their new trades into
copy intents, applies strict risk controls, and executes them in paper mode by
default. Live trading is opt-in and requires explicit confirmation.

## Current Status

This project is under active implementation. The safe paper-trading loop is
usable now. Native live execution is wired through the official Polymarket Rust
SDK, but it remains opt-in and guarded by explicit flags plus an environment
private key.

## Intended Usage

```bash
polyfollow setup
polyfollow leader add 0xabc... \
  --label "weather specialist" \
  --account main \
  --copy-ratio 0.10 \
  --max-order 20 \
  --max-daily 100

polyfollow leader add 0xdef... \
  --label "small fixed" \
  --fixed-order 10 \
  --no-sell

polyfollow leader list
polyfollow run --paper --once
polyfollow status
polyfollow pnl
```

Import leader candidates from PolyAlpha:

```bash
polyfollow leader import-polyalpha /path/to/polyalpha/data/oktrader.sqlite \
  --min-score 0.80 \
  --copy-ratio 0.05 \
  --max-order 15 \
  --max-daily 75 \
  --dry-run
```

Supported PolyAlpha inputs are JSON exports with wallet/account fields or a
SQLite database containing `wallet_follow_scores`.

Live trading will stay blocked unless invoked explicitly:

```bash
export POLYFOLLOW_PRIVATE_KEY="0x..."
polyfollow run --live --confirm-live --once
```

Paper mode polls the Polymarket Data API, normalizes leader trades,
deduplicates them, enriches with CLOB order-book checks when token ids are
available, builds copy intents, and records paper fills. Live mode submits
market FAK orders to the Polymarket CLOB only after all risk checks pass.
Paper sells are matched against tracked paper buys using FIFO, so `pnl` can
show realized PnL instead of only open notional.

## Design

```text
src/app/          command orchestration and follow loop
src/cli.rs        clap command definitions
src/config.rs     TOML config model and validation
src/engine.rs     sizing and risk decisions
src/execution.rs  paper/live execution adapters
src/market.rs     CLOB order-book enrichment
src/monitor.rs    Polymarket Data API trade polling
src/storage/      SQLite schema, audit rows, and paper ledger
src/server.rs     local read-only HTTP API
src/watch.rs      CLOB websocket watcher
```

See [PLAN.md](PLAN.md) for the implementation roadmap.

## Configuration Model

PolyFollow uses one TOML config file plus one SQLite database:

- Config: global mode and risk, account wallet, per-leader sizing/risk.
- SQLite: observed trades, dedupe state, copy intents, paper fills, live attempts.
- Live key: `POLYFOLLOW_PRIVATE_KEY` or `POLYMARKET_PRIVATE_KEY`. Keys are read
  from the environment and are not written to the config or SQLite database.

Default paths:

```bash
polyfollow config path
```

Example leader controls:

```bash
polyfollow leader add 0x2222222222222222222222222222222222222222 \
  --label smart1 \
  --copy-ratio 0.2 \
  --max-order 25 \
  --max-daily 100 \
  --no-sell
```

Human output is the default. Add `--json` for agents:

```bash
polyfollow --json leader list
polyfollow --json run --paper --once --limit 50
polyfollow --json orders
polyfollow --json logs
polyfollow --json status
```

## Local HTTP API

The local API is read-only in this milestone, so dashboards and agents can read
state without receiving an HTTP trading endpoint.

```bash
polyfollow serve --addr 127.0.0.1:8787
curl http://127.0.0.1:8787/health
curl http://127.0.0.1:8787/status
curl 'http://127.0.0.1:8787/orders?limit=20'
curl 'http://127.0.0.1:8787/logs?limit=20'
curl http://127.0.0.1:8787/pnl
```

Render a static dashboard without running a server:

```bash
polyfollow dashboard --out polyfollow-dashboard.html --limit 50
```

Replay normalized `LeaderTrade[]` JSON through the paper engine:

```bash
polyfollow --json backtest trades.json \
  --leader 0x2222222222222222222222222222222222222222
```

Preview or apply portfolio-level leader caps:

```bash
polyfollow allocate --capital 1000 --order-fraction 0.02 --daily-fraction 0.10
polyfollow allocate --capital 1000 --apply
```

Audit or disable leaders that repeatedly fail risk checks:

```bash
polyfollow cooldown --blocked-threshold 5
polyfollow cooldown --blocked-threshold 5 --apply
```

Fetch live context from a local MarketBridge instance:

```bash
polyfollow --json marketbridge-context \
  --base-url http://127.0.0.1:8080 \
  --symbol BTCUSDT \
  --symbol ETHUSDT \
  --market perp
```

## CLOB Websocket Watcher

Watch live order-book events for one or more Polymarket token ids:

```bash
polyfollow watch-clob --asset 123456789 --once --json
polyfollow watch-clob --assets-file token_ids.txt
```

## On-Chain Backup Watcher

When you want a raw backup feed from Polygon itself, poll OrderFilled logs from
the Polymarket exchange contract:

```bash
polyfollow --json watch-chain \
  --rpc-url https://polygon-rpc.com \
  --contract 0x... \
  --from-block 72100000 \
  --once
```

## Live Safety

Live trading requires all of these:

```bash
export POLYFOLLOW_PRIVATE_KEY="0x..."
polyfollow run --live --confirm-live
```

Optional account signature type in `config.toml`:

```toml
[account]
signature_type = "proxy" # proxy, eoa, or gnosis-safe

[[accounts]]
name = "research"
wallet = "0x..."
signature_type = "proxy"
```

Leaders can be assigned to an account with `--account research`. Live mode looks
for account-specific private keys first, for example
`POLYFOLLOW_PRIVATE_KEY_RESEARCH`, then falls back to `POLYFOLLOW_PRIVATE_KEY`
and `POLYMARKET_PRIVATE_KEY`.

## Notifications

Notifications are optional and best-effort. A failed notification logs a warning
but does not stop the follow loop.

```toml
[notifications]
webhook_url = "https://example.com/polyfollow"
telegram_bot_token = "123456:bot-token"
telegram_chat_id = "123456789"
notify_blocked = false
```
