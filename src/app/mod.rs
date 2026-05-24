use anyhow::Result;

mod follow;
mod leaders;
mod responses;
mod support;

use crate::cli::{Cli, Command, ConfigCommand};
use crate::config::{self, AppConfig, ExecutionMode};
use crate::execution::LiveExecutionConfig;
use crate::output::print_json;
use crate::server;
use crate::storage::Storage;
use crate::validate::normalize_address;
use crate::watch;

use self::follow::{run_loop, run_once};
use self::leaders::handle_leader;
use self::responses::{
    ConfigPathResponse, DoctorResponse, RunResponse, SetupResponse, StatusResponse,
};
use self::support::{config_path, db_path, doctor_warnings, print_response, run_mode};

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
        Command::WatchClob(args) => watch::watch_clob(args, json).await,
    }
}
