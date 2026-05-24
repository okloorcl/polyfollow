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
polyfollow leader add 0xabc... --label "weather specialist" --copy-ratio 0.10 --max-order 20
polyfollow leader add 0xdef... --label "small fixed" --fixed-order 10 --no-sell
polyfollow run --paper
polyfollow status
polyfollow pnl
```

Live trading will stay blocked unless invoked explicitly:

```bash
polyfollow run --live --confirm-live
```

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

