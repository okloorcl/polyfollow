use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "polyfollow",
    version,
    about = "Self-custody Polymarket copy-trading engine"
)]
pub struct Cli {
    /// Config file path. Defaults to ~/.config/polyfollow/config.toml.
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,

    /// SQLite database path. Overrides config.global.db_path.
    #[arg(long, global = true)]
    pub db: Option<PathBuf>,

    /// Print machine-readable JSON.
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Create a safe default config and initialize SQLite.
    Setup(SetupArgs),
    /// Inspect config.
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    /// Manage leader wallets.
    Leader {
        #[command(subcommand)]
        command: LeaderCommand,
    },
    /// Show runtime/storage status.
    Status,
    /// Validate config, database, and safety settings.
    Doctor,
    /// Start paper or live follow loop.
    Run(RunArgs),
    /// List recent copy intents.
    Orders(LimitArgs),
    /// Show paper/live PnL summary.
    Pnl,
    /// Show recent observed leader trades.
    Logs(LimitArgs),
    /// Start a local read-only HTTP API.
    Serve(ServeArgs),
    /// Watch Polymarket CLOB market websocket events for token ids.
    WatchClob(WatchClobArgs),
    /// Poll Polygon logs as an on-chain backup feed.
    WatchChain(WatchChainArgs),
    /// Render a static local HTML dashboard from SQLite.
    Dashboard(DashboardArgs),
    /// Replay normalized LeaderTrade JSON through paper execution.
    Backtest(BacktestArgs),
    /// Suggest or apply portfolio-level leader risk allocations.
    Allocate(AllocateArgs),
    /// Audit blocked leaders and optionally disable noisy leaders.
    Cooldown(CooldownArgs),
}

#[derive(Debug, Args)]
pub struct SetupArgs {
    /// Your proxy wallet address. Can be set later.
    #[arg(long)]
    pub wallet: Option<String>,

    /// Overwrite an existing config file.
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Print the effective config.
    Show,
    /// Print default config path and database path.
    Path,
}

#[derive(Debug, Subcommand)]
pub enum LeaderCommand {
    /// Add a leader wallet with per-leader controls.
    Add(LeaderAddArgs),
    /// List configured leaders.
    List,
    /// Remove a leader wallet.
    Remove(LeaderAddressArg),
    /// Update risk controls for a leader wallet.
    Update(LeaderUpdateArgs),
    /// Import follow candidates exported by PolyAlpha.
    ImportPolyalpha(PolyAlphaImportArgs),
}

#[derive(Debug, Args)]
pub struct LeaderAddressArg {
    pub address: String,
}

#[derive(Debug, Args)]
pub struct LeaderAddArgs {
    pub address: String,

    #[arg(long)]
    pub label: Option<String>,

    /// Account name from config.account or config.accounts.
    #[arg(long)]
    pub account: Option<String>,

    /// Ratio mode: copy leader notional * copy_ratio.
    #[arg(long, conflicts_with = "fixed_order")]
    pub copy_ratio: Option<String>,

    /// Fixed mode: copy this fixed USDC notional per BUY trade.
    #[arg(long, conflicts_with = "copy_ratio")]
    pub fixed_order: Option<String>,

    #[arg(long)]
    pub max_order: Option<String>,

    #[arg(long)]
    pub max_daily: Option<String>,

    #[arg(long)]
    pub max_position: Option<String>,

    #[arg(long)]
    pub market_allow: Vec<String>,

    #[arg(long)]
    pub market_block: Vec<String>,

    #[arg(long, default_value_t = false)]
    pub no_buy: bool,

    #[arg(long, default_value_t = false)]
    pub no_sell: bool,
}

#[derive(Debug, Args)]
pub struct LeaderUpdateArgs {
    pub address: String,

    #[arg(long)]
    pub label: Option<String>,

    #[arg(long)]
    pub enabled: Option<bool>,

    #[arg(long)]
    pub account: Option<String>,

    #[arg(long)]
    pub copy_ratio: Option<String>,

    #[arg(long)]
    pub fixed_order: Option<String>,

    #[arg(long)]
    pub max_order: Option<String>,

    #[arg(long)]
    pub max_daily: Option<String>,

    #[arg(long)]
    pub max_position: Option<String>,

    #[arg(long)]
    pub support_buy: Option<bool>,

    #[arg(long)]
    pub support_sell: Option<bool>,
}

#[derive(Debug, Args)]
pub struct PolyAlphaImportArgs {
    /// PolyAlpha JSON export or SQLite database path.
    pub input: PathBuf,

    /// Minimum score required for import.
    #[arg(long, default_value = "0.70")]
    pub min_score: String,

    /// Accepted verdict. Repeatable. Defaults to follow-like verdicts.
    #[arg(long)]
    pub verdict: Vec<String>,

