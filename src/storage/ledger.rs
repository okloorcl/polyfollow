use anyhow::Result;
use rusqlite::params;
use rust_decimal::Decimal;

use crate::types::{CopyIntent, IntentVerdict, PaperExecutionResult, TradeSide};

use super::{Storage, min_decimal, parse_decimal_or_zero};

impl Storage {
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
        include_live: bool,
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
        let paper_shares = parse_decimal_or_zero(value);
        if !include_live {
            return Ok(paper_shares);
        }
        let live_value: Option<String> = self.conn.query_row(
            r#"
            SELECT CAST(COALESCE(SUM(
                CASE i.side
                    WHEN 'buy' THEN CAST(i.shares AS REAL)
                    WHEN 'sell' THEN -CAST(i.shares AS REAL)
                    ELSE 0
                END
            ), 0) AS TEXT)
            FROM copy_intents i
            JOIN live_order_attempts a ON a.intent_id = i.intent_id
            WHERE i.leader_address = ?1
              AND i.token_id = ?2
              AND i.verdict = 'live'
              AND a.status = 'submitted'
            "#,
            params![leader_address, token_id],
            |row| row.get(0),
        )?;
        Ok(paper_shares + super::max_decimal(parse_decimal_or_zero(live_value), Decimal::ZERO))
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
