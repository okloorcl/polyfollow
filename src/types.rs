use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TradeSide {
    Buy,
    Sell,
}

impl TradeSide {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Buy => "buy",
            Self::Sell => "sell",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderTrade {
    pub leader_address: String,
    pub trade_id: String,
    pub source: String,
    pub source_timestamp: DateTime<Utc>,
    pub received_at: DateTime<Utc>,
    pub latency_ms: i64,
    pub side: TradeSide,
    pub condition_id: Option<String>,
    pub token_id: Option<String>,
    pub title: Option<String>,
    pub slug: Option<String>,
    pub event_slug: Option<String>,
    pub outcome: Option<String>,
    pub outcome_index: Option<i64>,
    pub price: Option<Decimal>,
    pub shares: Option<Decimal>,
    pub notional_usdc: Decimal,
    pub raw_json: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyIntent {
    pub intent_id: String,
    pub leader_address: String,
    pub trade_id: String,
    pub mode: String,
    pub side: TradeSide,
    pub market_id: Option<String>,
    pub token_id: Option<String>,
    pub target_price: Option<Decimal>,
    pub notional_usdc: Decimal,
    pub shares: Option<Decimal>,
    pub verdict: IntentVerdict,
    pub reasons: Vec<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IntentVerdict {
    Paper,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperFill {
    pub paper_fill_id: String,
    pub intent_id: String,
    pub entry_price: Option<Decimal>,
    pub shares: Option<Decimal>,
    pub notional_usdc: Decimal,
    pub status: String,
    pub opened_at: DateTime<Utc>,
}
