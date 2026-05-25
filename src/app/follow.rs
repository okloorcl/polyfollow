use anyhow::{Context, Result};

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
                    Some(
                        storage
                            .leader_token_open_shares(&leader.address, trade.token_id.as_deref())?,
                    )
                } else {
                    None
                },
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
                let account = cfg.account_for_leader(leader)?;
                let live_config = LiveExecutionConfig::from_env(account)?;
                match execute_live_market_order(&live_config, &intent).await {
                    Ok(response) => {
                        storage.insert_live_attempt(
                            &intent.intent_id,
                            "submitted",
                            &request,
                            Some(&response),
                        )?;
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
