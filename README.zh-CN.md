# PolyFollow

<p align="center">
  <strong>自托管、默认 paper 模式的 Polymarket 跟单执行引擎。</strong>
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

PolyFollow 用来跟踪一个或多个 Polymarket 领头钱包，把他们的新交易转换成你的跟单意图，经过严格的账户级和 leader 级风控之后执行。默认是 paper trading，也就是只模拟、不真钱下单。Live 模式已经接入，但必须显式传入确认参数，并且私钥只从环境变量读取。

它和 PolyAlpha 这类研究工具是分开的：PolyAlpha 更适合发现“谁可能值得跟”，PolyFollow 负责执行、风控、审计和复盘。

## 它能做什么

| 需求 | 命令 |
| --- | --- |
| 创建本地配置和 SQLite 数据库 | `polyfollow setup` |
| 添加聪明钱钱包，并设置单独风控 | `polyfollow leader add ...` |
| 从 PolyAlpha 导入候选地址 | `polyfollow leader import-polyalpha ...` |
| 安全跑一次 paper 跟单 | `polyfollow run --paper --once` |
| 连续 paper 跟单 | `polyfollow run --paper` |
| 显式开启真钱 live 跟单 | `polyfollow run --live --confirm-live` |
| 查看订单、日志、PnL、状态 | `orders`, `logs`, `pnl`, `status` |
| 开启本地只读 HTTP API | `polyfollow serve` |
| 生成本地静态仪表盘 | `polyfollow dashboard` |
| 监听 CLOB websocket 事件 | `polyfollow watch-clob` |
| 使用 Polygon 日志作为备用监控 | `polyfollow watch-chain` |
| 离线回放标准化交易 | `polyfollow backtest` |
| 给多个 leader 自动分配资金上限 | `polyfollow allocate` |
| 审计并禁用频繁触发风控的 leader | `polyfollow cooldown` |
| 获取 MarketBridge 上下文给 agent 用 | `polyfollow marketbridge-context` |

## 安装

PolyFollow 设计成单二进制应用。普通用户优先从最新 GitHub Release 下载对应平台的二进制：

```text
https://github.com/okloorcl/polyfollow/releases/latest
```

Release 会自动构建这些平台：

| 平台 | 架构 |
| --- | --- |
| Linux | `x86_64-unknown-linux-gnu`, `i686-unknown-linux-gnu`, `armv7-unknown-linux-gnueabihf`, `aarch64-unknown-linux-gnu` |
| macOS | `aarch64-apple-darwin` |
| Windows | `x86_64-pc-windows-msvc`, `i686-pc-windows-msvc` |

Linux/macOS 示例：

```bash
tar -xzf polyfollow-v0.1.0-aarch64-apple-darwin.tar.gz
cd polyfollow-v0.1.0-aarch64-apple-darwin
chmod +x polyfollow
./polyfollow --help
```

Windows 用户下载对应 `.zip`，解压后运行 `polyfollow.exe`。

开发时再从源码构建：

```bash
git clone https://github.com/okloorcl/polyfollow.git
cd polyfollow
cargo build --release
./target/release/polyfollow --help
```

开发时也可以直接用 Cargo 跑：

```bash
cargo run -- --help
cargo run -- setup
cargo run -- run --paper --once
```

注意 Cargo 的写法：`cargo run -- run --paper` 等价于执行 `polyfollow run --paper`。中间必须有 `--`，不要写成 `cargo run polyfollow run`。

## 快速开始

创建配置和数据库：

```bash
polyfollow setup --wallet 0x1111111111111111111111111111111111111111
polyfollow doctor
polyfollow config path
```

添加一个按比例跟单的 leader：

```bash
polyfollow leader add 0x2222222222222222222222222222222222222222 \
  --label "weather specialist" \
  --copy-ratio 0.10 \
  --max-order 20 \
  --max-daily 100 \
  --max-position 250
```

含义：

| 参数 | 作用 |
| --- | --- |
| `--copy-ratio 0.10` | leader 买 100 USDC，你复制 10 USDC |
| `--max-order 20` | 单笔最多 20 USDC |
| `--max-daily 100` | 这个 leader 每天最多复制 100 USDC |
| `--max-position 250` | 这个 leader 相关持仓最多 250 USDC |

添加一个固定金额跟单、忽略卖出的 leader：

```bash
polyfollow leader add 0x3333333333333333333333333333333333333333 \
  --label "small fixed" \
  --fixed-order 10 \
  --max-order 10 \
  --max-daily 50 \
  --no-sell
```

跑一次 paper：

