use std::path::PathBuf;

use anyhow::{Context, Result};
use rust_decimal::Decimal;
use serde::Serialize;

use crate::cli::{Cli, RunMode};
use crate::config::{self, AppConfig, ExecutionMode};
use crate::execution::private_key_env_candidates;
use crate::output::print_json;

pub(super) fn config_path(cli: &Cli) -> Result<PathBuf> {
    match &cli.config {
        Some(path) => Ok(path.clone()),
        None => config::default_config_path(),
    }
}

pub(super) fn db_path(db_override: Option<&PathBuf>, cfg: &AppConfig) -> PathBuf {
    db_override
        .cloned()
        .unwrap_or_else(|| cfg.global.db_path.clone())
}

pub(super) fn parse_decimal(value: &str, field: &str) -> Result<Decimal> {
    value
        .parse::<Decimal>()
        .with_context(|| format!("invalid decimal for --{field}: {value}"))
}

pub(super) fn print_response<T, F>(json: bool, value: &T, human: F) -> Result<()>
where
    T: Serialize,
    F: FnOnce(),
{
    if json {
        print_json(value)
    } else {
        human();
        Ok(())
    }
}

pub(super) fn run_mode(
    mode: Option<RunMode>,
    paper: bool,
    live: bool,
    configured: ExecutionMode,
) -> ExecutionMode {
    if paper {
        return ExecutionMode::Paper;
    }
    if live {
        return ExecutionMode::Live;
    }
    match mode {
        Some(RunMode::Paper) => ExecutionMode::Paper,
        Some(RunMode::Live) => ExecutionMode::Live,
        None => configured,
    }
}

pub(super) fn doctor_warnings(cfg: &AppConfig) -> Vec<String> {
    let mut warnings = Vec::new();
    if cfg.leaders.is_empty() {
        warnings.push("no leaders configured".to_string());
    }
    if !cfg.leaders.iter().any(|leader| leader.enabled) {
        warnings.push("no enabled leaders configured".to_string());
    }
    if cfg.global.kill_switch {
        warnings.push("global.kill_switch is true; execution should remain blocked".to_string());
    }
    for account in live_accounts(cfg) {
        if account.wallet.is_none() {
            warnings.push(format!(
                "account {} wallet is not configured; live trading will not work",
                account.name
            ));
        }
        let env_keys = private_key_env_candidates(&account.name);
        if !env_keys.iter().any(|key| std::env::var(key).is_ok()) {
            warnings.push(format!(
                "account {} private key env is missing; set {}",
                account.name,
                env_keys.join(", ")
            ));
        }
    }
    warnings
}

fn live_accounts(cfg: &AppConfig) -> Vec<&crate::config::AccountConfig> {
    let mut accounts = Vec::new();
    for leader in cfg.leaders.iter().filter(|leader| leader.enabled) {
        let account = cfg.account_for_leader(leader).unwrap_or(&cfg.account);
        if !accounts
            .iter()
            .any(|existing: &&crate::config::AccountConfig| existing.name == account.name)
        {
            accounts.push(account);
        }
    }
    accounts
}

#[cfg(test)]
mod tests {
    use crate::config::{AppConfig, CopyConfig, LeaderConfig, LeaderRiskConfig, MarketFilters};

    use super::*;

    #[test]
    fn doctor_warnings_report_missing_live_account_readiness() {
        let mut cfg = AppConfig {
            leaders: vec![LeaderConfig {
                address: "0x2222222222222222222222222222222222222222".to_string(),
                label: None,
                account_name: None,
                enabled: true,
                copy: CopyConfig::default(),
                risk: LeaderRiskConfig::default(),
                filters: MarketFilters::default(),
            }],
            ..Default::default()
        };
        cfg.account.name = "doctor_missing_env_unique".to_string();

        let warnings = doctor_warnings(&cfg);

        assert!(warnings.iter().any(|warning| warning.contains("wallet")));
        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("POLYFOLLOW_PRIVATE_KEY_DOCTOR_MISSING_ENV_UNIQUE"))
        );
    }
}
