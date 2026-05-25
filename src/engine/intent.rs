use chrono::Utc;
use rust_decimal::Decimal;

use crate::config::{CopyMode, ExecutionMode, LeaderConfig};
use crate::types::{CopyIntent, IntentVerdict, LeaderTrade, TradeSide};

use super::RiskContext;

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
    if trade.side == TradeSide::Buy
        && let (Some(open_positions), Some(max_open_positions)) =
            (risk_context.open_positions, risk_context.max_open_positions)
        && open_positions >= max_open_positions
    {
        reasons.push(format!(
            "open_positions {open_positions} reached max_open_positions {max_open_positions}"
        ));
    }
    if let (Some(realized_pnl), Some(max_daily_loss)) = (
        risk_context.realized_pnl_today_usdc,
        risk_context.max_daily_loss_usdc,
    ) && realized_pnl < Decimal::ZERO
    {
        let realized_loss = -realized_pnl;
        if realized_loss >= max_daily_loss {
            reasons.push(format!(
                "daily realized loss {realized_loss} reached max_daily_loss_usdc {max_daily_loss}"
            ));
        }
    }

    let calculated_notional = match leader.copy.mode {
        CopyMode::Ratio => trade.notional_usdc * leader.copy.ratio,
        CopyMode::Fixed => leader.copy.fixed_order_usdc,
    };
    let mut notional_usdc = min_decimal(calculated_notional, leader.risk.max_order_usdc);
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
                                "best_ask {best_ask} exceeds drift-adjusted max {max_price}"
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
                                "best_bid {best_bid} below drift-adjusted min {min_price}"
                            ));
                        }
                    }
                }
            }
        }
    }

    let mut shares = trade
        .price
        .filter(|price| *price > Decimal::ZERO)
        .map(|price| notional_usdc / price);
    if trade.side == TradeSide::Sell {
        match (risk_context.available_position_shares, shares, trade.price) {
            (Some(available), _, _) if available <= Decimal::ZERO => {
                reasons.push("no open tracked position to sell".to_string());
            }
            (Some(available), Some(requested), Some(price)) if requested > available => {
                shares = Some(available);
                notional_usdc = available * price;
            }
            (Some(_), Some(_), _) => {}
            (Some(_), None, _) => {
                reasons.push("sell requires a valid price to compute shares".to_string());
            }
            (None, _, _) => {
                reasons.push("sell requires tracked position context".to_string());
            }
        }
    }

    let verdict = if reasons.is_empty() {
        match mode {
            ExecutionMode::Paper => IntentVerdict::Paper,
            ExecutionMode::Live => IntentVerdict::Live,
        }
    } else {
        IntentVerdict::Blocked
    };

    CopyIntent {
        intent_id: format!("intent:{}:{}", leader.address, trade.trade_id),
        leader_address: leader.address.clone(),
        trade_id: trade.trade_id.clone(),
        mode: format!("{mode:?}").to_ascii_lowercase(),
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
