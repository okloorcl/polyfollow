use std::path::PathBuf;

use rust_decimal::Decimal;
use rust_decimal_macros::dec;

use super::ExecutionMode;

pub(super) fn default_mode() -> ExecutionMode {
    ExecutionMode::Paper
}

pub(super) fn default_db_path() -> PathBuf {
    dirs::data_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("polyfollow")
        .join("polyfollow.sqlite")
}

pub(super) fn default_data_api_base_url() -> String {
    "https://data-api.polymarket.com".to_string()
}

pub(super) fn default_clob_base_url() -> String {
    "https://clob.polymarket.com".to_string()
}

pub(super) fn default_poll_interval_secs() -> u64 {
    2
}

pub(super) fn default_max_consecutive_errors() -> u32 {
    0
}

pub(super) fn default_max_daily_loss() -> Decimal {
    dec!(100)
}

pub(super) fn default_max_open_positions() -> u32 {
    30
}

pub(super) fn default_account_name() -> String {
    "main".to_string()
}

pub(super) fn default_max_capital() -> Decimal {
    dec!(1000)
}

pub(super) fn default_account_max_daily_loss() -> Decimal {
    dec!(50)
}

pub(super) fn default_signature_type() -> String {
    "proxy".to_string()
}

pub(super) fn default_true() -> bool {
    true
}

pub(super) fn default_copy_ratio() -> Decimal {
    dec!(0.10)
}

pub(super) fn default_fixed_order() -> Decimal {
    dec!(10)
}

pub(super) fn default_leader_max_order() -> Decimal {
    dec!(20)
}

pub(super) fn default_leader_max_daily() -> Decimal {
    dec!(100)
}

pub(super) fn default_leader_max_position() -> Decimal {
    dec!(50)
}

pub(super) fn default_max_latency_secs() -> i64 {
    30
}

pub(super) fn default_price_drift_bps() -> Decimal {
    dec!(300)
}

pub(super) fn default_spread_bps() -> Decimal {
    dec!(250)
}

pub(super) fn default_min_depth() -> Decimal {
    dec!(100)
}
