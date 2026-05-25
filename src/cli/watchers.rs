use std::path::PathBuf;

use clap::Args;

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
