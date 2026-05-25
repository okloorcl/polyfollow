use anyhow::Result;
use rust_decimal::Decimal;
use serde::Serialize;

use crate::config::AppConfig;

#[derive(Debug, Clone, Serialize)]
pub struct AllocationPlan {
    pub capital_usdc: Decimal,
    pub enabled_leaders: usize,
    pub rows: Vec<AllocationRow>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AllocationRow {
    pub leader_address: String,
    pub budget_usdc: Decimal,
    pub suggested_max_order_usdc: Decimal,
    pub suggested_max_daily_usdc: Decimal,
}

pub fn build_allocation_plan(
    cfg: &AppConfig,
    capital: Decimal,
    order_fraction: Decimal,
    daily_fraction: Decimal,
) -> Result<AllocationPlan> {
    if capital <= Decimal::ZERO {
        anyhow::bail!("capital must be positive");
    }
    if order_fraction <= Decimal::ZERO || daily_fraction <= Decimal::ZERO {
        anyhow::bail!("fractions must be positive");
    }
    let enabled = cfg
        .leaders
        .iter()
        .filter(|leader| leader.enabled)
        .collect::<Vec<_>>();
    if enabled.is_empty() {
        anyhow::bail!("no enabled leaders to allocate");
    }
    let budget = capital / Decimal::from(enabled.len());
    Ok(AllocationPlan {
        capital_usdc: capital,
        enabled_leaders: enabled.len(),
        rows: enabled
            .into_iter()
            .map(|leader| AllocationRow {
                leader_address: leader.address.clone(),
                budget_usdc: budget,
                suggested_max_order_usdc: budget * order_fraction,
                suggested_max_daily_usdc: budget * daily_fraction,
            })
            .collect(),
    })
}

pub fn apply_allocation_plan(cfg: &mut AppConfig, plan: &AllocationPlan) {
    for row in &plan.rows {
        if let Some(leader) = cfg
            .leaders
            .iter_mut()
            .find(|leader| leader.address.eq_ignore_ascii_case(&row.leader_address))
        {
            leader.risk.max_order_usdc = row.suggested_max_order_usdc;
            leader.risk.max_daily_usdc = row.suggested_max_daily_usdc;
        }
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal_macros::dec;

    use super::*;
    use crate::config::{CopyConfig, LeaderConfig, LeaderRiskConfig};

    #[test]
    fn equal_weight_allocation_uses_enabled_leaders() {
        let cfg = AppConfig {
            leaders: vec![
                LeaderConfig {
                    address: "0x2222222222222222222222222222222222222222".to_string(),
                    label: None,
                    account_name: None,
                    enabled: true,
                    copy: CopyConfig::default(),
                    risk: LeaderRiskConfig::default(),
                    filters: Default::default(),
                },
                LeaderConfig {
                    address: "0x3333333333333333333333333333333333333333".to_string(),
                    label: None,
                    account_name: None,
                    enabled: false,
                    copy: CopyConfig::default(),
                    risk: LeaderRiskConfig::default(),
                    filters: Default::default(),
                },
            ],
            ..Default::default()
        };
        let plan = build_allocation_plan(&cfg, dec!(1000), dec!(0.02), dec!(0.1)).unwrap();
        assert_eq!(plan.enabled_leaders, 1);
        assert_eq!(plan.rows[0].suggested_max_order_usdc, dec!(20.00));
    }
}
