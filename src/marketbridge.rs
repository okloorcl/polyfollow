use anyhow::{Context, Result};
use serde_json::Value;

pub async fn fetch_agent_context(
    base_url: &str,
    symbols: &[String],
    market: &str,
) -> Result<Value> {
    if symbols.is_empty() {
        anyhow::bail!("provide at least one --symbol");
    }
    let url = format!("{}/v1/agent/context", base_url.trim_end_matches('/'));
    let symbols = symbols.join(",");
    let value = reqwest::Client::new()
        .get(url)
        .query(&[("symbols", symbols.as_str()), ("market", market)])
        .send()
        .await
        .context("failed to request MarketBridge context")?
        .error_for_status()
        .context("MarketBridge returned error status")?
        .json::<Value>()
        .await
        .context("MarketBridge returned invalid json")?;
    Ok(value)
}
