use std::path::PathBuf;

use anyhow::{Context, Result};
use rust_decimal::Decimal;
use serde::Serialize;

use crate::cli::{
    Cli, Command, ConfigCommand, LeaderAddArgs, LeaderCommand, LeaderUpdateArgs, RunMode,
};
use crate::config::{
    self, AppConfig, CopyConfig, CopyMode, ExecutionMode, LeaderConfig, LeaderRiskConfig,
};
use crate::output::print_json;
use crate::storage::Storage;
use crate::validate::normalize_address;

pub async fn run(cli: Cli) -> Result<()> {
    let config_path = config_path(&cli)?;
    let db_override = cli.db.clone();
    let json = cli.json;
    match cli.command {
        Command::Setup(args) => {
            let mut cfg = if args.force || !config_path.exists() {
                AppConfig::default()
            } else {
                config::load_or_default(&config_path)?
            };
            if let Some(wallet) = args.wallet {
                cfg.account.wallet = Some(normalize_address(&wallet)?);
            }
            config::save(&config_path, &cfg)?;
            let db_path = db_path(db_override.as_ref(), &cfg);
            let mut storage = Storage::open(&db_path)?;
            storage.sync_leaders(&cfg.leaders)?;
            let response = SetupResponse {
                config_path,
                db_path,
                mode: cfg.global.mode,
            };
            print_response(json, &response, || {
                println!("Config: {}", response.config_path.display());
                println!("Database: {}", response.db_path.display());
                println!("Mode: {:?}", response.mode);
            })
        }
        Command::Config { command } => {
            let cfg = config::load_or_default(&config_path)?;
            match command {
                ConfigCommand::Show => {
                    if cli.json {
                        print_json(&cfg)
                    } else {
                        println!("{}", toml::to_string_pretty(&cfg)?);
                        Ok(())
                    }
                }
                ConfigCommand::Path => {
                    let response = ConfigPathResponse {
                        config_path,
                        db_path: db_path(db_override.as_ref(), &cfg),
                    };
                    print_response(json, &response, || {
                        println!("Config: {}", response.config_path.display());
                        println!("Database: {}", response.db_path.display());
                    })
                }
            }
        }
        Command::Leader { command } => handle_leader(json, config_path, command),
        Command::Status => {
            let cfg = config::load_or_default(&config_path)?;
            let mut storage = Storage::open(&db_path(db_override.as_ref(), &cfg))?;
            storage.sync_leaders(&cfg.leaders)?;
            let response = StatusResponse {
                mode: cfg.global.mode,
                kill_switch: cfg.global.kill_switch,
                configured_leaders: cfg.leaders.len(),
                enabled_leaders: cfg.leaders.iter().filter(|leader| leader.enabled).count(),
                storage: storage.status()?,
            };
            print_response(json, &response, || {
                println!("Mode: {:?}", response.mode);
                println!("Kill switch: {}", response.kill_switch);
                println!(
                    "Leaders: {} configured, {} enabled",
                    response.configured_leaders, response.enabled_leaders
                );
                println!("Database: {}", response.storage.db_path);
                println!(
                    "Audit rows: processed_trades={}, intents={}, paper_fills={}, live_attempts={}",
                    response.storage.processed_trade_count,
                    response.storage.copy_intent_count,
                    response.storage.paper_fill_count,
                    response.storage.live_order_attempt_count
                );
            })
        }
        Command::Doctor => {
            let cfg = config::load_or_default(&config_path)?;
            cfg.validate()?;
            let mut storage = Storage::open(&db_path(db_override.as_ref(), &cfg))?;
            storage.sync_leaders(&cfg.leaders)?;
            let warnings = doctor_warnings(&cfg);
            let response = DoctorResponse {
                ok: warnings.is_empty(),
                warnings,
            };
            print_response(json, &response, || {
                if response.ok {
                    println!("Doctor: ok");
                } else {
                    println!("Doctor: warnings");
                    for warning in &response.warnings {
                        println!("- {warning}");
                    }
                }
            })
        }
        Command::Run(args) => {
            let cfg = config::load_or_default(&config_path)?;
            let mode = run_mode(args.mode, args.paper, args.live, cfg.global.mode);
            if matches!(mode, ExecutionMode::Live) && !args.confirm_live {
                anyhow::bail!("live mode requires --confirm-live");
            }
            if matches!(mode, ExecutionMode::Live) {
                anyhow::bail!("live execution is not implemented yet; paper mode is available");
            }
            let response = RunResponse {
                mode,
                once: args.once,
                enabled_leaders: cfg.leaders.iter().filter(|leader| leader.enabled).count(),
                message: "monitor and paper execution will land in the next milestone".to_string(),
            };
            print_response(json, &response, || {
                println!(
                    "Run: {:?}, enabled leaders={}, once={}",
                    response.mode, response.enabled_leaders, response.once
                );
                println!("{}", response.message);
            })
        }
        Command::Orders | Command::Pnl | Command::Logs => {
            anyhow::bail!("this report command will be implemented with paper/live execution")
        }
    }
}

