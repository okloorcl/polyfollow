use chrono::Utc;
use rust_decimal::Decimal;

use crate::config::{CopyMode, ExecutionMode, LeaderConfig};
use crate::market::BookMetrics;
use crate::types::{CopyIntent, IntentVerdict, LeaderTrade, TradeSide};

#[derive(Debug, Clone, Default)]
pub struct RiskContext {
    pub leader_daily_notional_usdc: Decimal,
    pub market_open_notional_usdc: Decimal,
    pub book: Option<BookMetrics>,
    pub book_error: Option<&'static str>,
}

pub fn build_intent(
    mode: ExecutionMode,
    leader: &LeaderConfig,
    trade: &LeaderTrade,
    risk_context: RiskContext,
) -> CopyIntent {
    let mut reasons = Vec::new();
    if !leader.enabled {
        reasons.push("leader disabled".to_string());
    }
    if trade.side == TradeSide::Buy && !leader.risk.support_buy {
        reasons.push("buy not supported for leader".to_string());
    }
    if trade.side == TradeSide::Sell && !leader.risk.support_sell {
        reasons.push("sell not supported for leader".to_string());
    }
    if trade.latency_ms > leader.risk.max_latency_secs * 1000 {
        reasons.push(format!(
            "latency_ms {} exceeds max {}",
            trade.latency_ms,
            leader.risk.max_latency_secs * 1000
        ));
    }

    let calculated_notional = match leader.copy.mode {
        CopyMode::Ratio => trade.notional_usdc * leader.copy.ratio,
        CopyMode::Fixed => leader.copy.fixed_order_usdc,
    };
    let notional_usdc = min_decimal(calculated_notional, leader.risk.max_order_usdc);
    if notional_usdc <= Decimal::ZERO {
        reasons.push("copy notional is zero".to_string());
    }
    if risk_context.leader_daily_notional_usdc + notional_usdc > leader.risk.max_daily_usdc {
        reasons.push(format!(
            "leader daily notional {} + {} exceeds max_daily_usdc {}",
            risk_context.leader_daily_notional_usdc, notional_usdc, leader.risk.max_daily_usdc
        ));
    }
    if risk_context.market_open_notional_usdc + notional_usdc > leader.risk.max_position_usdc {
        reasons.push(format!(
            "market open notional {} + {} exceeds max_position_usdc {}",
            risk_context.market_open_notional_usdc, notional_usdc, leader.risk.max_position_usdc
        ));
    }
    if !leader.filters.allow.is_empty() && !market_matches_any(trade, &leader.filters.allow) {
        reasons.push("market not in allowlist".to_string());
    }
    if market_matches_any(trade, &leader.filters.block) {
        reasons.push("market is blocklisted".to_string());
    }
    if let Some(book_error) = risk_context.book_error {
        reasons.push(book_error.to_string());
    }
    if let Some(book) = risk_context.book.as_ref() {
        if let Some(spread_bps) = book.spread_bps
            && spread_bps > leader.risk.max_spread_bps
        {
            reasons.push(format!(
                "spread_bps {} exceeds max_spread_bps {}",
                spread_bps, leader.risk.max_spread_bps
            ));
        }
        if book.executable_depth_usdc < leader.risk.min_depth_usdc {
            reasons.push(format!(
                "executable_depth_usdc {} below min_depth_usdc {}",
                book.executable_depth_usdc, leader.risk.min_depth_usdc
            ));
        }
        if let Some(leader_price) = trade.price {
            match trade.side {
                TradeSide::Buy => {
                    if let Some(best_ask) = book.best_ask {
                        let max_price = leader_price
                            * (Decimal::ONE
                                + leader.risk.max_price_drift_bps / Decimal::from(10_000));
                        if best_ask > max_price {
                            reasons.push(format!(
                                "best_ask {} exceeds drift-adjusted max {}",
                                best_ask, max_price
                            ));
                        }
                    }
                }
                TradeSide::Sell => {
                    if let Some(best_bid) = book.best_bid {
                        let min_price = leader_price
                            * (Decimal::ONE
                                - leader.risk.max_price_drift_bps / Decimal::from(10_000));
                        if best_bid < min_price {
                            reasons.push(format!(
                                "best_bid {} below drift-adjusted min {}",
                                best_bid, min_price
                            ));
                        }
                    }
                }
            }
        }
    }

    let shares = trade
        .price
        .filter(|price| *price > Decimal::ZERO)
        .map(|price| notional_usdc / price);

    let verdict = if reasons.is_empty() {
        match mode {
            ExecutionMode::Paper => IntentVerdict::Paper,
            ExecutionMode::Live => IntentVerdict::Blocked,
        }
    } else {
        IntentVerdict::Blocked
    };

    let mut reasons = reasons;
    if matches!(mode, ExecutionMode::Live) {
        reasons.push("live execution is blocked until native executor lands".to_string());
    }

    CopyIntent {
        intent_id: format!("intent:{}:{}", leader.address, trade.trade_id),
        leader_address: leader.address.clone(),
        trade_id: trade.trade_id.clone(),
        mode: format!("{:?}", mode).to_ascii_lowercase(),
        side: trade.side,
        market_id: trade.condition_id.clone(),
        token_id: trade.token_id.clone(),
        target_price: trade.price,
        notional_usdc,
        shares,
        verdict,
        reasons,
        created_at: Utc::now(),
    }
}

