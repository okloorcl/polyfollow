use chrono::Utc;
use std::str::FromStr;

use anyhow::{Context, Result};
use polymarket_client_sdk::POLYGON;
use polymarket_client_sdk::auth::{LocalSigner, Signer as _};
use polymarket_client_sdk::clob;
use polymarket_client_sdk::clob::types::{Amount, OrderType, Side, SignatureType};
use polymarket_client_sdk::types::{Decimal as SdkDecimal, U256};

use crate::config::AccountConfig;
use crate::types::{CopyIntent, IntentVerdict, PaperFill, TradeSide};

pub fn paper_fill_for(intent: &CopyIntent) -> Option<PaperFill> {
    if intent.verdict != IntentVerdict::Paper || intent.side != TradeSide::Buy {
        return None;
    }
    Some(PaperFill {
        paper_fill_id: format!("paper:{}", intent.intent_id),
        intent_id: intent.intent_id.clone(),
        entry_price: intent.target_price,
        shares: intent.shares,
        notional_usdc: intent.notional_usdc,
        status: "open".to_string(),
        opened_at: Utc::now(),
    })
}

#[derive(Debug, Clone)]
pub struct LiveExecutionConfig {
    pub private_key: String,
    pub signature_type: SignatureType,
}

impl LiveExecutionConfig {
    pub fn from_env(account: &AccountConfig) -> Result<Self> {
        let private_key = std::env::var("POLYFOLLOW_PRIVATE_KEY")
            .or_else(|_| std::env::var("POLYMARKET_PRIVATE_KEY"))
            .context("set POLYFOLLOW_PRIVATE_KEY or POLYMARKET_PRIVATE_KEY for live mode")?;
        Ok(Self {
            private_key,
            signature_type: parse_signature_type(&account.signature_type),
        })
    }
}

pub async fn execute_live_market_order(
    config: &LiveExecutionConfig,
    intent: &CopyIntent,
) -> Result<serde_json::Value> {
    let token_id = intent
        .token_id
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("live order requires token_id"))?;
    let signer = LocalSigner::from_str(&config.private_key)
        .context("invalid private key")?
        .with_chain_id(Some(POLYGON));
    let client = clob::Client::default()
        .authentication_builder(&signer)
        .signature_type(config.signature_type)
        .authenticate()
        .await
        .context("failed to authenticate with Polymarket CLOB")?;

    let side = match intent.side {
        crate::types::TradeSide::Buy => Side::Buy,
        crate::types::TradeSide::Sell => Side::Sell,
    };
    let amount = match intent.side {
        crate::types::TradeSide::Buy => {
            Amount::usdc(SdkDecimal::from_str(&intent.notional_usdc.to_string())?)?
        }
        crate::types::TradeSide::Sell => {
            let shares = intent
                .shares
                .ok_or_else(|| anyhow::anyhow!("sell intent requires shares"))?;
            Amount::shares(SdkDecimal::from_str(&shares.to_string())?)?
        }
    };
    let order = client
        .market_order()
        .token_id(U256::from_str(token_id).context("invalid token id")?)
        .side(side)
        .amount(amount)
        .order_type(OrderType::FAK)
        .build()
        .await?;
    let order = client.sign(&signer, order).await?;
    let response = client.post_order(order).await?;
    Ok(serde_json::json!({
        "debug": format!("{response:?}")
    }))
}

fn parse_signature_type(value: &str) -> SignatureType {
    match value {
        "proxy" => SignatureType::Proxy,
        "gnosis-safe" => SignatureType::GnosisSafe,
        _ => SignatureType::Eoa,
    }
}
