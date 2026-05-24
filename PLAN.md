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
- [ ] Polymarket Data API activity polling for multiple leaders.
- [ ] Trade normalization and dedupe.
- [ ] Per-leader sizing: `ratio` and `fixed`.
- [ ] Risk engine: max order, max daily, max position, max latency, price drift,
      min depth, max spread, buy/sell toggles, kill switch.
- [ ] Paper executor and PnL/status reports.
- [ ] Live executor skeleton using the same intent contract, blocked unless
      explicitly confirmed.
- [ ] README with simple “download and run” style usage.

## P1 Scope

- [ ] Native live Polymarket CLOB order execution using the official Rust SDK.
- [ ] CLOB websocket or activity websocket monitor.
- [ ] On-chain websocket backup monitor.
- [ ] FIFO sell matching.
- [ ] Multi-account allocation.
- [ ] Webhook/Telegram notifications.
- [ ] Import leader scores from PolyAlpha.
- [ ] Local HTTP API.

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