fn min_decimal(left: Decimal, right: Decimal) -> Decimal {
    if left <= right { left } else { right }
}

fn market_matches_any(trade: &LeaderTrade, patterns: &[String]) -> bool {
    if patterns.is_empty() {
        return false;
    }
    let haystack = [
        trade.condition_id.as_deref(),
        trade.slug.as_deref(),
        trade.event_slug.as_deref(),
        trade.title.as_deref(),
        trade.outcome.as_deref(),
    ]
    .into_iter()
    .flatten()
    .map(str::to_ascii_lowercase)
    .collect::<Vec<_>>()
    .join(" ");
    patterns
        .iter()
        .map(|pattern| pattern.to_ascii_lowercase())
        .any(|pattern| haystack.contains(&pattern))
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use rust_decimal_macros::dec;

    use super::*;
    use crate::config::{CopyConfig, LeaderRiskConfig};

    #[test]
    fn ratio_intent_caps_to_max_order() {
        let leader = LeaderConfig {
            address: "0x2222222222222222222222222222222222222222".to_string(),
            label: None,
            enabled: true,
            copy: CopyConfig {
                mode: CopyMode::Ratio,
                ratio: dec!(0.5),
                fixed_order_usdc: dec!(10),
            },
            risk: LeaderRiskConfig {
                max_order_usdc: dec!(20),
                ..LeaderRiskConfig::default()
            },
            filters: Default::default(),
        };
        let trade = LeaderTrade {
            leader_address: leader.address.clone(),
            trade_id: "tx".to_string(),
            source: "test".to_string(),
            source_timestamp: Utc::now(),
            received_at: Utc::now(),
            latency_ms: 100,
            side: TradeSide::Buy,
            condition_id: None,
            token_id: None,
            title: None,
            slug: None,
            event_slug: None,
            outcome: None,
            outcome_index: None,
            price: Some(dec!(0.5)),
            shares: Some(dec!(200)),
            notional_usdc: dec!(100),
            raw_json: serde_json::json!({}),
        };
        let intent = build_intent(
            ExecutionMode::Paper,
            &leader,
            &trade,
            RiskContext::default(),
        );
        assert_eq!(intent.notional_usdc, dec!(20));
        assert_eq!(intent.shares, Some(dec!(40)));
        assert_eq!(intent.verdict, IntentVerdict::Paper);
    }
}
