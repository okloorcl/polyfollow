use chrono::Utc;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

use super::*;
use crate::types::TradeSide;

#[test]
fn paper_sell_closes_fifo_and_records_realized_pnl() {
    let path = std::env::temp_dir().join(format!(
        "polyfollow-fifo-{}.sqlite",
        chrono::Utc::now().timestamp_nanos_opt().unwrap()
    ));
    let mut storage = Storage::open(&path).unwrap();
    let leader = "0x2222222222222222222222222222222222222222";
    let token_id = "123";

    let buy_one = intent(
        "buy-1",
        leader,
        token_id,
        TradeSide::Buy,
        dec!(0.4),
        dec!(40),
    );
    let buy_two = intent(
        "buy-2",
        leader,
        token_id,
        TradeSide::Buy,
        dec!(0.6),
        dec!(60),
    );
    storage.insert_copy_intent(&buy_one).unwrap();
    storage.apply_paper_intent(&buy_one).unwrap();
    storage.insert_copy_intent(&buy_two).unwrap();
    storage.apply_paper_intent(&buy_two).unwrap();

    let mut sell = intent(
        "sell-1",
        leader,
        token_id,
        TradeSide::Sell,
        dec!(0.5),
        dec!(75),
    );
    sell.shares = Some(dec!(150));
    storage.insert_copy_intent(&sell).unwrap();
    let result = storage.apply_paper_intent(&sell).unwrap();

    assert_eq!(result.closed_lots, 2);
    assert_eq!(result.realized_pnl_usdc, dec!(5.0));
    assert_eq!(
        storage
            .leader_token_open_shares(leader, Some(token_id), false)
            .unwrap(),
        dec!(50)
    );
    assert_eq!(storage.pnl_summary().unwrap().realized_pnl_usdc, "5.0");

    let _ = std::fs::remove_file(path);
}

#[test]
fn daily_notional_counts_accepted_live_and_paper_intents() {
    let path = std::env::temp_dir().join(format!(
        "polyfollow-daily-{}.sqlite",
        chrono::Utc::now().timestamp_nanos_opt().unwrap()
    ));
    let storage = Storage::open(&path).unwrap();
    let leader = "0x2222222222222222222222222222222222222222";
    let token_id = "123";

    let paper = intent(
        "paper-1",
        leader,
        token_id,
        TradeSide::Buy,
        dec!(0.5),
        dec!(25),
    );
    let mut live = intent(
        "live-1",
        leader,
        token_id,
        TradeSide::Buy,
        dec!(0.5),
        dec!(30),
    );
    live.mode = "live".to_string();
    live.verdict = IntentVerdict::Live;
    let mut blocked = intent(
        "blocked-1",
        leader,
        token_id,
        TradeSide::Buy,
        dec!(0.5),
        dec!(40),
    );
    blocked.verdict = IntentVerdict::Blocked;
    blocked.reasons = vec!["risk block".to_string()];

    storage.insert_copy_intent(&paper).unwrap();
    storage.insert_copy_intent(&live).unwrap();
    storage.insert_copy_intent(&blocked).unwrap();

    assert_eq!(storage.leader_daily_notional(leader).unwrap(), dec!(55));

    let _ = std::fs::remove_file(path);
}