```bash
polyfollow run --paper --once
polyfollow orders
polyfollow logs
polyfollow pnl
polyfollow status
```

给 agent 或脚本使用 JSON：

```bash
polyfollow --json leader list
polyfollow --json run --paper --once --limit 50
polyfollow --json orders --limit 50
polyfollow --json live-attempts --limit 20
polyfollow --json pnl
```

## Live 真钱下单

Live 模式不会被误触发，必须同时满足：

```bash
export POLYFOLLOW_PRIVATE_KEY="0x..."
polyfollow run --live --confirm-live
```

如果你配置了多个账户，也可以使用账户专属私钥。比如 leader 被分配到 `research` 账户，PolyFollow 会优先读取 `POLYFOLLOW_PRIVATE_KEY_RESEARCH`，然后才回退到 `POLYFOLLOW_PRIVATE_KEY` 和 `POLYMARKET_PRIVATE_KEY`。

```toml
[[accounts]]
name = "research"
wallet = "0x1111111111111111111111111111111111111111"
signature_type = "proxy"
```

`signature_type` 会严格校验，只允许 `proxy`、`gnosis-safe` 或 `eoa`。
未知值会直接报错，不会静默回退到其他签名模式。

```bash
export POLYFOLLOW_PRIVATE_KEY_RESEARCH="0x..."
polyfollow leader update 0x2222222222222222222222222222222222222222 \
  --account research
polyfollow run --live --confirm-live --once
```

私钥只从环境变量读取，不写入 TOML 配置，也不会写入 SQLite 审计数据库。

Live 前先运行：

```bash
polyfollow doctor
```

`doctor` 会报告缺失的钱包、私钥环境变量、没有启用的 leader、kill switch
状态，但不会打印私钥内容。Live 后可以查看交易所响应：

```bash
polyfollow live-attempts --limit 20
polyfollow --json live-attempts --limit 20
```

成功响应会包含 exchange order id、exchange status、success flag、trade ids
以及 Polymarket 返回的 transaction hashes。交易所返回 `success=false` 时会
记录为 `rejected`，不会记为 `submitted`，因此不会污染 live open exposure。

## 命令说明

### 全局参数

| 参数 | 含义 |
| --- | --- |
| `--config <path>` | 指定配置文件，默认是 `~/.config/polyfollow/config.toml` |
| `--db <path>` | 覆盖 `global.db_path` 指定的 SQLite 路径 |
| `--json` | 输出机器可读 JSON |

### 初始化和配置

```bash
polyfollow setup
polyfollow setup --wallet 0x1111111111111111111111111111111111111111
polyfollow setup --force
polyfollow config show
polyfollow config path
polyfollow doctor
```

`doctor` 会检查配置结构、钱包地址、数据库访问和 live 安全条件。

### Leader 管理

```bash
polyfollow leader add <wallet> [options]
polyfollow leader update <wallet> [options]
polyfollow leader remove <wallet>
polyfollow leader list
```

常用 leader 参数：

| 参数 | 含义 |
| --- | --- |
| `--label <text>` | 钱包备注名 |
| `--account <name>` | 把这个 leader 路由到某个账户 |
| `--copy-ratio <decimal>` | 按 leader 名义金额比例跟单，例如 `0.10` |
| `--fixed-order <usdc>` | 每次买入固定复制多少 USDC |
| `--max-order <usdc>` | 单笔复制上限 |
| `--max-daily <usdc>` | 每日复制上限 |
| `--max-position <usdc>` | open position 上限 |
| `--market-allow <text>` | 只允许匹配文本的市场，可重复传入 |
| `--market-block <text>` | 屏蔽匹配文本的市场，可重复传入 |
| `--no-buy` | 不复制买入 |
| `--no-sell` | 不复制卖出 |

只在 update 中使用的开关：

```bash
polyfollow leader update <wallet> --enabled false
polyfollow leader update <wallet> --support-buy false
polyfollow leader update <wallet> --support-sell true
```

### 从 PolyAlpha 导入

PolyFollow 可以读取 PolyAlpha 的 JSON 导出，也可以读取包含 `wallet_follow_scores` 的 SQLite 数据库。

```bash
polyfollow leader import-polyalpha /path/to/polyalpha.sqlite \
  --min-score 0.80 \
  --copy-ratio 0.05 \
  --max-order 15 \
  --max-daily 75 \
  --dry-run
```

建议先用 `--dry-run` 预览候选地址，确认没问题之后再真正写入配置。

### 跟单循环

