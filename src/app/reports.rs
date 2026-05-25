use std::path::PathBuf;

use anyhow::Result;

use crate::cli::LimitArgs;
use crate::config;
use crate::storage::Storage;

use super::support::{db_path, print_response};

pub(super) fn handle_orders(
    json: bool,
    config_path: PathBuf,
    db_override: Option<PathBuf>,
    args: LimitArgs,
) -> Result<()> {
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

pub(super) fn handle_live_attempts(
    json: bool,
    config_path: PathBuf,
    db_override: Option<PathBuf>,
    args: LimitArgs,
) -> Result<()> {
    let cfg = config::load_or_default(&config_path)?;
    let storage = Storage::open(&db_path(db_override.as_ref(), &cfg))?;
    let rows = storage.recent_live_attempts(args.limit)?;
    print_response(json, &rows, || {
        if rows.is_empty() {
            println!("No live order attempts yet.");
            return;
        }
        for row in &rows {
            let order_id = row.order_id.as_deref().unwrap_or("-");
            let exchange_status = row.exchange_status.as_deref().unwrap_or("-");
            println!(
                "{} status={} exchange_status={} success={:?} order_id={} txs={} at={}",
                row.intent_id,
                row.status,
                exchange_status,
                row.success,
                order_id,
                row.transaction_hashes.len(),
                row.created_at
            );
        }
    })
}

pub(super) fn handle_pnl(
    json: bool,
    config_path: PathBuf,
    db_override: Option<PathBuf>,
) -> Result<()> {
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

pub(super) fn handle_logs(
    json: bool,
    config_path: PathBuf,
    db_override: Option<PathBuf>,
    args: LimitArgs,
) -> Result<()> {
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
