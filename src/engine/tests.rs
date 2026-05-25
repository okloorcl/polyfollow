use chrono::Utc;
use rust_decimal_macros::dec;

use super::*;
use crate::config::{CopyConfig, CopyMode, ExecutionMode, LeaderConfig, LeaderRiskConfig};
use crate::types::{IntentVerdict, LeaderTrade, TradeSide};

#[test]
fn ratio_intent_caps_to_max_order() {
    let leader = LeaderConfig {
        address: "0x2222222222222222222222222222222222222222".to_string(),
        label: None,
        account_name: None,
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
    let trade = trade(&leader, "tx", TradeSide::Buy, dec!(100));
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

#[test]
fn sell_intent_caps_to_tracked_position() {
    let leader = LeaderConfig {
        address: "0x2222222222222222222222222222222222222222".to_string(),
        label: None,
        account_name: None,
        enabled: true,
        copy: CopyConfig {
            mode: CopyMode::Ratio,
            ratio: dec!(1),
            fixed_order_usdc: dec!(10),
        },
        risk: LeaderRiskConfig {
            max_order_usdc: dec!(100),
            ..LeaderRiskConfig::default()
        },
        filters: Default::default(),
    };
    let trade = trade(&leader, "tx-sell", TradeSide::Sell, dec!(50));
    let intent = build_intent(
        ExecutionMode::Paper,
        &leader,
        &trade,
        RiskContext {
            available_position_shares: Some(dec!(20)),
            ..RiskContext::default()
        },
    );
    assert_eq!(intent.notional_usdc, dec!(10.0));
    assert_eq!(intent.shares, Some(dec!(20)));
    assert_eq!(intent.verdict, IntentVerdict::Paper);
}

#[test]
fn buy_intent_blocks_when_global_position_cap_is_reached() {
    let leader = default_leader();
    let trade = trade(&leader, "tx-cap", TradeSide::Buy, dec!(10));
    let intent = build_intent(
        ExecutionMode::Paper,
        &leader,
        &trade,
        RiskContext {
            open_positions: Some(30),
            max_open_positions: Some(30),
            ..RiskContext::default()
        },
    );

    assert_eq!(intent.verdict, IntentVerdict::Blocked);
    assert!(
        intent
            .reasons
            .iter()
            .any(|reason| reason.contains("max_open_positions"))
    );
}

#[test]
fn buy_intent_blocks_when_daily_loss_cap_is_reached() {
    let leader = default_leader();
    let trade = trade(&leader, "tx-loss", TradeSide::Buy, dec!(10));
    let intent = build_intent(
        ExecutionMode::Paper,
        &leader,
        &trade,
        RiskContext {
            realized_pnl_today_usdc: Some(dec!(-50)),
            max_daily_loss_usdc: Some(dec!(50)),
            ..RiskContext::default()
        },
    );

    assert_eq!(intent.verdict, IntentVerdict::Blocked);
    assert!(
        intent
            .reasons
            .iter()
            .any(|reason| reason.contains("max_daily_loss_usdc"))
    );
}

fn default_leader() -> LeaderConfig {
    LeaderConfig {
        address: "0x2222222222222222222222222222222222222222".to_string(),
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
    }
}

fn trade(
    leader: &LeaderConfig,
    trade_id: &str,
    side: TradeSide,
    notional_usdc: rust_decimal::Decimal,
) -> LeaderTrade {
    LeaderTrade {
        leader_address: leader.address.clone(),
        trade_id: trade_id.to_string(),
        source: "test".to_string(),
        source_timestamp: Utc::now(),
        received_at: Utc::now(),
        latency_ms: 100,
        side,
        condition_id: Some("market-1".to_string()),
        token_id: Some("123".to_string()),
        title: None,
        slug: None,
        event_slug: None,
        outcome: None,
        outcome_index: None,
        price: Some(dec!(0.5)),
        shares: Some(notional_usdc / dec!(0.5)),
        notional_usdc,
        raw_json: serde_json::json!({}),
    }
}
