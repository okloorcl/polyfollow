use std::path::PathBuf;

use anyhow::{Context, Result};
use rust_decimal::Decimal;
use serde::Serialize;

use crate::cli::{
    Cli, Command, ConfigCommand, LeaderAddArgs, LeaderCommand, LeaderUpdateArgs,
    PolyAlphaImportArgs, RunMode,
};
use crate::config::{
    self, AppConfig, CopyConfig, CopyMode, ExecutionMode, LeaderConfig, LeaderRiskConfig,
};
use crate::engine::{RiskContext, build_intent};
use crate::execution::{LiveExecutionConfig, execute_live_market_order};
use crate::market::OrderBookClient;
use crate::monitor::ActivityPoller;
use crate::notify::Notifier;
use crate::output::print_json;
use crate::polyalpha::{PolyAlphaCandidate, load_candidates};
use crate::server;
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
                for leader in cfg.leaders.iter().filter(|leader| leader.enabled) {
                    let account = cfg.account_for_leader(leader)?;
                    LiveExecutionConfig::from_env(account)?;
                }
            }
            let mut storage = Storage::open(&db_path(db_override.as_ref(), &cfg))?;
            storage.sync_leaders(&cfg.leaders)?;
            let run_stats = if args.once {
                run_once(&cfg, &mut storage, mode, args.limit).await?
            } else {
                run_loop(&cfg, &mut storage, mode, args.limit).await?
            };
            let response = RunResponse {
                mode,
                once: args.once,
                enabled_leaders: cfg.leaders.iter().filter(|leader| leader.enabled).count(),
                fetched_trades: run_stats.fetched_trades,
                new_trades: run_stats.new_trades,
                blocked_intents: run_stats.blocked_intents,
                paper_fills: run_stats.paper_fills,
                message: "completed polling".to_string(),
            };
            print_response(json, &response, || {
                println!(
                    "Run: {:?}, enabled leaders={}, once={}, fetched={}, new={}, paper={}, blocked={}",
                    response.mode,
                    response.enabled_leaders,
                    response.once,
                    response.fetched_trades,
                    response.new_trades,
                    response.paper_fills,
                    response.blocked_intents
                );
                println!("{}", response.message);
            })
        }
        Command::Orders(args) => {
            let cfg = config::load_or_default(&config_path)?;
            let storage = Storage::open(&db_path(db_override.as_ref(), &cfg))?;
            let rows = storage.recent_intents(args.limit)?;
            print_response(json, &rows, || {
                if rows.is_empty() {
                    println!("No copy intents yet.");
                    return;
                }
                for row in &rows {
                    println!(
                        "{} {} {} notional={} verdict={} at={}",
                        row.side,
                        row.leader_address,
                        row.trade_id,
                        row.notional_usdc,
                        row.verdict,
                        row.created_at
                    );
                }
            })
        }
        Command::Pnl => {
            let cfg = config::load_or_default(&config_path)?;
            let storage = Storage::open(&db_path(db_override.as_ref(), &cfg))?;
            let summary = storage.pnl_summary()?;
            print_response(json, &summary, || {
                println!("Open paper fills: {}", summary.open_paper_fills);
                println!("Closed paper fills: {}", summary.closed_paper_fills);
                println!("Open notional USDC: {}", summary.open_notional_usdc);
                println!("Realized PnL USDC: {}", summary.realized_pnl_usdc);
            })
        }
        Command::Logs(args) => {
            let cfg = config::load_or_default(&config_path)?;
            let storage = Storage::open(&db_path(db_override.as_ref(), &cfg))?;
            let rows = storage.recent_logs(args.limit)?;
            print_response(json, &rows, || {
                if rows.is_empty() {
                    println!("No observed trades yet.");
                    return;
                }
                for row in &rows {
                    println!(
                        "{} {} source={} status={} at={}",
                        row.leader_address, row.trade_id, row.source, row.status, row.observed_at
                    );
                }
            })
        }
        Command::Serve(args) => {
            let cfg = config::load_or_default(&config_path)?;
            let db_path = db_path(db_override.as_ref(), &cfg);
            server::serve(cfg, db_path, &args.addr).await
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
        LeaderCommand::ImportPolyalpha(args) => import_polyalpha(json, path, &mut cfg, args),
    }
}

fn import_polyalpha(
    json: bool,
    path: PathBuf,
    cfg: &mut AppConfig,
    args: PolyAlphaImportArgs,
) -> Result<()> {
    let min_score = parse_decimal(&args.min_score, "min-score")?;
    let copy_ratio = parse_decimal(&args.copy_ratio, "copy-ratio")?;
    let max_order = parse_decimal(&args.max_order, "max-order")?;
    let max_daily = parse_decimal(&args.max_daily, "max-daily")?;
    let candidates = load_candidates(&args.input, min_score, &args.verdict)?;
    let mut imported = Vec::new();
    let mut skipped_existing = 0usize;

    if !args.dry_run {
        for candidate in &candidates {
            if cfg
                .leaders
                .iter()
                .any(|leader| leader.address.eq_ignore_ascii_case(&candidate.address))
            {
                skipped_existing += 1;
                continue;
            }
            let leader = leader_from_candidate(candidate, copy_ratio, max_order, max_daily);
            cfg.add_leader(leader)?;
            imported.push(candidate.clone());
        }
        config::save(&path, cfg)?;
    }

    let response = PolyAlphaImportResponse {
        dry_run: args.dry_run,
        candidates,
        imported,
        skipped_existing,
    };
    print_response(json, &response, || {
        println!("PolyAlpha candidates: {}", response.candidates.len());
        if response.dry_run {
            println!("Dry run: config was not changed.");
        } else {
            println!("Imported leaders: {}", response.imported.len());
            println!("Skipped existing: {}", response.skipped_existing);
        }
    })
}

