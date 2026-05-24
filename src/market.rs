use anyhow::{Context, Result};
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::types::TradeSide;

#[derive(Debug, Clone)]
pub struct BookMetrics {
    pub best_bid: Option<Decimal>,
    pub best_ask: Option<Decimal>,
    pub spread_bps: Option<Decimal>,
    pub executable_depth_usdc: Decimal,
}

#[derive(Clone)]
pub struct OrderBookClient {
    client: reqwest::Client,
    base_url: String,
}

impl OrderBookClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.into().trim_end_matches('/').to_string(),
        }
    }

    pub async fn metrics_for(
        &self,
        token_id: &str,
        side: TradeSide,
        target_notional: Decimal,
    ) -> Result<BookMetrics> {
        let url = format!("{}/book", self.base_url);
        let book = self
            .client
            .get(url)
            .query(&[("token_id", token_id)])
            .send()
            .await
            .context("failed to request CLOB order book")?
            .error_for_status()
            .context("CLOB order book returned an error status")?
            .json::<OrderBook>()
            .await
            .context("failed to decode CLOB order book")?;
        Ok(book.into_metrics(side, target_notional))
    }
}

#[derive(Debug, Deserialize)]
struct OrderBook {
    #[serde(default)]
    bids: Vec<BookLevel>,
    #[serde(default)]
    asks: Vec<BookLevel>,
}

#[derive(Debug, Deserialize)]
struct BookLevel {
    price: Decimal,
    size: Decimal,
}

impl OrderBook {
    fn into_metrics(self, side: TradeSide, target_notional: Decimal) -> BookMetrics {
        let best_bid = self.bids.iter().map(|level| level.price).max();
        let best_ask = self.asks.iter().map(|level| level.price).min();
        let spread_bps = best_bid.zip(best_ask).and_then(|(bid, ask)| {
            let mid = (bid + ask) / Decimal::from(2);
            (mid > Decimal::ZERO).then_some((ask - bid) / mid * Decimal::from(10_000))
        });
        let levels = match side {
            TradeSide::Buy => self.asks,
            TradeSide::Sell => self.bids,
        };
        let executable_depth_usdc = executable_depth(levels, target_notional);
        BookMetrics {
            best_bid,
            best_ask,
            spread_bps,
            executable_depth_usdc,
        }
    }
}

fn executable_depth(levels: Vec<BookLevel>, target_notional: Decimal) -> Decimal {
    let mut remaining = target_notional;
    let mut depth = Decimal::ZERO;
    for level in levels {
        if remaining <= Decimal::ZERO {
            break;
        }
        let level_notional = level.price * level.size;
        let used = if level_notional <= remaining {
            level_notional
        } else {
            remaining
        };
        depth += used;
        remaining -= used;
    }
    depth
}
