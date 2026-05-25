use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::params;
use rust_decimal::Decimal;

use super::{Storage, max_decimal, parse_decimal_or_zero};

impl Storage {
    pub fn open_position_count(&self) -> Result<u32> {
        let paper_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM paper_fills WHERE status = 'open'",
            [],
            |row| row.get(0),
        )?;
        let live_count: i64 = self.conn.query_row(
            r#"
            SELECT COUNT(*)
            FROM (
                SELECT COALESCE(i.market_id, i.token_id, i.intent_id) AS position_key,
                       SUM(CASE i.side WHEN 'buy' THEN 1 WHEN 'sell' THEN -1 ELSE 0 END) AS net
                FROM copy_intents i
                JOIN live_order_attempts a ON a.intent_id = i.intent_id
                WHERE i.verdict = 'live'
                  AND a.status = 'submitted'
                GROUP BY position_key
                HAVING net > 0
            )
            "#,
            [],
            |row| row.get(0),
        )?;
        Ok((paper_count.max(0) + live_count.max(0)) as u32)
    }

    pub fn leader_daily_notional(&self, leader_address: &str) -> Result<Decimal> {
        self.leader_daily_notional_at(leader_address, Utc::now())
    }

    pub fn leader_daily_notional_at(
        &self,
        leader_address: &str,
        at: DateTime<Utc>,
    ) -> Result<Decimal> {
        let value: Option<String> = self.conn.query_row(
            r#"
            SELECT CAST(COALESCE(SUM(CAST(notional_usdc AS REAL)), 0) AS TEXT)
            FROM copy_intents
            WHERE leader_address = ?1
              AND verdict IN ('paper', 'live')
              AND date(created_at) = date(?2)
            "#,
            params![leader_address, at.to_rfc3339()],
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
        let paper_value: Option<String> = self.conn.query_row(
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
        let live_value: Option<String> = self.conn.query_row(
            r#"
            SELECT CAST(COALESCE(SUM(
                CASE i.side
                    WHEN 'buy' THEN CAST(i.notional_usdc AS REAL)
                    WHEN 'sell' THEN -CAST(i.notional_usdc AS REAL)
                    ELSE 0
                END
            ), 0) AS TEXT)
            FROM copy_intents i
            JOIN live_order_attempts a ON a.intent_id = i.intent_id
            WHERE i.leader_address = ?1
              AND i.market_id = ?2
              AND i.verdict = 'live'
              AND a.status = 'submitted'
            "#,
            params![leader_address, market_id],
            |row| row.get(0),
        )?;
        Ok(parse_decimal_or_zero(paper_value)
            + max_decimal(parse_decimal_or_zero(live_value), Decimal::ZERO))
    }
}
