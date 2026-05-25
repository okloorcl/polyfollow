use std::path::PathBuf;

use clap::Args;

#[derive(Debug, Args)]
pub struct ServeArgs {
    /// Bind address for the local HTTP API.
    #[arg(long, default_value = "127.0.0.1:8787")]
    pub addr: String,
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

#[derive(Debug, Args)]
pub struct MarketBridgeContextArgs {
    /// MarketBridge base URL.
    #[arg(long, default_value = "http://127.0.0.1:8080")]
    pub base_url: String,

    /// Symbol to request. Repeatable.
    #[arg(long = "symbol")]
    pub symbols: Vec<String>,

    /// Product type / market parameter, e.g. spot or perp.
    #[arg(long, default_value = "perp")]
    pub market: String,
}