    /// Copy ratio assigned to imported leaders.
    #[arg(long, default_value = "0.10")]
    pub copy_ratio: String,

    /// Per-order cap assigned to imported leaders.
    #[arg(long, default_value = "20")]
    pub max_order: String,

    /// Per-day cap assigned to imported leaders.
    #[arg(long, default_value = "100")]
    pub max_daily: String,

    /// Preview candidates without writing config.
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct RunArgs {
    /// Execution mode. Defaults to config.global.mode.
    #[arg(long)]
    pub mode: Option<RunMode>,

    /// Shortcut for --mode paper.
    #[arg(long, conflicts_with = "live")]
    pub paper: bool,

    /// Shortcut for --mode live. Requires --confirm-live.
    #[arg(long, conflicts_with = "paper")]
    pub live: bool,

    /// Required for live mode.
    #[arg(long)]
    pub confirm_live: bool,

    /// Run one polling cycle and exit.
    #[arg(long)]
    pub once: bool,

    /// Max activities to request per leader per polling cycle.
    #[arg(long, default_value_t = 100)]
    pub limit: usize,
}

#[derive(Debug, Args)]
pub struct LimitArgs {
    #[arg(long, default_value_t = 20)]
    pub limit: usize,
}

#[derive(Debug, Args)]
pub struct ServeArgs {
    /// Bind address for the local HTTP API.
    #[arg(long, default_value = "127.0.0.1:8787")]
    pub addr: String,
}

#[derive(Debug, Args)]
pub struct WatchClobArgs {
    /// CLOB market websocket URL.
    #[arg(
        long,
        default_value = "wss://ws-subscriptions-clob.polymarket.com/ws/market"
    )]
    pub ws_url: String,

    /// Token/asset id to subscribe. Repeatable.
    #[arg(long = "asset")]
    pub assets: Vec<String>,

    /// File with one token/asset id per line.
    #[arg(long)]
    pub assets_file: Option<PathBuf>,

    /// Send one subscription per chunk.
    #[arg(long, default_value_t = 500)]
    pub chunk_size: usize,

    /// Send PING every N seconds.
    #[arg(long, default_value_t = 10)]
    pub ping_secs: u64,

    /// Exit after first event payload.
    #[arg(long)]
    pub once: bool,
}

#[derive(Debug, Args)]
pub struct WatchChainArgs {
    /// Polygon JSON-RPC URL.
    #[arg(long)]
    pub rpc_url: String,

    /// Contract address to monitor. Repeatable.
    #[arg(long = "contract")]
    pub contracts: Vec<String>,

    /// Topic0 to monitor. Repeatable. Defaults to Polymarket OrderFilled topics.
    #[arg(long = "topic")]
    pub topics: Vec<String>,

    /// Start block. Defaults to latest block.
    #[arg(long)]
    pub from_block: Option<u64>,

    /// Max blocks per eth_getLogs request.
    #[arg(long, default_value_t = 1000)]
    pub batch_blocks: u64,

    /// Poll interval for continuous mode.
    #[arg(long, default_value_t = 5)]
    pub poll_secs: u64,

    /// Run one polling cycle and exit.
    #[arg(long)]
    pub once: bool,
}

#[derive(Debug, Args)]
pub struct DashboardArgs {
    /// Output HTML file.
    #[arg(long, default_value = "polyfollow-dashboard.html")]
    pub out: PathBuf,

    /// Number of recent orders/logs to include.
    #[arg(long, default_value_t = 30)]
    pub limit: usize,
}

#[derive(Debug, Args)]
pub struct BacktestArgs {
    /// JSON file containing an array of normalized LeaderTrade records.
    pub input: PathBuf,

    /// Leader wallet to backtest. Must exist in config.
    #[arg(long)]
    pub leader: String,
}

#[derive(Debug, Args)]
pub struct AllocateArgs {
    /// Override account capital. Defaults to config.account.max_capital_usdc.
    #[arg(long)]
    pub capital: Option<String>,

    /// Max order as a fraction of each leader budget.
    #[arg(long, default_value = "0.02")]
    pub order_fraction: String,

    /// Max daily notional as a fraction of each leader budget.
    #[arg(long, default_value = "0.10")]
    pub daily_fraction: String,

    /// Write suggested caps back to config.
    #[arg(long)]
    pub apply: bool,
}

#[derive(Debug, Args)]
pub struct CooldownArgs {
    /// Disable/suggest leaders with at least this many blocked intents.
    #[arg(long, default_value_t = 5)]
    pub blocked_threshold: usize,

    /// Write disabled leaders back to config.
    #[arg(long)]
    pub apply: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum RunMode {
    Paper,
    Live,
}
