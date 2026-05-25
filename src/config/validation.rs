use anyhow::{Context, Result};
use rust_decimal::Decimal;

use crate::validate::validate_address;

use super::{AccountConfig, AppConfig};

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
            validate_leader_risk(leader)?;
        }
        Ok(())
    }
}

fn validate_leader_risk(leader: &super::LeaderConfig) -> Result<()> {
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
    Ok(())
}

fn validate_account(account: &AccountConfig, label: &str) -> Result<()> {
    if account.max_capital_usdc <= Decimal::ZERO {
        anyhow::bail!("{label}.max_capital_usdc must be positive");
    }
    crate::execution::parse_signature_type(&account.signature_type)
        .with_context(|| format!("{label}.signature_type is invalid"))?;
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
