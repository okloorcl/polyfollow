use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

use crate::validate::{normalize_address, validate_address};

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

impl AppConfig {
    pub fn validate(&self) -> Result<()> {
        ensure_positive(
            self.global.poll_interval_secs,
            "global.poll_interval_secs must be positive",
        )?;
        ensure_decimal_non_negative(
            self.global.max_daily_loss_usdc,
            "global.max_daily_loss_usdc must be non-negative",
        )?;
        ensure_positive(
            self.global.max_open_positions,
            "global.max_open_positions must be positive",
        )?;
        validate_account(&self.account, "account")?;
        if let Some(wallet) = &self.account.wallet {
            validate_address(wallet).context("account.wallet is not a valid address")?;
        }
        let mut account_names = std::collections::HashSet::new();
        account_names.insert(self.account.name.clone());
        for account in &self.accounts {
            validate_account(account, &format!("account {}", account.name))?;
            if !account_names.insert(account.name.clone()) {
                anyhow::bail!("duplicate account name: {}", account.name);
            }
            if let Some(wallet) = &account.wallet {
                validate_address(wallet)
                    .with_context(|| format!("account {} wallet is invalid", account.name))?;
            }
        }
        for leader in &self.leaders {
            validate_address(&leader.address)
                .with_context(|| format!("leader address is invalid: {}", leader.address))?;
            if let Some(account_name) = &leader.account_name
                && !account_names.contains(account_name)
            {
                anyhow::bail!(
                    "leader {} references unknown account_name {}",
                    leader.address,
                    account_name
                );
            }
            if leader.copy.ratio < Decimal::ZERO {
                anyhow::bail!("leader {} copy.ratio must be non-negative", leader.address);
            }
            if leader.copy.fixed_order_usdc < Decimal::ZERO {
                anyhow::bail!(
                    "leader {} copy.fixed_order_usdc must be non-negative",
                    leader.address
                );
            }
            if leader.risk.max_order_usdc <= Decimal::ZERO {
                anyhow::bail!("leader {} max_order_usdc must be positive", leader.address);
            }
            if leader.risk.max_daily_usdc <= Decimal::ZERO {
                anyhow::bail!("leader {} max_daily_usdc must be positive", leader.address);
            }
            if leader.risk.max_position_usdc <= Decimal::ZERO {
                anyhow::bail!(
                    "leader {} max_position_usdc must be positive",
                    leader.address
                );
            }
            if leader.risk.max_latency_secs < 0 {
                anyhow::bail!(
                    "leader {} max_latency_secs must be non-negative",
                    leader.address
                );
            }
            if leader.risk.max_price_drift_bps < Decimal::ZERO {
                anyhow::bail!(
                    "leader {} max_price_drift_bps must be non-negative",
                    leader.address
                );
            }
            if leader.risk.max_spread_bps < Decimal::ZERO {
                anyhow::bail!(
                    "leader {} max_spread_bps must be non-negative",
                    leader.address
                );
            }
            if leader.risk.min_depth_usdc < Decimal::ZERO {
                anyhow::bail!(
                    "leader {} min_depth_usdc must be non-negative",
                    leader.address
                );
            }
        }
        Ok(())
    }

    pub fn add_leader(&mut self, mut leader: LeaderConfig) -> Result<()> {
        leader.address = normalize_address(&leader.address)?;
        if self
            .leaders
            .iter()
            .any(|existing| existing.address.eq_ignore_ascii_case(&leader.address))
        {
            anyhow::bail!("leader already exists: {}", leader.address);
        }
        self.leaders.push(leader);
        Ok(())
    }

    pub fn remove_leader(&mut self, address: &str) -> Result<LeaderConfig> {
        let address = normalize_address(address)?;
        let index = self
            .leaders
            .iter()
            .position(|leader| leader.address.eq_ignore_ascii_case(&address))
            .ok_or_else(|| anyhow::anyhow!("leader not found: {address}"))?;
        Ok(self.leaders.remove(index))
    }

