use anyhow::{Context, Result};
use chrono::{DateTime, TimeZone, Utc};
use rust_decimal::Decimal;
use serde::Deserialize;
use serde_json::Value;

use crate::config::LeaderConfig;
use crate::types::{LeaderTrade, TradeSide};

#[derive(Clone)]
pub struct ActivityPoller {
    client: reqwest::Client,
    base_url: String,
}

impl ActivityPoller {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.into().trim_end_matches('/').to_string(),
        }
    }

    pub async fn fetch_trades(
        &self,
        leader: &LeaderConfig,
        limit: usize,
    ) -> Result<Vec<LeaderTrade>> {
        let url = format!("{}/activity", self.base_url);
        let activities = self
            .client
            .get(url)
            .query(&[
                ("user", leader.address.as_str()),
                ("limit", &limit.to_string()),
            ])
            .send()
            .await
            .context("failed to request Polymarket activities")?
            .error_for_status()
            .context("Polymarket activities returned an error status")?
            .json::<Vec<Activity>>()
            .await
            .context("failed to decode Polymarket activities")?;

        let now = Utc::now();
        activities
            .into_iter()
            .filter(|activity| activity.activity_type.eq_ignore_ascii_case("TRADE"))
            .filter_map(|activity| normalize_activity(&leader.address, activity, now).transpose())
            .collect::<Result<Vec<_>>>()
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Activity {
    #[serde(rename = "type")]
    activity_type: String,
    proxy_wallet: Option<String>,
    timestamp: Option<i64>,
    condition_id: Option<String>,
    transaction_hash: Option<String>,
    asset: Option<String>,
    side: Option<String>,
    outcome_index: Option<i64>,
    title: Option<String>,
    slug: Option<String>,
    event_slug: Option<String>,
    outcome: Option<String>,
    price: Option<Value>,
    size: Option<Value>,
    usdc_size: Option<Value>,
    #[serde(flatten)]
    extra: serde_json::Map<String, Value>,
}

fn normalize_activity(
    leader_address: &str,
    activity: Activity,
    received_at: DateTime<Utc>,
) -> Result<Option<LeaderTrade>> {
    let Some(raw_side) = activity.side.as_deref() else {
        return Ok(None);
    };
    let side = match raw_side.to_ascii_uppercase().as_str() {
        "BUY" => TradeSide::Buy,
        "SELL" => TradeSide::Sell,
        _ => return Ok(None),
    };
    let Some(trade_id) = activity.transaction_hash.clone() else {
        return Ok(None);
    };
    let Some(timestamp) = activity.timestamp else {
        return Ok(None);
    };

    let source_timestamp = parse_polymarket_timestamp(timestamp)
        .ok_or_else(|| anyhow::anyhow!("invalid activity timestamp: {timestamp}"))?;
    let latency_ms = (received_at - source_timestamp).num_milliseconds().max(0);
    let price = activity.price.as_ref().and_then(decimal_from_value);
    let shares = activity.size.as_ref().and_then(decimal_from_value);
    let notional_usdc = activity
        .usdc_size
        .as_ref()
        .and_then(decimal_from_value)
        .or_else(|| price.zip(shares).map(|(price, shares)| price * shares))
        .unwrap_or(Decimal::ZERO);

    let mut raw = serde_json::Map::new();
    raw.insert("type".to_string(), Value::String(activity.activity_type));
    if let Some(proxy_wallet) = activity.proxy_wallet {
        raw.insert("proxyWallet".to_string(), Value::String(proxy_wallet));
    }
    raw.insert("timestamp".to_string(), Value::Number(timestamp.into()));
    if let Some(value) = &activity.condition_id {
        raw.insert("conditionId".to_string(), Value::String(value.clone()));
    }
    raw.insert(
        "transactionHash".to_string(),
        Value::String(trade_id.clone()),
    );
    for (key, value) in activity.extra {
        raw.insert(key, value);
    }

    Ok(Some(LeaderTrade {
        leader_address: leader_address.to_string(),
        trade_id,
        source: "data_api_activity".to_string(),
        source_timestamp,
        received_at,
        latency_ms,
        side,
        condition_id: activity.condition_id,
        token_id: activity.asset,
        title: activity.title,
        slug: activity.slug,
        event_slug: activity.event_slug,
        outcome: activity.outcome,
        outcome_index: activity.outcome_index,
        price,
        shares,
        notional_usdc,
        raw_json: Value::Object(raw),
    }))
}

fn parse_polymarket_timestamp(timestamp: i64) -> Option<DateTime<Utc>> {
    if timestamp > 10_000_000_000 {
        Utc.timestamp_millis_opt(timestamp).single()
    } else {
        Utc.timestamp_opt(timestamp, 0).single()
    }
}

fn decimal_from_value(value: &Value) -> Option<Decimal> {
    match value {
        Value::Number(number) => number.to_string().parse().ok(),
        Value::String(text) => text.parse().ok(),
        _ => None,
    }
}
