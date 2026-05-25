use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::Connection;
use rust_decimal::Decimal;

mod ledger;
mod queries;
mod risk;
mod rows;
mod schema;
#[cfg(test)]
mod tests;
mod writes;

use crate::types::IntentVerdict;

pub use rows::{IntentRow, LeaderBlockedCount, PnlSummary, StorageStatus, TradeLogRow};

pub struct Storage {
    conn: Connection,
    db_path: String,
}

impl Storage {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        let conn = Connection::open(path)
            .with_context(|| format!("failed to open sqlite {}", path.display()))?;
        let storage = Self {
            conn,
            db_path: path.display().to_string(),
        };
        storage.init()?;
        Ok(storage)
    }

    pub fn init(&self) -> Result<()> {
        schema::init(&self.conn)
    }

    pub fn status(&self) -> Result<StorageStatus> {
        Ok(StorageStatus {
            db_path: self.db_path.clone(),
            leader_count: self.count("leaders")?,
            processed_trade_count: self.count("processed_trades")?,
            copy_intent_count: self.count("copy_intents")?,
            paper_fill_count: self.count("paper_fills")?,
            live_order_attempt_count: self.count("live_order_attempts")?,
        })
    }

    fn count(&self, table: &str) -> Result<i64> {
        let sql = format!("SELECT COUNT(*) FROM {table}");
        Ok(self.conn.query_row(&sql, [], |row| row.get(0))?)
    }
}

fn verdict_label(verdict: IntentVerdict) -> &'static str {
    match verdict {
        IntentVerdict::Paper => "paper",
        IntentVerdict::Live => "live",
        IntentVerdict::Blocked => "blocked",
    }
}

fn parse_decimal_or_zero(value: Option<String>) -> Decimal {
    value
        .and_then(|value| value.parse::<Decimal>().ok())
        .unwrap_or(Decimal::ZERO)
}

fn min_decimal(left: Decimal, right: Decimal) -> Decimal {
    if left <= right { left } else { right }
}

fn max_decimal(left: Decimal, right: Decimal) -> Decimal {
    if left >= right { left } else { right }
}