```bash
polyfollow run --paper --once
polyfollow run --paper --limit 100
polyfollow run --mode paper
polyfollow run --live --confirm-live --once
```

每一轮会执行：

1. 拉取每个 enabled leader 的 Polymarket 活动。
2. 标准化交易。
3. 去重，避免重复复制。
4. 如果有 token id，则补充 CLOB order book 检查。
5. 应用 sizing、市场过滤、延迟、spread、depth、price drift、每日上限、单笔上限和持仓上限。
6. 写入 copy intent。
7. 使用 paper ledger 或 live CLOB adapter 执行。
8. 写入 SQLite 审计记录，并可选发送通知。

### 状态和报表

```bash
polyfollow status
polyfollow orders --limit 50
polyfollow live-attempts --limit 20
polyfollow logs --limit 50
polyfollow pnl
```

Paper 模式的卖出会按 FIFO 匹配之前的 paper 买入，所以 `pnl` 能展示 realized PnL，而不只是 open notional。

### 本地 HTTP API

启动只读 API：

```bash
polyfollow serve --addr 127.0.0.1:8787
```

接口：

```bash
curl http://127.0.0.1:8787/health
curl http://127.0.0.1:8787/status
curl http://127.0.0.1:8787/leaders
curl 'http://127.0.0.1:8787/orders?limit=20'
curl 'http://127.0.0.1:8787/live-attempts?limit=20'
curl 'http://127.0.0.1:8787/logs?limit=20'
curl http://127.0.0.1:8787/pnl
```

HTTP API 是只读的，不提供 HTTP 下单端点。

### 静态 Dashboard

```bash
polyfollow dashboard --out polyfollow-dashboard.html --limit 50
```

它会从 SQLite 渲染一个本地 HTML 快照，适合快速查看状态，不需要一直开 server。

### CLOB Websocket 监听

```bash
polyfollow watch-clob --asset 123456789 --once
polyfollow watch-clob --asset 123 --asset 456 --json
polyfollow watch-clob --assets-file token_ids.txt --chunk-size 500
```

用于监听指定 Polymarket token/asset id 的 market websocket 事件。

### Polygon 链上备用监听

```bash
polyfollow --json watch-chain \
  --rpc-url https://polygon-rpc.com \
  --contract 0x0000000000000000000000000000000000000000 \
  --from-block 72100000 \
  --once
```

这个命令适合作为原始链上日志备份源，帮助你独立观察交易所合约事件。

### Backtest 回放

用标准化的 `LeaderTrade[]` JSON 离线回放 paper engine：

```bash
polyfollow --json backtest trades.json \
  --leader 0x2222222222222222222222222222222222222222
```

对应 leader 必须已经存在于配置中，这样回放时才能使用同一套 sizing 和 risk 规则。

### 资金分配优化

预览或应用 portfolio-level caps：

```bash
polyfollow allocate --capital 1000 --order-fraction 0.02 --daily-fraction 0.10
polyfollow allocate --capital 1000 --apply
```

这个命令用于把账户资金分配到多个 enabled leader，并保持每个 leader 的风险预算一致。

### Cooldown 审计

检查频繁被风控拦截的 leader：

```bash
polyfollow cooldown --blocked-threshold 5
polyfollow cooldown --blocked-threshold 5 --apply
```

不加 `--apply` 只给建议；加上 `--apply` 会把噪音 leader 写回配置为 disabled。

### MarketBridge 上下文

从本地 MarketBridge 获取市场上下文：

```bash
polyfollow --json marketbridge-context \
  --base-url http://127.0.0.1:8080 \
  --symbol BTCUSDT \
  --symbol ETHUSDT \
  --market perp
```

适合 agent 工作流：把 Polymarket 跟单信号和更广泛的行情上下文放在一起分析。

## 配置文件

查看默认配置路径：

```bash
polyfollow config path
```

最小结构示例：

