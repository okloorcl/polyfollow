use anyhow::Result;
use rusqlite::Connection;

pub(super) fn init(conn: &Connection) -> Result<()> {
    conn.execute_batch(
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
