use std::path::PathBuf;

use clap::{Args, Subcommand};

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

    #[arg(long, conflicts_with = "fixed_order")]
    pub copy_ratio: Option<String>,

    #[arg(long, conflicts_with = "copy_ratio")]
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