fn handle_leader(json: bool, path: PathBuf, command: LeaderCommand) -> Result<()> {
    let mut cfg = config::load_or_default(&path)?;
    match command {
        LeaderCommand::Add(args) => {
            let leader = leader_from_add(args)?;
            cfg.add_leader(leader)?;
            config::save(&path, &cfg)?;
            print_leaders(json, &cfg)
        }
        LeaderCommand::List => print_leaders(json, &cfg),
        LeaderCommand::Remove(args) => {
            let removed = cfg.remove_leader(&args.address)?;
            config::save(&path, &cfg)?;
            print_response(json, &removed, || {
                println!("Removed leader {}", removed.address);
            })
        }
        LeaderCommand::Update(args) => {
            let leader = cfg.leader_mut(&args.address)?;
            apply_leader_update(leader, args)?;
            let leader = leader.clone();
            config::save(&path, &cfg)?;
            print_response(json, &leader, || {
                println!("Updated leader {}", leader.address);
            })
        }
    }
}

fn leader_from_add(args: LeaderAddArgs) -> Result<LeaderConfig> {
    let mut copy = CopyConfig::default();
    if let Some(value) = args.fixed_order {
        copy.mode = CopyMode::Fixed;
        copy.fixed_order_usdc = parse_decimal(&value, "fixed-order")?;
    }
    if let Some(value) = args.copy_ratio {
        copy.mode = CopyMode::Ratio;
        copy.ratio = parse_decimal(&value, "copy-ratio")?;
    }

    let mut risk = LeaderRiskConfig::default();
    if let Some(value) = args.max_order {
        risk.max_order_usdc = parse_decimal(&value, "max-order")?;
    }
    if let Some(value) = args.max_daily {
        risk.max_daily_usdc = parse_decimal(&value, "max-daily")?;
    }
    if let Some(value) = args.max_position {
        risk.max_position_usdc = parse_decimal(&value, "max-position")?;
    }
    risk.support_buy = !args.no_buy;
    risk.support_sell = !args.no_sell;

    Ok(LeaderConfig {
        address: normalize_address(&args.address)?,
        label: args.label,
        enabled: true,
        copy,
        risk,
        filters: crate::config::MarketFilters {
            allow: args.market_allow,
            block: args.market_block,
        },
    })
}

fn apply_leader_update(leader: &mut LeaderConfig, args: LeaderUpdateArgs) -> Result<()> {
    if let Some(label) = args.label {
        leader.label = Some(label);
    }
    if let Some(enabled) = args.enabled {
        leader.enabled = enabled;
    }
    if let Some(value) = args.fixed_order {
        leader.copy.mode = CopyMode::Fixed;
        leader.copy.fixed_order_usdc = parse_decimal(&value, "fixed-order")?;
    }
    if let Some(value) = args.copy_ratio {
        leader.copy.mode = CopyMode::Ratio;
        leader.copy.ratio = parse_decimal(&value, "copy-ratio")?;
    }
    if let Some(value) = args.max_order {
        leader.risk.max_order_usdc = parse_decimal(&value, "max-order")?;
    }
    if let Some(value) = args.max_daily {
        leader.risk.max_daily_usdc = parse_decimal(&value, "max-daily")?;
    }
    if let Some(value) = args.max_position {
        leader.risk.max_position_usdc = parse_decimal(&value, "max-position")?;
    }
    if let Some(value) = args.support_buy {
        leader.risk.support_buy = value;
    }
    if let Some(value) = args.support_sell {
        leader.risk.support_sell = value;
    }
    Ok(())
}

fn print_leaders(json: bool, cfg: &AppConfig) -> Result<()> {
    print_response(json, &cfg.leaders, || {
        if cfg.leaders.is_empty() {
            println!("No leaders configured.");
            return;
        }
        for leader in &cfg.leaders {
            let label = leader.label.as_deref().unwrap_or("-");
            println!(
                "{} [{}] enabled={} mode={:?} max_order={} max_daily={} buy={} sell={}",
                leader.address,
                label,
                leader.enabled,
                leader.copy.mode,
                leader.risk.max_order_usdc,
                leader.risk.max_daily_usdc,
                leader.risk.support_buy,
                leader.risk.support_sell
            );
        }
    })
}

fn run_mode(
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

fn doctor_warnings(cfg: &AppConfig) -> Vec<String> {
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

fn config_path(cli: &Cli) -> Result<PathBuf> {
    match &cli.config {
        Some(path) => Ok(path.clone()),
        None => config::default_config_path(),
    }
}

fn db_path(db_override: Option<&PathBuf>, cfg: &AppConfig) -> PathBuf {
    db_override
        .cloned()
        .unwrap_or_else(|| cfg.global.db_path.clone())
}

fn parse_decimal(value: &str, field: &str) -> Result<Decimal> {
    value
        .parse::<Decimal>()
        .with_context(|| format!("invalid decimal for --{field}: {value}"))
}

fn print_response<T, F>(json: bool, value: &T, human: F) -> Result<()>
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

#[derive(Debug, Serialize)]
struct SetupResponse {
    config_path: PathBuf,
    db_path: PathBuf,
    mode: ExecutionMode,
}

#[derive(Debug, Serialize)]
struct ConfigPathResponse {
    config_path: PathBuf,
    db_path: PathBuf,
}

#[derive(Debug, Serialize)]
struct StatusResponse {
    mode: ExecutionMode,
    kill_switch: bool,
    configured_leaders: usize,
    enabled_leaders: usize,
    storage: crate::storage::StorageStatus,
}

#[derive(Debug, Serialize)]
struct DoctorResponse {
    ok: bool,
    warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
struct RunResponse {
    mode: ExecutionMode,
    once: bool,
    enabled_leaders: usize,
    message: String,
}