```toml
[global]
mode = "paper"
db_path = "/Users/you/.local/share/polyfollow/polyfollow.sqlite"
data_api_base_url = "https://data-api.polymarket.com"
clob_base_url = "https://clob.polymarket.com"
poll_interval_secs = 10
max_daily_loss_usdc = "100"
max_open_positions = 50
kill_switch = false

[account]
name = "main"
wallet = "0x1111111111111111111111111111111111111111"
max_capital_usdc = "1000"
max_daily_loss_usdc = "50"
signature_type = "proxy"

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

## 安全模型

| 保护 | 行为 |
| --- | --- |
| 默认 paper | `setup` 和普通 `run` 默认不真钱下单 |
| Live 二次确认 | Live 必须显式 `--live --confirm-live` |
| 环境变量私钥 | 私钥只从环境变量读取 |
| Kill switch | `global.kill_switch = true` 会阻止新的复制执行 |
| 全局上限 | `max_open_positions` 以及 global/account 中更严格的 daily-loss 上限会阻止新的买入 |
| Leader 级上限 | 单笔、每日、持仓都有独立上限 |
| 市场过滤 | 每个 leader 可以配置 allow/block |
| 市场质量检查 | 有 order book 数据时检查 drift、spread、depth |
| FIFO 卖出 | Paper 卖出按 FIFO 匹配买入并计算 realized PnL |
| 完整审计 | observed trade、intent、fill、live attempt、blocked reason 都进 SQLite |

## 架构

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

## 项目结构

```text
src/main.rs         二进制入口
src/cli/            Clap 命令模块
src/app/            命令编排、报表和跟单循环
src/config/         TOML 配置、默认值、存储和校验
src/engine/         sizing 和 risk 决策模块
src/execution.rs    paper/live 执行适配器
src/market.rs       CLOB order book 补充
src/monitor.rs      Polymarket Data API 轮询
src/watch.rs        CLOB websocket watcher
src/chain.rs        Polygon log polling
src/storage/        SQLite schema、rows、paper ledger、risk、audit access
src/server.rs       本地只读 HTTP API
src/dashboard.rs    静态 HTML dashboard
src/backtest.rs     离线回放引擎
src/allocation.rs   portfolio-level caps 建议
src/cooldown.rs     blocked leader 审计
src/polyalpha.rs    PolyAlpha 导入
src/marketbridge.rs MarketBridge context fetcher
```

## 技术栈

| 层 | 技术 |
| --- | --- |
| 语言 | Rust 2024 |
| 异步运行时 | Tokio |
| CLI | Clap derive |
| 配置 | TOML, Serde |
| 存储 | SQLite via Rusqlite bundled |
| HTTP client | Reqwest with Rustls |
| HTTP server | Axum |
| Websocket | tokio-tungstenite |
| 金额计算 | rust_decimal |
| Polymarket 执行 | official `polymarket-client-sdk` with CLOB feature |
| 链上辅助 | tiny-keccak |
| Agent 输出 | `--json` stable JSON |

## 数据源

| 来源 | 用途 |
| --- | --- |
| Polymarket Data API | leader 活动轮询 |
| Polymarket CLOB REST/order book | spread、depth、drift 检查和 live 执行 |
| Polymarket CLOB websocket | token/asset 事件监听 |
| Polygon JSON-RPC logs | 链上备用监控 |
| PolyAlpha JSON/SQLite | 候选 leader 导入 |
| MarketBridge HTTP | 可选市场上下文补充 |

## 输出设计

PolyFollow 同时面向人和 agent：

- 人类命令默认输出可读文本。
- `--json` 输出稳定机器可读结构。
- 本地 HTTP API 只读，适合 dashboard 和 agent。
- 每个交易决策都保留审计上下文。
- Live 执行不会隐藏在 HTTP endpoint 后面。

## 开发

运行验证：

```bash
cargo fmt --check
cargo check
cargo test
```

构建优化二进制：

```bash
cargo build --release
./target/release/polyfollow --help
```

Release profile 已启用 thin LTO、单 codegen unit、strip 和 `panic = "abort"`，用于生成更小的优化二进制。

CI 会在 push 和 pull request 时执行 formatting、clippy、tests 和多 target build。GitHub Release 由版本 tag 触发：

```bash
git tag v0.1.0
git push origin v0.1.0
```

Release workflow 会上传 Linux/macOS 的 `.tar.gz`、Windows 的 `.zip`，并为每个 artifact 生成 `.sha256` 校验文件。

## 路线图

见 [PLAN.md](PLAN.md)。当前已完成：

- P0：配置、leader、SQLite 审计、轮询、去重、sizing、risk、paper loop、受保护的 live execution。
- P1：CLOB websocket watcher、链上备用 watcher、FIFO PnL、多账户私钥、通知、PolyAlpha 导入、本地 HTTP API。
- P2：静态 dashboard、backtesting、cooldown audit、allocation optimizer、MarketBridge context。

后续可以继续增强 release 自动化、dashboard、交易所 telemetry 和策略评估。

## 免责声明

PolyFollow 是研究和自动化软件。预测市场有风险，流动性可能突然消失，被复制的交易者也可能出错，live execution 可能造成亏损。请先使用 paper 模式，用很小的资金上限测试，并在真正 live 前理解每一个 leader 的风险。
