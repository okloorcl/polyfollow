use anyhow::{Context, Result};
use chrono::Utc;

use crate::config::{AppConfig, ExecutionMode};
use crate::engine::{RiskContext, build_intent};
use crate::execution::{LiveExecutionConfig, execute_live_market_order};
use crate::market::OrderBookClient;
use crate::monitor::ActivityPoller;
use crate::notify::Notifier;
use crate::storage::Storage;

use super::responses::RunStats;

pub(super) async fn run_once(
    cfg: &AppConfig,
    storage: &mut Storage,
    mode: ExecutionMode,
    limit: usize,
) -> Result<RunStats> {
    if cfg.global.kill_switch {
        anyhow::bail!("global kill switch is enabled");
    }

    let poller = ActivityPoller::new(&cfg.global.data_api_base_url);
    let order_books = OrderBookClient::new(&cfg.global.clob_base_url);
    let notifier = Notifier::new(&cfg.notifications);
    let mut stats = RunStats::default();
    for leader in cfg.leaders.iter().filter(|leader| leader.enabled) {
        let account = cfg.account_for_leader(leader)?;
        let trades = poller
            .fetch_trades(leader, limit)
            .await
            .with_context(|| format!("failed to poll leader {}", leader.address))?;
        stats.fetched_trades += trades.len();
        for trade in trades {
            if storage.has_processed_trade(&trade.leader_address, &trade.trade_id)? {
                continue;
            }
            let mut risk_context = RiskContext {
                leader_daily_notional_usdc: storage.leader_daily_notional(&leader.address)?,
                market_open_notional_usdc: storage
                    .leader_market_open_notional(&leader.address, trade.condition_id.as_deref())?,
                available_position_shares: if trade.side == crate::types::TradeSide::Sell {
                    Some(storage.leader_token_open_shares(
                        &leader.address,
                        trade.token_id.as_deref(),
                        matches!(mode, ExecutionMode::Live),
                    )?)
                } else {
                    None
                },
                open_positions: Some(storage.open_position_count()?),
                max_open_positions: Some(cfg.global.max_open_positions),
                realized_pnl_today_usdc: Some(storage.daily_realized_pnl_at(Utc::now())?),
                max_daily_loss_usdc: Some(min_decimal(
                    cfg.global.max_daily_loss_usdc,
                    account.max_daily_loss_usdc,
                )),
                book: None,
                book_error: None,
            };
            if let Some(token_id) = trade.token_id.as_deref() {
                let preview_intent = build_intent(mode, leader, &trade, RiskContext::default());
                match order_books
                    .metrics_for(token_id, trade.side, preview_intent.notional_usdc)
                    .await
                {
                    Ok(book) => risk_context.book = Some(book),
                    Err(error) => {
                        tracing::warn!(%token_id, error = %error, "failed to fetch order book");
                        risk_context.book_error = Some("order book unavailable");
                    }
                }
            }
            let intent = build_intent(mode, leader, &trade, risk_context);
            let inserted = storage.insert_processed_trade(&trade, "observed")?;
            if !inserted {
                continue;
            }
            stats.new_trades += 1;
            storage.insert_copy_intent(&intent)?;
            if intent.verdict == crate::types::IntentVerdict::Paper {
                let result = storage.apply_paper_intent(&intent)?;
                stats.paper_fills += result.opened_fills + result.closed_lots;
                notifier.notify_intent(&intent, Some(&result)).await;
            } else if intent.verdict == crate::types::IntentVerdict::Live {
                let request = serde_json::to_value(&intent)?;
                let live_config = LiveExecutionConfig::from_env(account)?;
                match execute_live_market_order(&live_config, &intent).await {
                    Ok(response) => {
                        let status = live_attempt_status(&response);
                        storage.insert_live_attempt(
                            &intent.intent_id,
                            status,
                            &request,
                            Some(&response),
                        )?;
                        if status != "submitted" {
                            anyhow::bail!("{}", live_rejection_message(&response));
                        }
                        notifier.notify_intent(&intent, None).await;
                    }
                    Err(error) => {
                        storage.insert_live_attempt(
                            &intent.intent_id,
                            "failed",
                            &request,
                            Some(&serde_json::json!({"error": error.to_string()})),
                        )?;
                        return Err(error);
                    }
                }
            } else {
                stats.blocked_intents += 1;
                notifier.notify_intent(&intent, None).await;
            }
        }
    }
    Ok(stats)
}

fn min_decimal(left: rust_decimal::Decimal, right: rust_decimal::Decimal) -> rust_decimal::Decimal {
    if left <= right { left } else { right }
}

fn live_attempt_status(response: &serde_json::Value) -> &'static str {
    match response.get("success").and_then(serde_json::Value::as_bool) {
        Some(true) => "submitted",
        Some(false) => "rejected",
        None => "unknown",
    }
}

fn live_rejection_message(response: &serde_json::Value) -> String {
    let exchange_status = response
        .get("status")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown");
    let error_msg = response
        .get("error_msg")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("no exchange error message");
    format!("live order was not submitted: status={exchange_status}, error={error_msg}")
}

pub(super) async fn run_loop(
    cfg: &AppConfig,
    storage: &mut Storage,
    mode: ExecutionMode,
    limit: usize,
) -> Result<RunStats> {
    loop {
        let stats = run_once(cfg, storage, mode, limit).await?;
        println!(
            "cycle: fetched={}, new={}, paper={}, blocked={}",
            stats.fetched_trades, stats.new_trades, stats.paper_fills, stats.blocked_intents
        );
        tokio::time::sleep(std::time::Duration::from_secs(
            cfg.global.poll_interval_secs,
        ))
        .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_attempt_status_distinguishes_exchange_rejections() {
        assert_eq!(
            live_attempt_status(&serde_json::json!({"success": true})),
            "submitted"
        );
        assert_eq!(
            live_attempt_status(&serde_json::json!({"success": false})),
            "rejected"
        );
        assert_eq!(
            live_attempt_status(&serde_json::json!({"status": "MATCHED"})),
            "unknown"
        );
    }

    #[test]
    fn live_rejection_message_includes_exchange_reason() {
        let message = live_rejection_message(&serde_json::json!({
            "status": "UNMATCHED",
            "error_msg": "insufficient balance"
        }));

        assert!(message.contains("UNMATCHED"));
        assert!(message.contains("insufficient balance"));
    }
}
