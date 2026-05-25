use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

mod leaders;
#[cfg(test)]
mod tests;
mod tools;
mod watchers;

pub use leaders::*;
pub use tools::*;
pub use watchers::*;

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
    /// Fetch agent-friendly context from a local MarketBridge instance.
    MarketbridgeContext(MarketBridgeContextArgs),
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

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum RunMode {
    Paper,
    Live,
}
