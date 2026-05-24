use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::{Connection, params};
use rust_decimal::Decimal;
use serde::Serialize;

mod ledger;
mod rows;
mod schema;
#[cfg(test)]
mod tests;

use crate::types::{CopyIntent, IntentVerdict, LeaderTrade, PaperFill};

pub use rows::{IntentRow, PnlSummary, StorageStatus, TradeLogRow};

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

    pub fn sync_leaders<T: Serialize>(&mut self, leaders: &[T]) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM leaders", [])?;
        for leader in leaders {
            let value = serde_json::to_value(leader)?;
            let address = value
                .get("address")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let label = value.get("label").and_then(|v| v.as_str());
            let enabled = value
                .get("enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            tx.execute(
                r#"
                INSERT INTO leaders (address, label, enabled, config_json, updated_at)
                VALUES (?1, ?2, ?3, ?4, datetime('now'))
                "#,
                params![
                    address,
                    label,
                    i64::from(enabled),
                    serde_json::to_string(leader)?,
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
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

    pub fn has_processed_trade(&self, leader_address: &str, trade_id: &str) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM processed_trades WHERE leader_address = ?1 AND trade_id = ?2",
            params![leader_address, trade_id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn insert_processed_trade(&self, trade: &LeaderTrade, status: &str) -> Result<bool> {
        let rows = self.conn.execute(
            r#"
            INSERT OR IGNORE INTO processed_trades (
                leader_address, trade_id, source, status, trade_json, observed_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            params![
                trade.leader_address,
                trade.trade_id,
                trade.source,
                status,
                serde_json::to_string(trade)?,
                trade.received_at.to_rfc3339(),
            ],
        )?;
        Ok(rows > 0)
    }

    pub fn insert_copy_intent(&self, intent: &CopyIntent) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO copy_intents (
                intent_id, leader_address, trade_id, mode, side, market_id, token_id,
                target_price, notional_usdc, shares, verdict, reasons_json, created_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            "#,
            params![
                intent.intent_id,
                intent.leader_address,
                intent.trade_id,
                intent.mode,
                intent.side.as_str(),
                intent.market_id,
                intent.token_id,
                intent.target_price.map(|value| value.to_string()),
                intent.notional_usdc.to_string(),
                intent.shares.map(|value| value.to_string()),
                verdict_label(intent.verdict),
                serde_json::to_string(&intent.reasons)?,
                intent.created_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn insert_paper_fill(&self, fill: &PaperFill) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO paper_fills (
                paper_fill_id, intent_id, entry_price, shares, notional_usdc,
                status, opened_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                fill.paper_fill_id,
                fill.intent_id,
                fill.entry_price.map(|value| value.to_string()),
                fill.shares.map(|value| value.to_string()),
                fill.notional_usdc.to_string(),
                fill.status,
                fill.opened_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn insert_live_attempt(
        &self,
        intent_id: &str,
        status: &str,
        request: &serde_json::Value,
        response: Option<&serde_json::Value>,
    ) -> Result<()> {
        let attempt_id = format!("live:{intent_id}:{}", chrono::Utc::now().timestamp_millis());
        self.conn.execute(
            r#"
            INSERT INTO live_order_attempts (
                attempt_id, intent_id, status, request_json, response_json, created_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'))
            "#,
            params![
                attempt_id,
                intent_id,
                status,
                serde_json::to_string(request)?,
                response.map(serde_json::to_string).transpose()?,
            ],
        )?;
        Ok(())
    }

    pub fn recent_intents(&self, limit: usize) -> Result<Vec<IntentRow>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT intent_id, leader_address, trade_id, mode, side, market_id, token_id,
                   target_price, notional_usdc, shares, verdict, reasons_json, created_at
            FROM copy_intents
            ORDER BY created_at DESC
            LIMIT ?1
            "#,
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(IntentRow {
                intent_id: row.get(0)?,
                leader_address: row.get(1)?,
                trade_id: row.get(2)?,
                mode: row.get(3)?,
                side: row.get(4)?,
                market_id: row.get(5)?,
                token_id: row.get(6)?,
                target_price: row.get(7)?,
                notional_usdc: row.get(8)?,
                shares: row.get(9)?,
                verdict: row.get(10)?,
                reasons_json: row.get(11)?,
                created_at: row.get(12)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn recent_logs(&self, limit: usize) -> Result<Vec<TradeLogRow>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT leader_address, trade_id, source, status, observed_at
            FROM processed_trades
            ORDER BY observed_at DESC
            LIMIT ?1
            "#,
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(TradeLogRow {
                leader_address: row.get(0)?,
                trade_id: row.get(1)?,
                source: row.get(2)?,
                status: row.get(3)?,
                observed_at: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn pnl_summary(&self) -> Result<PnlSummary> {
        let open_paper_fills: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM paper_fills WHERE status = 'open'",
            [],
            |row| row.get(0),
        )?;
        let closed_paper_fills: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM paper_fills WHERE status != 'open'",
            [],
            |row| row.get(0),
        )?;
        let open_notional_usdc: Option<String> = self.conn.query_row(
            "SELECT CAST(COALESCE(SUM(CAST(notional_usdc AS REAL)), 0) AS TEXT) FROM paper_fills WHERE status = 'open'",
            [],
            |row| row.get(0),
        )?;
        let realized_pnl_usdc: Option<String> = self.conn.query_row(
            "SELECT CAST(COALESCE(SUM(CAST(pnl_usdc AS REAL)), 0) AS TEXT) FROM paper_fills WHERE status != 'open'",
            [],
            |row| row.get(0),
        )?;
        Ok(PnlSummary {
            open_paper_fills,
            closed_paper_fills,
            open_notional_usdc: open_notional_usdc.unwrap_or_else(|| "0".to_string()),
            realized_pnl_usdc: realized_pnl_usdc.unwrap_or_else(|| "0".to_string()),
        })
    }

    pub fn leader_daily_notional(&self, leader_address: &str) -> Result<Decimal> {
        let value: Option<String> = self.conn.query_row(
            r#"
            SELECT CAST(COALESCE(SUM(CAST(p.notional_usdc AS REAL)), 0) AS TEXT)
            FROM paper_fills p
            JOIN copy_intents i ON i.intent_id = p.intent_id
            WHERE i.leader_address = ?1
              AND date(p.opened_at) = date('now')
            "#,
            params![leader_address],
            |row| row.get(0),
        )?;
        Ok(parse_decimal_or_zero(value))
    }

    pub fn leader_market_open_notional(
        &self,
        leader_address: &str,
        market_id: Option<&str>,
    ) -> Result<Decimal> {
        let Some(market_id) = market_id else {
            return Ok(Decimal::ZERO);
        };
        let value: Option<String> = self.conn.query_row(
            r#"
            SELECT CAST(COALESCE(SUM(CAST(p.notional_usdc AS REAL)), 0) AS TEXT)
            FROM paper_fills p
            JOIN copy_intents i ON i.intent_id = p.intent_id
            WHERE i.leader_address = ?1
              AND i.market_id = ?2
              AND p.status = 'open'
            "#,
            params![leader_address, market_id],
            |row| row.get(0),
        )?;
        Ok(parse_decimal_or_zero(value))
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