#[test]
fn market_open_notional_counts_submitted_live_exposure() {
    let path = std::env::temp_dir().join(format!(
        "polyfollow-live-position-{}.sqlite",
        chrono::Utc::now().timestamp_nanos_opt().unwrap()
    ));
    let storage = Storage::open(&path).unwrap();
    let leader = "0x2222222222222222222222222222222222222222";
    let token_id = "123";

    let mut submitted_buy = intent(
        "live-buy-submitted",
        leader,
        token_id,
        TradeSide::Buy,
        dec!(0.5),
        dec!(50),
    );
    submitted_buy.mode = "live".to_string();
    submitted_buy.verdict = IntentVerdict::Live;
    let mut failed_buy = intent(
        "live-buy-failed",
        leader,
        token_id,
        TradeSide::Buy,
        dec!(0.5),
        dec!(80),
    );
    failed_buy.mode = "live".to_string();
    failed_buy.verdict = IntentVerdict::Live;
    let mut submitted_sell = intent(
        "live-sell-submitted",
        leader,
        token_id,
        TradeSide::Sell,
        dec!(0.5),
        dec!(20),
    );
    submitted_sell.mode = "live".to_string();
    submitted_sell.verdict = IntentVerdict::Live;

    storage.insert_copy_intent(&submitted_buy).unwrap();
    storage
        .insert_live_attempt(
            &submitted_buy.intent_id,
            "submitted",
            &serde_json::json!({}),
            Some(&serde_json::json!({})),
        )
        .unwrap();
    storage.insert_copy_intent(&failed_buy).unwrap();
    storage
        .insert_live_attempt(
            &failed_buy.intent_id,
            "failed",
            &serde_json::json!({}),
            Some(&serde_json::json!({})),
        )
        .unwrap();
    storage.insert_copy_intent(&submitted_sell).unwrap();
    storage
        .insert_live_attempt(
            &submitted_sell.intent_id,
            "submitted",
            &serde_json::json!({}),
            Some(&serde_json::json!({})),
        )
        .unwrap();

    assert_eq!(
        storage
            .leader_market_open_notional(leader, Some("condition-1"))
            .unwrap(),
        dec!(30)
    );

    let _ = std::fs::remove_file(path);
}

#[test]
fn token_open_shares_can_include_submitted_live_exposure() {
    let path = std::env::temp_dir().join(format!(
        "polyfollow-live-shares-{}.sqlite",
        chrono::Utc::now().timestamp_nanos_opt().unwrap()
    ));
    let storage = Storage::open(&path).unwrap();
    let leader = "0x2222222222222222222222222222222222222222";
    let token_id = "123";

    let mut buy = intent(
        "live-share-buy",
        leader,
        token_id,
        TradeSide::Buy,
        dec!(0.5),
        dec!(50),
    );
    buy.mode = "live".to_string();
    buy.verdict = IntentVerdict::Live;
    let mut sell = intent(
        "live-share-sell",
        leader,
        token_id,
        TradeSide::Sell,
        dec!(0.5),
        dec!(20),
    );
    sell.mode = "live".to_string();
    sell.verdict = IntentVerdict::Live;

    storage.insert_copy_intent(&buy).unwrap();
    storage
        .insert_live_attempt(
            &buy.intent_id,
            "submitted",
            &serde_json::json!({}),
            Some(&serde_json::json!({})),
        )
        .unwrap();
    storage.insert_copy_intent(&sell).unwrap();
    storage
        .insert_live_attempt(
            &sell.intent_id,
            "submitted",
            &serde_json::json!({}),
            Some(&serde_json::json!({})),
        )
        .unwrap();

    assert_eq!(
        storage
            .leader_token_open_shares(leader, Some(token_id), false)
            .unwrap(),
        Decimal::ZERO
    );
    assert_eq!(
        storage
            .leader_token_open_shares(leader, Some(token_id), true)
            .unwrap(),
        dec!(60)
    );

    let _ = std::fs::remove_file(path);
}

fn intent(
    trade_id: &str,
    leader: &str,
    token_id: &str,
    side: TradeSide,
    price: Decimal,
    notional: Decimal,
) -> CopyIntent {
    CopyIntent {
        intent_id: format!("intent:{leader}:{trade_id}"),
        leader_address: leader.to_string(),
        trade_id: trade_id.to_string(),
        mode: "paper".to_string(),
        side,
        market_id: Some("condition-1".to_string()),
        token_id: Some(token_id.to_string()),
        target_price: Some(price),
        notional_usdc: notional,
        shares: Some(notional / price),
        verdict: IntentVerdict::Paper,
        reasons: Vec::new(),
        created_at: Utc::now(),
    }
}
