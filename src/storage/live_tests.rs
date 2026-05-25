use rust_decimal_macros::dec;

use super::*;
use crate::types::TradeSide;

#[test]
fn recent_live_attempts_extracts_exchange_response_fields() {
    let path = std::env::temp_dir().join(format!(
        "polyfollow-live-attempts-{}.sqlite",
        chrono::Utc::now().timestamp_nanos_opt().unwrap()
    ));
    let storage = Storage::open(&path).unwrap();
    let intent = super::tests::intent(
        "live-audit-1",
        "0x2222222222222222222222222222222222222222",
        "123",
        TradeSide::Buy,
        dec!(0.5),
        dec!(20),
    );
    storage.insert_copy_intent(&intent).unwrap();
    storage
        .insert_live_attempt(
            &intent.intent_id,
            "submitted",
            &serde_json::json!({"side": "buy"}),
            Some(&serde_json::json!({
                "order_id": "0xorder",
                "status": "MATCHED",
                "success": true,
                "error_msg": null,
                "transaction_hashes": ["0xtx"]
            })),
        )
        .unwrap();

    let rows = storage.recent_live_attempts(10).unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].intent_id, intent.intent_id);
    assert_eq!(rows[0].status, "submitted");
    assert_eq!(rows[0].order_id.as_deref(), Some("0xorder"));
    assert_eq!(rows[0].exchange_status.as_deref(), Some("MATCHED"));
    assert_eq!(rows[0].success, Some(true));
    assert_eq!(rows[0].transaction_hashes, vec!["0xtx"]);

    let _ = std::fs::remove_file(path);
}
