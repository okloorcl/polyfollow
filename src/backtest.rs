use std::path::Path;

use anyhow::{Context, Result};
use rust_decimal::Decimal;
use serde::Serialize;

use crate::config::{AppConfig, ExecutionMode, LeaderConfig};
use crate::engine::{RiskContext, build_intent};
use crate::storage::Storage;
use crate::types::{LeaderTrade, TradeSide};
use crate::validate::normalize_address;

#[derive(Debug, Serialize)]
pub struct BacktestReport {
    pub leader: String,
    pub trades: usize,
    pub intents: usize,
    pub fills: usize,
    pub blocked: usize,
    pub open_notional_usdc: String,
    pub realized_pnl_usdc: String,
}

pub fn run_backtest(cfg: &AppConfig, leader: &str, input: &Path) -> Result<BacktestReport> {
    let leader_address = normalize_address(leader)?;
    let leader = cfg
        .leaders
        .iter()
        .find(|item| item.address.eq_ignore_ascii_case(&leader_address))
        .ok_or_else(|| anyhow::anyhow!("leader not found in config: {leader_address}"))?;
    let trades = load_trades(input, &leader.address)?;
    let temp = std::env::temp_dir().join(format!(
        "polyfollow-backtest-{}.sqlite",
        chrono::Utc::now().timestamp_nanos_opt().unwrap()
    ));
    let mut storage = Storage::open(&temp)?;
    let mut report = replay(cfg, leader, &trades, &mut storage)?;
    let pnl = storage.pnl_summary()?;
    report.open_notional_usdc = pnl.open_notional_usdc;
    report.realized_pnl_usdc = pnl.realized_pnl_usdc;
    let _ = std::fs::remove_file(temp);
    Ok(report)
}

fn load_trades(path: &Path, leader_address: &str) -> Result<Vec<LeaderTrade>> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let mut trades = serde_json::from_str::<Vec<LeaderTrade>>(&text)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    trades.retain(|trade| trade.leader_address.eq_ignore_ascii_case(leader_address));
    trades.sort_by_key(|trade| trade.source_timestamp);
    Ok(trades)
}

fn replay(
    cfg: &AppConfig,
    leader: &LeaderConfig,
    trades: &[LeaderTrade],
    storage: &mut Storage,
) -> Result<BacktestReport> {
    let mut report = BacktestReport {
        leader: leader.address.clone(),
        trades: trades.len(),
        intents: 0,
        fills: 0,
        blocked: 0,
        open_notional_usdc: Decimal::ZERO.to_string(),
        realized_pnl_usdc: Decimal::ZERO.to_string(),
    };
    for trade in trades {
        let context = RiskContext {
            leader_daily_notional_usdc: storage
                .leader_daily_notional_at(&leader.address, trade.source_timestamp)?,
            market_open_notional_usdc: storage
                .leader_market_open_notional(&leader.address, trade.condition_id.as_deref())?,
            available_position_shares: if trade.side == TradeSide::Sell {
                Some(storage.leader_token_open_shares(
                    &leader.address,
                    trade.token_id.as_deref(),
                    false,
                )?)
            } else {
                None
            },
            open_positions: Some(storage.open_position_count()?),
            max_open_positions: Some(cfg.global.max_open_positions),
            realized_pnl_today_usdc: Some(storage.daily_realized_pnl_at(trade.source_timestamp)?),
            max_daily_loss_usdc: Some(min_decimal(
                cfg.global.max_daily_loss_usdc,
                cfg.account_for_leader(leader)?.max_daily_loss_usdc,
            )),
            book: None,
            book_error: None,
        };
        let mut intent = build_intent(ExecutionMode::Paper, leader, trade, context);
        intent.created_at = trade.source_timestamp;
        storage.insert_copy_intent(&intent)?;
        report.intents += 1;
        if intent.verdict == crate::types::IntentVerdict::Paper {
            let result = storage.apply_paper_intent(&intent)?;
            report.fills += result.opened_fills + result.closed_lots;
        } else {
            report.blocked += 1;
        }
    }
    Ok(report)
}

fn min_decimal(left: Decimal, right: Decimal) -> Decimal {
    if left <= right { left } else { right }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use rust_decimal_macros::dec;

    use super::*;
    use crate::config::{CopyConfig, CopyMode, LeaderRiskConfig};
    use crate::types::TradeSide;

    #[test]
    fn backtest_replays_normalized_trades() {
        let leader = "0x2222222222222222222222222222222222222222";
        let path = std::env::temp_dir().join(format!(
            "polyfollow-backtest-{}.json",
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        std::fs::write(
            &path,
            serde_json::to_string(&vec![trade(leader, "buy", TradeSide::Buy)]).unwrap(),
        )
        .unwrap();
        let cfg = AppConfig {
            leaders: vec![LeaderConfig {
                address: leader.to_string(),
                label: None,
                account_name: None,
                enabled: true,
                copy: CopyConfig {
                    mode: CopyMode::Ratio,
                    ratio: dec!(1),
                    fixed_order_usdc: dec!(10),
                },
                risk: LeaderRiskConfig::default(),
                filters: Default::default(),
            }],
            ..AppConfig::default()
        };
        let report = run_backtest(&cfg, leader, &path).unwrap();
        assert_eq!(report.trades, 1);
        assert_eq!(report.fills, 1);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn backtest_applies_daily_caps_by_trade_day() {
        let leader = "0x2222222222222222222222222222222222222222";
        let path = std::env::temp_dir().join(format!(
            "polyfollow-backtest-days-{}.json",
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        let day_one = Utc::now() - chrono::Duration::days(2);
        let day_two = Utc::now() - chrono::Duration::days(1);
        std::fs::write(
            &path,
            serde_json::to_string(&vec![
                trade_at(leader, "day-1", TradeSide::Buy, day_one),
                trade_at(leader, "day-2", TradeSide::Buy, day_two),
            ])
            .unwrap(),
        )
        .unwrap();
        let cfg = AppConfig {
            leaders: vec![LeaderConfig {
                address: leader.to_string(),
                label: None,
                account_name: None,
                enabled: true,
                copy: CopyConfig {
                    mode: CopyMode::Ratio,
                    ratio: dec!(1),
                    fixed_order_usdc: dec!(10),
                },
                risk: LeaderRiskConfig {
                    max_daily_usdc: dec!(15),
                    ..LeaderRiskConfig::default()
                },
                filters: Default::default(),
            }],
            ..AppConfig::default()
        };

        let report = run_backtest(&cfg, leader, &path).unwrap();

        assert_eq!(report.trades, 2);
        assert_eq!(report.fills, 2);
        assert_eq!(report.blocked, 0);

        let _ = std::fs::remove_file(path);
    }

    fn trade(leader: &str, id: &str, side: TradeSide) -> LeaderTrade {
        trade_at(leader, id, side, Utc::now())
    }

    fn trade_at(
        leader: &str,
        id: &str,
        side: TradeSide,
        source_timestamp: chrono::DateTime<Utc>,
    ) -> LeaderTrade {
        LeaderTrade {
            leader_address: leader.to_string(),
            trade_id: id.to_string(),
            source: "test".to_string(),
            source_timestamp,
            received_at: source_timestamp,
            latency_ms: 10,
            side,
            condition_id: Some("market".to_string()),
            token_id: Some("123".to_string()),
            title: None,
            slug: None,
            event_slug: None,
            outcome: None,
            outcome_index: None,
            price: Some(dec!(0.5)),
            shares: Some(dec!(20)),
            notional_usdc: dec!(10),
            raw_json: serde_json::json!({}),
        }
    }
}
