use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct StorageStatus {
    pub db_path: String,
    pub leader_count: i64,
    pub processed_trade_count: i64,
    pub copy_intent_count: i64,
    pub paper_fill_count: i64,
    pub live_order_attempt_count: i64,
}

#[derive(Debug, Serialize)]
pub struct IntentRow {
    pub intent_id: String,
    pub leader_address: String,
    pub trade_id: String,
    pub mode: String,
    pub side: String,
    pub market_id: Option<String>,
    pub token_id: Option<String>,
    pub target_price: Option<String>,
    pub notional_usdc: String,
    pub shares: Option<String>,
    pub verdict: String,
    pub reasons_json: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct TradeLogRow {
    pub leader_address: String,
    pub trade_id: String,
    pub source: String,
    pub status: String,
    pub observed_at: String,
}

#[derive(Debug, Serialize)]
pub struct PnlSummary {
    pub open_paper_fills: i64,
    pub closed_paper_fills: i64,
    pub open_notional_usdc: String,
    pub realized_pnl_usdc: String,
}