    pub fn leader_mut(&mut self, address: &str) -> Result<&mut LeaderConfig> {
        let address = normalize_address(address)?;
        self.leaders
            .iter_mut()
            .find(|leader| leader.address.eq_ignore_ascii_case(&address))
            .ok_or_else(|| anyhow::anyhow!("leader not found: {address}"))
    }

    pub fn account_for_leader(&self, leader: &LeaderConfig) -> Result<&AccountConfig> {
        let Some(account_name) = leader.account_name.as_deref() else {
            return Ok(&self.account);
        };
        if self.account.name == account_name {
            return Ok(&self.account);
        }
        self.accounts
            .iter()
            .find(|account| account.name == account_name)
            .ok_or_else(|| anyhow::anyhow!("unknown account_name: {account_name}"))
    }
}

fn validate_account(account: &AccountConfig, label: &str) -> Result<()> {
    if account.max_capital_usdc <= Decimal::ZERO {
        anyhow::bail!("{label}.max_capital_usdc must be positive");
    }
    ensure_decimal_non_negative(
        account.max_daily_loss_usdc,
        &format!("{label}.max_daily_loss_usdc must be non-negative"),
    )
}

fn ensure_decimal_non_negative(value: Decimal, message: &str) -> Result<()> {
    if value < Decimal::ZERO {
        anyhow::bail!("{message}");
    }
    Ok(())
}

fn ensure_positive<T>(value: T, message: &str) -> Result<()>
where
    T: PartialOrd + From<u8>,
{
    if value <= T::from(0) {
        anyhow::bail!("{message}");
    }
    Ok(())
}

pub fn default_config_path() -> Result<PathBuf> {
    let base = dirs::config_dir()
        .or_else(dirs::home_dir)
        .context("failed to resolve config directory")?;
    Ok(base.join("polyfollow").join(DEFAULT_CONFIG_FILE))
}

pub fn load_or_default(path: &PathBuf) -> Result<AppConfig> {
    if !path.exists() {
        return Ok(AppConfig::default());
    }
    let text =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let config = toml::from_str::<AppConfig>(&text)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    config.validate()?;
    Ok(config)
}

pub fn save(path: &PathBuf, config: &AppConfig) -> Result<()> {
    config.validate()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let text = toml::to_string_pretty(config)?;

    #[cfg(unix)]
    {
        use std::io::Write as _;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)
            .with_context(|| format!("failed to create {}", path.display()))?;
        file.write_all(text.as_bytes())
            .with_context(|| format!("failed to write {}", path.display()))?;
    }

    #[cfg(not(unix))]
    fs::write(path, text).with_context(|| format!("failed to write {}", path.display()))?;

    Ok(())
}

fn default_mode() -> ExecutionMode {
    ExecutionMode::Paper
}

fn default_db_path() -> PathBuf {
    dirs::data_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("polyfollow")
        .join("polyfollow.sqlite")
}

fn default_data_api_base_url() -> String {
    "https://data-api.polymarket.com".to_string()
}

fn default_clob_base_url() -> String {
    "https://clob.polymarket.com".to_string()
}

fn default_poll_interval_secs() -> u64 {
    2
}

fn default_max_daily_loss() -> Decimal {
    dec!(100)
}

fn default_max_open_positions() -> u32 {
    30
}

fn default_account_name() -> String {
    "main".to_string()
}

fn default_max_capital() -> Decimal {
    dec!(1000)
}

fn default_account_max_daily_loss() -> Decimal {
    dec!(50)
}

fn default_signature_type() -> String {
    "proxy".to_string()
}

fn default_true() -> bool {
    true
}

fn default_copy_ratio() -> Decimal {
    dec!(0.10)
}

fn default_fixed_order() -> Decimal {
    dec!(10)
}

fn default_leader_max_order() -> Decimal {
    dec!(20)
}

fn default_leader_max_daily() -> Decimal {
    dec!(100)
}

fn default_leader_max_position() -> Decimal {
    dec!(50)
}

fn default_max_latency_secs() -> i64 {
    30
}

fn default_price_drift_bps() -> Decimal {
    dec!(300)
}

fn default_spread_bps() -> Decimal {
    dec!(250)
}

fn default_min_depth() -> Decimal {
    dec!(100)
}

#[cfg(test)]
mod tests {
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
}
