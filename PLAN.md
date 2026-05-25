# PolyFollow Plan

PolyFollow is a self-custody Polymarket copy-trading engine. It is intentionally
separate from PolyAlpha: PolyAlpha researches who may be worth following;
PolyFollow executes paper/live follow decisions with strict risk controls.

## Product Principles

- One downloadable Rust binary.
- CLI first; JSON output for agents.
- Paper trading is the default.
- Live trading requires explicit `--live --confirm-live`.
- Every observed trade, skipped trade, intent, paper fill, and live order must be
  auditable in SQLite.
- Private keys must not be logged or stored in the trade database.
- Money is represented with decimal types, not floating point.

## Architecture

```text
leader sources
  -> activity poller / future websocket / future on-chain watcher
  -> trade normalizer
  -> dedupe
  -> market/orderbook enricher
  -> sizing engine
  -> risk engine
  -> copy intent
  -> paper executor / live executor
  -> audit log + pnl
```

## P0 Scope

- [x] Project scaffold and product plan.
- [x] TOML configuration with global, account, and per-leader controls.
- [x] CLI commands: `setup`, `leader add/list/remove/update`, `run`, `status`,
      `orders`, `live-attempts`, `pnl`, `logs`, `doctor`, `config show`.
- [x] SQLite storage for leaders, processed trades, intents, paper fills, live
      order attempts, and risk state.
- [x] Polymarket Data API activity polling for multiple leaders.
- [x] Trade normalization and dedupe.
- [x] Per-leader sizing: `ratio` and `fixed`.
- [x] Risk engine.
      - [x] max order, max daily, max position, max latency, buy/sell toggles,
            market allow/block filters, kill switch.
      - [x] price drift, min depth, max spread with CLOB order-book enrichment
            when a token id is available.
- [x] Paper executor and PnL/status reports.
- [x] Native live Polymarket CLOB market-order executor using the official Rust
      SDK, environment private key, and explicit `--live --confirm-live`.
- [x] Structured live exchange response audit with order id, status, success,
      trade ids, and transaction hashes.
- [x] README with simple "download and run" style usage.

## P1 Scope

- [x] Native live Polymarket CLOB order execution using the official Rust SDK.
- [x] CLOB market websocket monitor for token/asset ids.
- [x] On-chain raw log backup monitor for Polymarket exchange contracts.
- [x] FIFO sell matching for paper positions and realized PnL.
- [x] Multi-account leader allocation with per-account live key env lookup.
- [x] Live readiness diagnostics for wallets, env keys, enabled leaders, and
      kill-switch state.
- [x] Webhook/Telegram notifications for copy intents.
- [x] Import leader scores from PolyAlpha JSON exports or SQLite.
- [x] Local read-only HTTP API for status, leaders, orders, logs, and PnL.

## P2 Scope

- [x] Static local HTML dashboard from SQLite state.
- [x] Offline backtesting for normalized leader trade JSON.
- [x] Leader cooldown audit with optional config demotion.
- [x] Portfolio-level allocation optimizer with optional config apply.
- [x] MarketBridge agent context enrichment command.
- [x] GitHub Actions CI for formatting, clippy, tests, and multi-target builds.
- [x] Tag-driven GitHub Release workflow for multi-platform binaries and
      checksums.

## References Studied

- `copysignal/polymarket-copy-trading-bot`: minimal TypeScript polling and CLOB
  copy flow; useful but not safe enough as-is.
- `copysignal/PolyHermes`: mature risk/template/dual-monitoring ideas; too heavy
  for a single-binary CLI.
- `polymarket-cli`: official Rust CLI and best reference for wallet config, CLOB
  signing, orders, balances, spreads, books, and cancellations.
