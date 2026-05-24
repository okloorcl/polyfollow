# PolyFollow

Self-custody Polymarket copy-trading engine.

PolyFollow tracks one or more leader wallets, converts their new trades into
copy intents, applies strict risk controls, and executes them in paper mode by
default. Live trading is opt-in and requires explicit confirmation.

## Current Status

This project is under active implementation. P0 focuses on a safe paper-trading
closed loop before enabling real orders.

## Intended Usage

```bash
polyfollow setup
polyfollow leader add 0xabc... \
  --label "weather specialist" \
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

Live trading will stay blocked unless invoked explicitly:

```bash
polyfollow run --live --confirm-live
```

Live execution is intentionally blocked in the current milestone. Paper mode
already polls the Polymarket Data API, normalizes leader trades, deduplicates
them, builds copy intents, and records paper fills.

## Design

```text
Polymarket Data API
  -> leader activity poller
  -> normalized trade events
  -> SQLite dedupe
  -> sizing + risk engine
  -> paper/live executor
  -> audit log + reports
```

See [PLAN.md](PLAN.md) for the implementation roadmap.

## Configuration Model

PolyFollow uses one TOML config file plus one SQLite database:

- Config: global mode and risk, account wallet, per-leader sizing/risk.
- SQLite: observed trades, dedupe state, copy intents, paper fills, live attempts.

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
