use chrono::Utc;
use std::str::FromStr;

use anyhow::{Context, Result};
use polymarket_client_sdk_v2::POLYGON;
use polymarket_client_sdk_v2::auth::{LocalSigner, Signer as _};
use polymarket_client_sdk_v2::clob::types::response::PostOrderResponse;
use polymarket_client_sdk_v2::clob::types::{Amount, OrderType, Side, SignatureType};
use polymarket_client_sdk_v2::clob::{Client, Config};
use polymarket_client_sdk_v2::types::{Address, Decimal as SdkDecimal, U256};

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
    pub clob_base_url: String,
    pub funder: Option<Address>,
    pub signature_type: SignatureType,
}

impl LiveExecutionConfig {
    pub fn from_env(account: &AccountConfig, clob_base_url: &str) -> Result<Self> {
        let env_keys = private_key_env_candidates(&account.name);
        let private_key = std::env::var(&env_keys[0])
            .or_else(|_| std::env::var("POLYFOLLOW_PRIVATE_KEY"))
            .or_else(|_| std::env::var("POLYMARKET_PRIVATE_KEY"))
            .with_context(|| {
                format!(
                    "set {}, POLYFOLLOW_PRIVATE_KEY, or POLYMARKET_PRIVATE_KEY for live mode",
                    env_keys[0]
                )
            })?;
        let signature_type = parse_signature_type(&account.signature_type)?;
        let funder = account
            .funder
            .as_deref()
            .or(account.wallet.as_deref())
            .map(Address::from_str)
            .transpose()
            .context("account funder/wallet is not a valid address")?;
        if requires_funder(signature_type) && funder.is_none() {
            anyhow::bail!(
                "signature_type {} requires account.funder or account.wallet",
                account.signature_type
            );
        }
        Ok(Self {
            private_key,
            clob_base_url: clob_base_url.trim_end_matches('/').to_string(),
            funder,
            signature_type,
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
    let mut auth = Client::new(&config.clob_base_url, Config::default())?
        .authentication_builder(&signer)
        .signature_type(config.signature_type);
    if let Some(funder) = config.funder {
        auth = auth.funder(funder);
    }
    let client = auth
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
    Ok(live_order_response_json(&response))
}

fn live_order_response_json(response: &PostOrderResponse) -> serde_json::Value {
    serde_json::json!({
        "order_id": &response.order_id,
        "success": response.success,
        "status": response.status.to_string(),
        "error_msg": &response.error_msg,
        "making_amount": response.making_amount.to_string(),
        "taking_amount": response.taking_amount.to_string(),
        "transaction_hashes": response
            .transaction_hashes
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        "trade_ids": &response.trade_ids,
    })
}

pub(crate) fn parse_signature_type(value: &str) -> Result<SignatureType> {
    match value {
        "eoa" => Ok(SignatureType::Eoa),
        "proxy" => Ok(SignatureType::Proxy),
        "gnosis-safe" => Ok(SignatureType::GnosisSafe),
        "poly-1271" | "poly1271" | "poly_1271" => Ok(SignatureType::Poly1271),
        _ => {
            anyhow::bail!(
                "unsupported signature_type {value}; expected eoa, proxy, gnosis-safe, or poly-1271"
            )
        }
    }
}

fn requires_funder(signature_type: SignatureType) -> bool {
    matches!(
        signature_type,
        SignatureType::Proxy | SignatureType::GnosisSafe | SignatureType::Poly1271
    )
}

pub(crate) fn private_key_env_candidates(account_name: &str) -> [String; 3] {
    [
        account_private_key_env(account_name),
        "POLYFOLLOW_PRIVATE_KEY".to_string(),
        "POLYMARKET_PRIVATE_KEY".to_string(),
    ]
}

fn account_private_key_env(account_name: &str) -> String {
    let suffix = account_name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    format!("POLYFOLLOW_PRIVATE_KEY_{suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_order_response_json_preserves_audit_fields() {
        let response: PostOrderResponse = serde_json::from_value(serde_json::json!({
            "errorMsg": null,
            "makingAmount": "12.5",
            "takingAmount": "8.25",
            "orderID": "0xorder",
            "status": "MATCHED",
            "success": true,
            "transactionsHashes": [
                "0x0000000000000000000000000000000000000000000000000000000000000123"
            ],
            "tradeIds": ["trade-1"]
        }))
        .expect("valid post-order response");

        let payload = live_order_response_json(&response);

        assert_eq!(payload["order_id"], "0xorder");
        assert_eq!(payload["success"], true);
        assert_eq!(payload["status"], "MATCHED");
        assert_eq!(payload["making_amount"], "12.5");
        assert_eq!(payload["taking_amount"], "8.25");
        assert_eq!(
            payload["transaction_hashes"][0],
            "0x0000000000000000000000000000000000000000000000000000000000000123"
        );
        assert_eq!(payload["trade_ids"][0], "trade-1");
    }

    #[test]
    fn parse_signature_type_rejects_unknown_values() {
        assert!(parse_signature_type("proxy").is_ok());
        assert!(parse_signature_type("gnosis-safe").is_ok());
        assert!(parse_signature_type("poly-1271").is_ok());
        assert!(parse_signature_type("eoa").is_ok());
        assert!(parse_signature_type("gnosis").is_err());
    }

    #[test]
    fn live_config_uses_account_funder_before_wallet() {
        let env_key = "POLYFOLLOW_PRIVATE_KEY_FUNDER_TEST";
        // SAFETY: Tests use a unique environment variable name and do not share it.
        unsafe {
            std::env::set_var(
                env_key,
                "0x0123456789012345678901234567890123456789012345678901234567890123",
            )
        };
        let account = AccountConfig {
            name: "funder_test".to_string(),
            wallet: Some("0x1111111111111111111111111111111111111111".to_string()),
            funder: Some("0x2222222222222222222222222222222222222222".to_string()),
            signature_type: "poly-1271".to_string(),
            ..Default::default()
        };

        let config = LiveExecutionConfig::from_env(&account, "https://clob.polymarket.com")
            .expect("valid live config");

        assert_eq!(
            config.funder,
            Some(Address::from_str("0x2222222222222222222222222222222222222222").unwrap())
        );
        // SAFETY: Cleanup for the unique test environment variable.
        unsafe { std::env::remove_var(env_key) };
    }
}
