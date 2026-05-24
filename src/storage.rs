use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::{Connection, params};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct StorageStatus {
    pub db_path: String,
    pub leader_count: i64,
    pub processed_trade_count: i64,
    pub copy_intent_count: i64,
    pub paper_fill_count: i64,
    pub live_order_attempt_count: i64,
}

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
        self.conn.execute_batch(
            r#"
            PRAGMA journal_mode = WAL;
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS leaders (
                address TEXT PRIMARY KEY,
                label TEXT,
                enabled INTEGER NOT NULL,
                config_json TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS processed_trades (
                leader_address TEXT NOT NULL,
                trade_id TEXT NOT NULL,
                source TEXT NOT NULL,
                status TEXT NOT NULL,
                trade_json TEXT NOT NULL,
                observed_at TEXT NOT NULL,
                PRIMARY KEY (leader_address, trade_id)
            );

            CREATE TABLE IF NOT EXISTS copy_intents (
                intent_id TEXT PRIMARY KEY,
                leader_address TEXT NOT NULL,
                trade_id TEXT NOT NULL,
                mode TEXT NOT NULL,
                side TEXT NOT NULL,
                market_id TEXT,
                token_id TEXT,
                target_price TEXT,
                notional_usdc TEXT NOT NULL,
                shares TEXT,
                verdict TEXT NOT NULL,
                reasons_json TEXT NOT NULL,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS paper_fills (
                paper_fill_id TEXT PRIMARY KEY,
                intent_id TEXT NOT NULL,
                entry_price TEXT,
                shares TEXT,
                notional_usdc TEXT NOT NULL,
                status TEXT NOT NULL,
                opened_at TEXT NOT NULL,
                exit_price TEXT,
                exit_at TEXT,
                pnl_usdc TEXT,
                pnl_bps TEXT
            );

            CREATE TABLE IF NOT EXISTS live_order_attempts (
                attempt_id TEXT PRIMARY KEY,
                intent_id TEXT NOT NULL,
                status TEXT NOT NULL,
                request_json TEXT NOT NULL,
                response_json TEXT,
                created_at TEXT NOT NULL
            );
            "#,
        )?;
        Ok(())
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

    fn count(&self, table: &str) -> Result<i64> {
        let sql = format!("SELECT COUNT(*) FROM {table}");
        Ok(self.conn.query_row(&sql, [], |row| row.get(0))?)
    }
}
