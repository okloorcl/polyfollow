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
            .leader_token_open_shares(leader, Some(token_id))
            .unwrap(),
        dec!(50)
    );
    assert_eq!(storage.pnl_summary().unwrap().realized_pnl_usdc, "5.0");

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
