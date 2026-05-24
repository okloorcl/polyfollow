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
      `orders`, `pnl`, `logs`, `doctor`, `config show`.
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
- [x] README with simple “download and run” style usage.

## P1 Scope

- [x] Native live Polymarket CLOB order execution using the official Rust SDK.
- [x] CLOB market websocket monitor for token/asset ids.
- [ ] On-chain websocket backup monitor.
- [x] FIFO sell matching for paper positions and realized PnL.
- [x] Multi-account leader allocation with per-account live key env lookup.
- [x] Webhook/Telegram notifications for copy intents.
- [x] Import leader scores from PolyAlpha JSON exports or SQLite.
- [x] Local read-only HTTP API for status, leaders, orders, logs, and PnL.

## P2 Scope

- [ ] Local dashboard.
- [ ] Backtesting for leader follow configs.
- [ ] Automatic leader cooldown/demotion.
- [ ] Portfolio-level allocation optimizer.
- [ ] MarketBridge realtime enrichment.

## References Studied

- `copysignal/polymarket-copy-trading-bot`: minimal TypeScript polling and CLOB
  copy flow; useful but not safe enough as-is.
- `copysignal/PolyHermes`: mature risk/template/dual-monitoring ideas; too heavy
  for a single-binary CLI.
- `polymarket-cli`: official Rust CLI and best reference for wallet config, CLOB
  signing, orders, balances, spreads, books, and cancellations.
