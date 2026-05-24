use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::{Connection, params};
use rust_decimal::Decimal;
use serde::Serialize;

use crate::types::{
    CopyIntent, IntentVerdict, LeaderTrade, PaperExecutionResult, PaperFill, TradeSide,
};

#[derive(Debug, Serialize)]
pub struct StorageStatus {
    pub db_path: String,
    pub leader_count: i64,
    pub processed_trade_count: i64,
    pub copy_intent_count: i64,
    pub paper_fill_count: i64,
    pub live_order_attempt_count: i64,
}

#[derive(Debug, Serialize)]
pub struct IntentRow {
    pub intent_id: String,
    pub leader_address: String,
    pub trade_id: String,
    pub mode: String,
    pub side: String,
    pub market_id: Option<String>,
    pub token_id: Option<String>,
    pub target_price: Option<String>,
    pub notional_usdc: String,
    pub shares: Option<String>,
    pub verdict: String,
    pub reasons_json: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct TradeLogRow {
    pub leader_address: String,
    pub trade_id: String,
    pub source: String,
    pub status: String,
    pub observed_at: String,
}

#[derive(Debug, Serialize)]
pub struct PnlSummary {
    pub open_paper_fills: i64,
    pub closed_paper_fills: i64,
    pub open_notional_usdc: String,
    pub realized_pnl_usdc: String,
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

    pub fn apply_paper_intent(&mut self, intent: &CopyIntent) -> Result<PaperExecutionResult> {
        match intent.side {
            TradeSide::Buy => {
                let Some(fill) = crate::execution::paper_fill_for(intent) else {
                    return Ok(PaperExecutionResult {
                        opened_fills: 0,
                        closed_lots: 0,
                        realized_pnl_usdc: Decimal::ZERO,
                    });
                };
                self.insert_paper_fill(&fill)?;
                Ok(PaperExecutionResult {
                    opened_fills: 1,
                    closed_lots: 0,
                    realized_pnl_usdc: Decimal::ZERO,
                })
            }
            TradeSide::Sell => self.close_paper_fifo(intent),
        }
    }

    pub fn leader_token_open_shares(
        &self,
        leader_address: &str,
        token_id: Option<&str>,
    ) -> Result<Decimal> {
        let Some(token_id) = token_id else {
            return Ok(Decimal::ZERO);
        };
        let value: Option<String> = self.conn.query_row(
            r#"
            SELECT CAST(COALESCE(SUM(CAST(p.shares AS REAL)), 0) AS TEXT)
            FROM paper_fills p
            JOIN copy_intents i ON i.intent_id = p.intent_id
            WHERE i.leader_address = ?1
              AND i.token_id = ?2
              AND i.side = 'buy'
              AND p.status = 'open'
            "#,
            params![leader_address, token_id],
            |row| row.get(0),
        )?;
        Ok(parse_decimal_or_zero(value))
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

    fn close_paper_fifo(&mut self, intent: &CopyIntent) -> Result<PaperExecutionResult> {
        if intent.verdict != IntentVerdict::Paper {
            return Ok(PaperExecutionResult {
                opened_fills: 0,
                closed_lots: 0,
                realized_pnl_usdc: Decimal::ZERO,
            });
        }
        let token_id = intent
            .token_id
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("paper sell requires token_id"))?;
        let exit_price = intent
            .target_price
            .ok_or_else(|| anyhow::anyhow!("paper sell requires target_price"))?;
        let mut remaining = intent
            .shares
            .ok_or_else(|| anyhow::anyhow!("paper sell requires shares"))?;
        if remaining <= Decimal::ZERO {
            return Ok(PaperExecutionResult {
                opened_fills: 0,
                closed_lots: 0,
                realized_pnl_usdc: Decimal::ZERO,
            });
        }

        let tx = self.conn.transaction()?;
        let lots = {
            let mut stmt = tx.prepare(
                r#"
                SELECT p.paper_fill_id, p.intent_id, p.entry_price, p.shares,
                       p.notional_usdc, p.opened_at
                FROM paper_fills p
                JOIN copy_intents i ON i.intent_id = p.intent_id
                WHERE i.leader_address = ?1
                  AND i.token_id = ?2
                  AND i.side = 'buy'
                  AND p.status = 'open'
                ORDER BY p.opened_at ASC, p.paper_fill_id ASC
                "#,
            )?;
            let rows = stmt.query_map(params![intent.leader_address, token_id], |row| {
                Ok(OpenPaperLot {
                    paper_fill_id: row.get(0)?,
                    intent_id: row.get(1)?,
                    entry_price: parse_decimal_or_zero(row.get::<_, Option<String>>(2)?),
                    shares: parse_decimal_or_zero(row.get::<_, Option<String>>(3)?),
                    notional_usdc: parse_decimal_or_zero(row.get::<_, Option<String>>(4)?),
                    opened_at: row.get(5)?,
                })
            })?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };

        let mut closed_lots = 0;
        let mut realized_pnl_usdc = Decimal::ZERO;
        for lot in lots {
            if remaining <= Decimal::ZERO {
                break;
            }
            if lot.shares <= Decimal::ZERO {
                continue;
            }
            let close_shares = min_decimal(remaining, lot.shares);
            let cost_basis = if lot.entry_price > Decimal::ZERO {
                lot.entry_price * close_shares
            } else {
                lot.notional_usdc * close_shares / lot.shares
            };
            let proceeds = exit_price * close_shares;
            let pnl = proceeds - cost_basis;
            let pnl_bps = if cost_basis > Decimal::ZERO {
                pnl / cost_basis * Decimal::from(10_000)
            } else {
                Decimal::ZERO
            };
            tx.execute(
                r#"
                UPDATE paper_fills
                SET shares = ?1,
                    notional_usdc = ?2,
                    status = 'closed',
                    exit_price = ?3,
                    exit_at = datetime('now'),
                    pnl_usdc = ?4,
                    pnl_bps = ?5
                WHERE paper_fill_id = ?6
                "#,
                params![
                    close_shares.to_string(),
                    cost_basis.to_string(),
                    exit_price.to_string(),
                    pnl.to_string(),
                    pnl_bps.to_string(),
                    lot.paper_fill_id,
                ],
            )?;

            let residual_shares = lot.shares - close_shares;
            if residual_shares > Decimal::ZERO {
                let residual_notional = lot.notional_usdc - cost_basis;
                let residual_id = format!(
                    "paper:remaining:{}:{}",
                    lot.paper_fill_id,
                    chrono::Utc::now().timestamp_micros()
                );
                tx.execute(
                    r#"
                    INSERT INTO paper_fills (
                        paper_fill_id, intent_id, entry_price, shares, notional_usdc,
                        status, opened_at
                    )
                    VALUES (?1, ?2, ?3, ?4, ?5, 'open', ?6)
                    "#,
                    params![
                        residual_id,
                        lot.intent_id,
                        lot.entry_price.to_string(),
                        residual_shares.to_string(),
                        residual_notional.to_string(),
                        lot.opened_at,
                    ],
                )?;
            }

            remaining -= close_shares;
            closed_lots += 1;
            realized_pnl_usdc += pnl;
        }
        tx.commit()?;
        Ok(PaperExecutionResult {
            opened_fills: 0,
            closed_lots,
            realized_pnl_usdc,
        })
    }
}

struct OpenPaperLot {
    paper_fill_id: String,
    intent_id: String,
    entry_price: Decimal,
    shares: Decimal,
    notional_usdc: Decimal,
    opened_at: String,
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

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use rust_decimal_macros::dec;

