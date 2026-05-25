use rust_decimal::Decimal;

use crate::market::BookMetrics;

#[derive(Debug, Clone, Default)]
pub struct RiskContext {
    pub leader_daily_notional_usdc: Decimal,
    pub market_open_notional_usdc: Decimal,
    pub available_position_shares: Option<Decimal>,
    pub open_positions: Option<u32>,
    pub max_open_positions: Option<u32>,
    pub realized_pnl_today_usdc: Option<Decimal>,
    pub max_daily_loss_usdc: Option<Decimal>,
    pub book: Option<BookMetrics>,
    pub book_error: Option<&'static str>,
}
