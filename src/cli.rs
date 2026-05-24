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

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum RunMode {
    Paper,
    Live,
}