    use super::*;

    #[test]
    fn paper_sell_closes_fifo_and_records_realized_pnl() {
        let path = std::env::temp_dir().join(format!(
            "polyfollow-fifo-{}.sqlite",
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        let mut storage = Storage::open(&path).unwrap();
        let leader = "0x2222222222222222222222222222222222222222";
        let token_id = "123";

        let buy_one = intent(
            "buy-1",
            leader,
            token_id,
            TradeSide::Buy,
            dec!(0.4),
            dec!(40),
        );
        let buy_two = intent(
            "buy-2",
            leader,
            token_id,
            TradeSide::Buy,
            dec!(0.6),
            dec!(60),
        );
        storage.insert_copy_intent(&buy_one).unwrap();
        storage.apply_paper_intent(&buy_one).unwrap();
        storage.insert_copy_intent(&buy_two).unwrap();
        storage.apply_paper_intent(&buy_two).unwrap();

        let mut sell = intent(
            "sell-1",
            leader,
            token_id,
            TradeSide::Sell,
            dec!(0.5),
            dec!(75),
        );
        sell.shares = Some(dec!(150));
        storage.insert_copy_intent(&sell).unwrap();
        let result = storage.apply_paper_intent(&sell).unwrap();

        assert_eq!(result.closed_lots, 2);
        assert_eq!(result.realized_pnl_usdc, dec!(5.0));
        assert_eq!(
            storage
                .leader_token_open_shares(leader, Some(token_id))
                .unwrap(),
            dec!(50)
        );
        assert_eq!(storage.pnl_summary().unwrap().realized_pnl_usdc, "5.0");

        let _ = std::fs::remove_file(path);
    }

    fn intent(
        trade_id: &str,
        leader: &str,
        token_id: &str,
        side: TradeSide,
        price: Decimal,
        notional: Decimal,
    ) -> CopyIntent {
        CopyIntent {
            intent_id: format!("intent:{leader}:{trade_id}"),
            leader_address: leader.to_string(),
            trade_id: trade_id.to_string(),
            mode: "paper".to_string(),
            side,
            market_id: Some("condition-1".to_string()),
            token_id: Some(token_id.to_string()),
            target_price: Some(price),
            notional_usdc: notional,
            shares: Some(notional / price),
            verdict: IntentVerdict::Paper,
            reasons: Vec::new(),
            created_at: Utc::now(),
        }
    }
}
