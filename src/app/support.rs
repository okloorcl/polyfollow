use std::path::PathBuf;

use anyhow::{Context, Result};
use rust_decimal::Decimal;
use serde::Serialize;

use crate::cli::{Cli, RunMode};
use crate::config::{self, AppConfig, ExecutionMode};
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
    if cfg.account.wallet.is_none() {
        warnings.push("account.wallet is not configured; live trading will not work".to_string());
    }
    if cfg.leaders.is_empty() {
        warnings.push("no leaders configured".to_string());
    }
    if cfg.global.kill_switch {
        warnings.push("global.kill_switch is true; execution should remain blocked".to_string());
    }
    warnings
}
