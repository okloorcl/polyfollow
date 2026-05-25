use rust_decimal::Decimal;
use rust_decimal_macros::dec;

use super::*;

fn valid_leader() -> LeaderConfig {
    LeaderConfig {
        address: "0x2222222222222222222222222222222222222222".to_string(),
        label: None,
        account_name: None,
        enabled: true,
        copy: CopyConfig::default(),
        risk: LeaderRiskConfig::default(),
        filters: Default::default(),
    }
}

#[test]
fn validate_rejects_negative_leader_risk_caps() {
    let mut cfg = AppConfig {
        leaders: vec![valid_leader()],
        ..Default::default()
    };
    cfg.leaders[0].risk.max_daily_usdc = dec!(-1);

    assert!(cfg.validate().is_err());
}

#[test]
fn validate_rejects_zero_account_capital() {
    let mut cfg = AppConfig::default();
    cfg.account.max_capital_usdc = Decimal::ZERO;

    assert!(cfg.validate().is_err());
}
