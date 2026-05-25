use anyhow::Result;
use rust_decimal::Decimal;

mod admin;
mod follow;
mod leaders;
mod reports;
mod responses;
mod support;

use crate::allocation;
use crate::backtest;
use crate::chain;
use crate::cli::{Cli, Command};
use crate::config::{self, ExecutionMode};
use crate::cooldown;
use crate::dashboard;
use crate::execution::LiveExecutionConfig;
use crate::marketbridge;
use crate::server;
use crate::storage::Storage;
use crate::watch;

use self::admin::{handle_config, handle_doctor, handle_setup, handle_status};
use self::follow::{run_loop, run_once};
use self::leaders::handle_leader;
use self::reports::{handle_live_attempts, handle_logs, handle_orders, handle_pnl};
use self::responses::RunResponse;
use self::support::{config_path, db_path, print_response, run_mode};

pub async fn run(cli: Cli) -> Result<()> {
    let config_path = config_path(&cli)?;
    let db_override = cli.db.clone();
    let json = cli.json;
    match cli.command {
        Command::Setup(args) => handle_setup(json, config_path, db_override, args),
        Command::Config { command } => handle_config(json, config_path, db_override, command),
        Command::Leader { command } => handle_leader(json, config_path, command),
        Command::Status => handle_status(json, config_path, db_override),
        Command::Doctor => handle_doctor(json, config_path, db_override),
        Command::Run(args) => {
            let cfg = config::load_or_default(&config_path)?;
            let mode = run_mode(args.mode, args.paper, args.live, cfg.global.mode);
            if matches!(mode, ExecutionMode::Live) && !args.confirm_live {
                anyhow::bail!("live mode requires --confirm-live");
            }
            if matches!(mode, ExecutionMode::Live) {
                for leader in cfg.leaders.iter().filter(|leader| leader.enabled) {
                    let account = cfg.account_for_leader(leader)?;
                    LiveExecutionConfig::from_env(account, &cfg.global.clob_base_url)?;
                }
            }
            let mut storage = Storage::open(&db_path(db_override.as_ref(), &cfg))?;
            storage.sync_leaders(&cfg.leaders)?;
            let run_stats = if args.once {
                run_once(&cfg, &mut storage, mode, args.limit).await?
            } else {
                let max_consecutive_errors = args
                    .max_consecutive_errors
                    .unwrap_or(cfg.global.max_consecutive_errors);
                run_loop(&cfg, &mut storage, mode, args.limit, max_consecutive_errors).await?
            };
            let response = RunResponse {
                mode,
                once: args.once,
                enabled_leaders: cfg.leaders.iter().filter(|leader| leader.enabled).count(),
                cycles: run_stats.cycles,
                failed_cycles: run_stats.failed_cycles,
                fetched_trades: run_stats.fetched_trades,
                new_trades: run_stats.new_trades,
                blocked_intents: run_stats.blocked_intents,
                paper_fills: run_stats.paper_fills,
                message: "completed polling".to_string(),
            };
            print_response(json, &response, || {
                println!(
                    "Run: {:?}, enabled leaders={}, once={}, cycles={}, failed_cycles={}, fetched={}, new={}, paper={}, blocked={}",
                    response.mode,
                    response.enabled_leaders,
                    response.once,
                    response.cycles,
                    response.failed_cycles,
                    response.fetched_trades,
                    response.new_trades,
                    response.paper_fills,
                    response.blocked_intents
                );
                println!("{}", response.message);
            })
        }
        Command::Orders(args) => handle_orders(json, config_path, db_override, args),
        Command::Pnl => handle_pnl(json, config_path, db_override),
        Command::LiveAttempts(args) => handle_live_attempts(json, config_path, db_override, args),
        Command::Logs(args) => handle_logs(json, config_path, db_override, args),
        Command::Serve(args) => {
            let cfg = config::load_or_default(&config_path)?;
            let db_path = db_path(db_override.as_ref(), &cfg);
            server::serve(cfg, db_path, &args.addr).await
        }
        Command::WatchClob(args) => watch::watch_clob(args, json).await,
        Command::WatchChain(args) => chain::watch_chain(args, json).await,
        Command::Dashboard(args) => {
            let cfg = config::load_or_default(&config_path)?;
            let storage = Storage::open(&db_path(db_override.as_ref(), &cfg))?;
            dashboard::render_dashboard(&cfg, &storage, &args.out, args.limit)?;
            print_response(json, &serde_json::json!({ "out": args.out }), || {
                println!("Dashboard: {}", args.out.display());
            })
        }
        Command::Backtest(args) => {
            let cfg = config::load_or_default(&config_path)?;
            let report = backtest::run_backtest(&cfg, &args.leader, &args.input)?;
            print_response(json, &report, || {
                println!(
                    "Backtest: leader={} trades={} intents={} fills={} blocked={}",
                    report.leader, report.trades, report.intents, report.fills, report.blocked
                );
                println!("Open notional USDC: {}", report.open_notional_usdc);
                println!("Realized PnL USDC: {}", report.realized_pnl_usdc);
            })
        }
        Command::Allocate(args) => {
            let mut cfg = config::load_or_default(&config_path)?;
            let capital = match args.capital {
                Some(value) => value.parse::<Decimal>()?,
                None => cfg.account.max_capital_usdc,
            };
            let order_fraction = args.order_fraction.parse::<Decimal>()?;
            let daily_fraction = args.daily_fraction.parse::<Decimal>()?;
            let plan =
                allocation::build_allocation_plan(&cfg, capital, order_fraction, daily_fraction)?;
            if args.apply {
                allocation::apply_allocation_plan(&mut cfg, &plan);
                config::save(&config_path, &cfg)?;
            }
            print_response(json, &plan, || {
                println!(
                    "Allocation: capital={} enabled_leaders={}",
                    plan.capital_usdc, plan.enabled_leaders
                );
                for row in &plan.rows {
                    println!(
                        "{} budget={} max_order={} max_daily={}",
                        row.leader_address,
                        row.budget_usdc,
                        row.suggested_max_order_usdc,
                        row.suggested_max_daily_usdc
                    );
                }
            })
        }
        Command::Cooldown(args) => {
            let mut cfg = config::load_or_default(&config_path)?;
            let storage = Storage::open(&db_path(db_override.as_ref(), &cfg))?;
            let report =
                cooldown::audit_and_apply(&mut cfg, &storage, args.blocked_threshold, args.apply)?;
            if args.apply {
                config::save(&config_path, &cfg)?;
            }
            print_response(json, &report, || {
                println!(
                    "Cooldown: threshold={} candidates={}",
                    report.threshold,
                    report.candidates.len()
                );
                for candidate in &report.candidates {
                    println!(
                        "{} blocked={} disabled={}",
                        candidate.leader_address, candidate.blocked_intents, candidate.disabled
                    );
                }
            })
        }
        Command::MarketbridgeContext(args) => {
            let value =
                marketbridge::fetch_agent_context(&args.base_url, &args.symbols, &args.market)
                    .await?;
            print_response(json, &value, || {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&value).unwrap_or_default()
                );
            })
        }
    }
}
