use anyhow::Result;
use rusqlite::params;
use serde_json::Value;

use super::{
    IntentRow, LeaderBlockedCount, LiveAttemptRow, PnlSummary, Storage, TradeLogRow,
    parse_decimal_or_zero,
};

impl Storage {
    pub fn has_processed_trade(&self, leader_address: &str, trade_id: &str) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM processed_trades WHERE leader_address = ?1 AND trade_id = ?2",
            params![leader_address, trade_id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
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

    pub fn recent_live_attempts(&self, limit: usize) -> Result<Vec<LiveAttemptRow>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT attempt_id, intent_id, status, response_json, created_at
            FROM live_order_attempts
            ORDER BY created_at DESC, attempt_id DESC
            LIMIT ?1
            "#,
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            let response_json: Option<String> = row.get(3)?;
            let response = response_json
                .as_deref()
                .and_then(|raw| serde_json::from_str::<Value>(raw).ok());
            Ok(LiveAttemptRow {
                attempt_id: row.get(0)?,
                intent_id: row.get(1)?,
                status: row.get(2)?,
                order_id: json_string(&response, "order_id"),
                exchange_status: json_string(&response, "status"),
                success: response
                    .as_ref()
                    .and_then(|value| value.get("success"))
                    .and_then(Value::as_bool),
                error_msg: json_string(&response, "error_msg"),
                transaction_hashes: json_string_array(&response, "transaction_hashes"),
                created_at: row.get(4)?,
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

    pub fn blocked_counts_by_leader(&self) -> Result<Vec<LeaderBlockedCount>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT leader_address, COUNT(*)
            FROM copy_intents
            WHERE verdict = 'blocked'
            GROUP BY leader_address
            ORDER BY COUNT(*) DESC
            "#,
        )?;
        let rows = stmt.query_map([], |row| {
            let count: i64 = row.get(1)?;
            Ok(LeaderBlockedCount {
                leader_address: row.get(0)?,
                blocked_intents: count.max(0) as usize,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn daily_realized_pnl_at(
        &self,
        at: chrono::DateTime<chrono::Utc>,
    ) -> Result<rust_decimal::Decimal> {
        let value: Option<String> = self.conn.query_row(
            r#"
            SELECT CAST(COALESCE(SUM(CAST(pnl_usdc AS REAL)), 0) AS TEXT)
            FROM paper_fills
            WHERE status != 'open'
              AND date(exit_at) = date(?1)
            "#,
            params![at.to_rfc3339()],
            |row| row.get(0),
        )?;
        Ok(parse_decimal_or_zero(value))
    }
}

fn json_string(value: &Option<Value>, key: &str) -> Option<String> {
    value
        .as_ref()
        .and_then(|value| value.get(key))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn json_string_array(value: &Option<Value>, key: &str) -> Vec<String> {
    value
        .as_ref()
        .and_then(|value| value.get(key))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}
