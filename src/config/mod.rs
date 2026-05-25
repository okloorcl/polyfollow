use std::path::PathBuf;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

mod defaults;
mod leaders;
mod store;
#[cfg(test)]
mod tests;
mod validation;

use defaults::*;

pub use store::{default_config_path, load_or_default, save};

pub const DEFAULT_CONFIG_FILE: &str = "config.toml";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub global: GlobalConfig,
    #[serde(default)]
    pub account: AccountConfig,
    #[serde(default)]
    pub accounts: Vec<AccountConfig>,
    #[serde(default)]
    pub notifications: NotificationConfig,
    #[serde(default)]
    pub leaders: Vec<LeaderConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    #[serde(default = "default_mode")]
    pub mode: ExecutionMode,
    #[serde(default = "default_db_path")]
    pub db_path: PathBuf,
    #[serde(default = "default_data_api_base_url")]
    pub data_api_base_url: String,
    #[serde(default = "default_clob_base_url")]
    pub clob_base_url: String,
    #[serde(default = "default_poll_interval_secs")]
    pub poll_interval_secs: u64,
    #[serde(default = "default_max_consecutive_errors")]
    pub max_consecutive_errors: u32,
    #[serde(default = "default_max_daily_loss")]
    pub max_daily_loss_usdc: Decimal,
    #[serde(default = "default_max_open_positions")]
    pub max_open_positions: u32,
    #[serde(default)]
    pub kill_switch: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountConfig {
    #[serde(default = "default_account_name")]
    pub name: String,
    #[serde(default)]
    pub wallet: Option<String>,
    #[serde(default)]
    pub funder: Option<String>,
    #[serde(default = "default_max_capital")]
    pub max_capital_usdc: Decimal,
    #[serde(default = "default_account_max_daily_loss")]
    pub max_daily_loss_usdc: Decimal,
    #[serde(default = "default_signature_type")]
    pub signature_type: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NotificationConfig {
    #[serde(default)]
    pub webhook_url: Option<String>,
    #[serde(default)]
    pub telegram_bot_token: Option<String>,
    #[serde(default)]
    pub telegram_chat_id: Option<String>,
    #[serde(default)]
    pub notify_blocked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderConfig {
    pub address: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub account_name: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub copy: CopyConfig,
    #[serde(default)]
    pub risk: LeaderRiskConfig,
    #[serde(default)]
    pub filters: MarketFilters,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyConfig {
    #[serde(default)]
    pub mode: CopyMode,
    #[serde(default = "default_copy_ratio")]
    pub ratio: Decimal,
    #[serde(default = "default_fixed_order")]
    pub fixed_order_usdc: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderRiskConfig {
    #[serde(default = "default_leader_max_order")]
    pub max_order_usdc: Decimal,
    #[serde(default = "default_leader_max_daily")]
    pub max_daily_usdc: Decimal,
    #[serde(default = "default_leader_max_position")]
    pub max_position_usdc: Decimal,
    #[serde(default = "default_max_latency_secs")]
    pub max_latency_secs: i64,
    #[serde(default = "default_price_drift_bps")]
    pub max_price_drift_bps: Decimal,
    #[serde(default = "default_spread_bps")]
    pub max_spread_bps: Decimal,
    #[serde(default = "default_min_depth")]
    pub min_depth_usdc: Decimal,
    #[serde(default = "default_true")]
    pub support_buy: bool,
    #[serde(default = "default_true")]
    pub support_sell: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MarketFilters {
    #[serde(default)]
    pub allow: Vec<String>,
    #[serde(default)]
    pub block: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    Paper,
    Live,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CopyMode {
    #[default]
    Ratio,
    Fixed,
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            mode: default_mode(),
            db_path: default_db_path(),
            data_api_base_url: default_data_api_base_url(),
            clob_base_url: default_clob_base_url(),
            poll_interval_secs: default_poll_interval_secs(),
            max_consecutive_errors: default_max_consecutive_errors(),
            max_daily_loss_usdc: default_max_daily_loss(),
            max_open_positions: default_max_open_positions(),
            kill_switch: false,
        }
    }
}

impl Default for AccountConfig {
    fn default() -> Self {
        Self {
            name: default_account_name(),
            wallet: None,
            funder: None,
            max_capital_usdc: default_max_capital(),
            max_daily_loss_usdc: default_account_max_daily_loss(),
            signature_type: default_signature_type(),
        }
    }
}

impl Default for CopyConfig {
    fn default() -> Self {
        Self {
            mode: CopyMode::Ratio,
            ratio: default_copy_ratio(),
            fixed_order_usdc: default_fixed_order(),
        }
    }
}

impl Default for LeaderRiskConfig {
    fn default() -> Self {
        Self {
            max_order_usdc: default_leader_max_order(),
            max_daily_usdc: default_leader_max_daily(),
            max_position_usdc: default_leader_max_position(),
            max_latency_secs: default_max_latency_secs(),
            max_price_drift_bps: default_price_drift_bps(),
            max_spread_bps: default_spread_bps(),
            min_depth_usdc: default_min_depth(),
            support_buy: true,
            support_sell: true,
        }
    }
}
