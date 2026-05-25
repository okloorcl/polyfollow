use std::path::PathBuf;

use anyhow::Result;

use crate::cli::{ConfigCommand, SetupArgs};
use crate::config::{self, AppConfig};
use crate::output::print_json;
use crate::storage::Storage;
use crate::validate::normalize_address;

use super::responses::{ConfigPathResponse, DoctorResponse, SetupResponse, StatusResponse};
use super::support::{db_path, doctor_warnings, print_response};

pub(super) fn handle_setup(
    json: bool,
    config_path: PathBuf,
    db_override: Option<PathBuf>,
    args: SetupArgs,
) -> Result<()> {
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

pub(super) fn handle_config(
    json: bool,
    config_path: PathBuf,
    db_override: Option<PathBuf>,
    command: ConfigCommand,
) -> Result<()> {
    let cfg = config::load_or_default(&config_path)?;
    match command {
        ConfigCommand::Show => {
            if json {
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

pub(super) fn handle_status(
    json: bool,
    config_path: PathBuf,
    db_override: Option<PathBuf>,
) -> Result<()> {
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

pub(super) fn handle_doctor(
    json: bool,
    config_path: PathBuf,
    db_override: Option<PathBuf>,
) -> Result<()> {
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
