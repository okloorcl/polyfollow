use anyhow::Result;
use rusqlite::params;
use serde::Serialize;

use crate::types::{CopyIntent, LeaderTrade, PaperFill};

use super::{Storage, verdict_label};

impl Storage {
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
}