fn leader_from_candidate(
    candidate: &PolyAlphaCandidate,
    copy_ratio: Decimal,
    max_order: Decimal,
    max_daily: Decimal,
) -> LeaderConfig {
    LeaderConfig {
        address: candidate.address.clone(),
        label: Some(candidate.label.clone()),
        account_name: None,
        enabled: true,
        copy: CopyConfig {
            mode: CopyMode::Ratio,
            ratio: copy_ratio,
            fixed_order_usdc: CopyConfig::default().fixed_order_usdc,
        },
        risk: LeaderRiskConfig {
            max_order_usdc: max_order,
            max_daily_usdc: max_daily,
            ..LeaderRiskConfig::default()
        },
        filters: Default::default(),
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
        account_name: args.account,
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
    if let Some(account) = args.account {
        leader.account_name = Some(account);
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

async fn run_once(
    cfg: &AppConfig,
    storage: &mut Storage,
    mode: ExecutionMode,
    limit: usize,
) -> Result<RunStats> {
    if cfg.global.kill_switch {
        anyhow::bail!("global kill switch is enabled");
    }

    let poller = ActivityPoller::new(&cfg.global.data_api_base_url);
    let order_books = OrderBookClient::new(&cfg.global.clob_base_url);
    let notifier = Notifier::new(&cfg.notifications);
    let mut stats = RunStats::default();
    for leader in cfg.leaders.iter().filter(|leader| leader.enabled) {
        let trades = poller
            .fetch_trades(leader, limit)
            .await
            .with_context(|| format!("failed to poll leader {}", leader.address))?;
        stats.fetched_trades += trades.len();
        for trade in trades {
            if storage.has_processed_trade(&trade.leader_address, &trade.trade_id)? {
                continue;
            }
            let mut risk_context = RiskContext {
                leader_daily_notional_usdc: storage.leader_daily_notional(&leader.address)?,
                market_open_notional_usdc: storage
                    .leader_market_open_notional(&leader.address, trade.condition_id.as_deref())?,
                available_position_shares: if trade.side == crate::types::TradeSide::Sell {
                    Some(
                        storage
                            .leader_token_open_shares(&leader.address, trade.token_id.as_deref())?,
                    )
                } else {
                    None
                },
                book: None,
                book_error: None,
            };
            if let Some(token_id) = trade.token_id.as_deref() {
                let preview_intent = build_intent(mode, leader, &trade, RiskContext::default());
                match order_books
                    .metrics_for(token_id, trade.side, preview_intent.notional_usdc)
                    .await
                {
                    Ok(book) => risk_context.book = Some(book),
                    Err(error) => {
                        tracing::warn!(%token_id, error = %error, "failed to fetch order book");
                        risk_context.book_error = Some("order book unavailable");
                    }
                }
            }
            let intent = build_intent(mode, leader, &trade, risk_context);
            let inserted = storage.insert_processed_trade(&trade, "observed")?;
            if !inserted {
                continue;
            }
            stats.new_trades += 1;
            storage.insert_copy_intent(&intent)?;
            if intent.verdict == crate::types::IntentVerdict::Paper {
                let result = storage.apply_paper_intent(&intent)?;
                stats.paper_fills += result.opened_fills + result.closed_lots;
                notifier.notify_intent(&intent, Some(&result)).await;
            } else if intent.verdict == crate::types::IntentVerdict::Live {
                let request = serde_json::to_value(&intent)?;
                let account = cfg.account_for_leader(leader)?;
                let live_config = LiveExecutionConfig::from_env(account)?;
                match execute_live_market_order(&live_config, &intent).await {
                    Ok(response) => {
                        storage.insert_live_attempt(
                            &intent.intent_id,
                            "submitted",
                            &request,
                            Some(&response),
                        )?;
                        notifier.notify_intent(&intent, None).await;
                    }
                    Err(error) => {
                        storage.insert_live_attempt(
                            &intent.intent_id,
                            "failed",
                            &request,
                            Some(&serde_json::json!({"error": error.to_string()})),
                        )?;
                        return Err(error);
                    }
                }
            } else {
                stats.blocked_intents += 1;
                notifier.notify_intent(&intent, None).await;
            }
        }
    }
    Ok(stats)
}

async fn run_loop(
    cfg: &AppConfig,
    storage: &mut Storage,
    mode: ExecutionMode,
    limit: usize,
) -> Result<RunStats> {
    let mut total = RunStats::default();
    loop {
        let stats = run_once(cfg, storage, mode, limit).await?;
        total.fetched_trades += stats.fetched_trades;
        total.new_trades += stats.new_trades;
        total.blocked_intents += stats.blocked_intents;
        total.paper_fills += stats.paper_fills;
        println!(
            "cycle: fetched={}, new={}, paper={}, blocked={}",
            stats.fetched_trades, stats.new_trades, stats.paper_fills, stats.blocked_intents
        );
        tokio::time::sleep(std::time::Duration::from_secs(
            cfg.global.poll_interval_secs,
        ))
        .await;
    }
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
struct PolyAlphaImportResponse {
    dry_run: bool,
    candidates: Vec<PolyAlphaCandidate>,
    imported: Vec<PolyAlphaCandidate>,
    skipped_existing: usize,
}

#[derive(Debug, Serialize)]
struct RunResponse {
    mode: ExecutionMode,
    once: bool,
    enabled_leaders: usize,
    fetched_trades: usize,
    new_trades: usize,
    blocked_intents: usize,
    paper_fills: usize,
    message: String,
}

#[derive(Debug, Default)]
struct RunStats {
    fetched_trades: usize,
    new_trades: usize,
    blocked_intents: usize,
    paper_fills: usize,
}
