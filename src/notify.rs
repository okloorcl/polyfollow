use anyhow::{Context, Result};
use reqwest::Client;
use serde::Serialize;

use crate::config::NotificationConfig;
use crate::types::{CopyIntent, IntentVerdict, PaperExecutionResult};

#[derive(Clone)]
pub struct Notifier {
    config: NotificationConfig,
    client: Client,
}

#[derive(Debug, Serialize)]
struct IntentNotification<'a> {
    kind: &'static str,
    intent: &'a CopyIntent,
    paper_result: Option<&'a PaperExecutionResult>,
}

impl Notifier {
    pub fn new(config: &NotificationConfig) -> Self {
        Self {
            config: config.clone(),
            client: Client::new(),
        }
    }

    pub async fn notify_intent(
        &self,
        intent: &CopyIntent,
        paper_result: Option<&PaperExecutionResult>,
    ) {
        if !self.should_notify(intent) {
            return;
        }
        let payload = IntentNotification {
            kind: "copy_intent",
            intent,
            paper_result,
        };
        if let Err(error) = self.send_webhook(&payload).await {
            tracing::warn!(error = %error, "webhook notification failed");
        }
        if let Err(error) = self.send_telegram(intent, paper_result).await {
            tracing::warn!(error = %error, "telegram notification failed");
        }
    }

    fn should_notify(&self, intent: &CopyIntent) -> bool {
        let has_target = self.config.webhook_url.is_some()
            || (self.config.telegram_bot_token.is_some() && self.config.telegram_chat_id.is_some());
        has_target && (self.config.notify_blocked || intent.verdict != IntentVerdict::Blocked)
    }

    async fn send_webhook<T: Serialize>(&self, payload: &T) -> Result<()> {
        let Some(url) = self.config.webhook_url.as_deref() else {
            return Ok(());
        };
        self.client
            .post(url)
            .json(payload)
            .send()
            .await
            .context("failed to send webhook")?
            .error_for_status()
            .context("webhook returned error status")?;
        Ok(())
    }

    async fn send_telegram(
        &self,
        intent: &CopyIntent,
        paper_result: Option<&PaperExecutionResult>,
    ) -> Result<()> {
        let (Some(token), Some(chat_id)) = (
            self.config.telegram_bot_token.as_deref(),
            self.config.telegram_chat_id.as_deref(),
        ) else {
            return Ok(());
        };
        let url = format!("https://api.telegram.org/bot{token}/sendMessage");
        self.client
            .post(url)
            .json(&serde_json::json!({
                "chat_id": chat_id,
                "text": telegram_text(intent, paper_result),
                "disable_web_page_preview": true
            }))
            .send()
            .await
            .context("failed to send telegram message")?
            .error_for_status()
            .context("telegram returned error status")?;
        Ok(())
    }
}

fn telegram_text(intent: &CopyIntent, paper_result: Option<&PaperExecutionResult>) -> String {
    let mut lines = vec![
        format!(
            "PolyFollow {}",
            format!("{:?}", intent.verdict).to_ascii_lowercase()
        ),
        format!("leader: {}", intent.leader_address),
        format!("side: {}", intent.side.as_str()),
        format!("notional: {} USDC", intent.notional_usdc),
    ];
    if let Some(shares) = intent.shares {
        lines.push(format!("shares: {shares}"));
    }
    if let Some(token_id) = intent.token_id.as_deref() {
        lines.push(format!("token: {token_id}"));
    }
    if !intent.reasons.is_empty() {
        lines.push(format!("reasons: {}", intent.reasons.join("; ")));
    }
    if let Some(result) = paper_result {
        if result.closed_lots > 0 {
            lines.push(format!("closed lots: {}", result.closed_lots));
            lines.push(format!("realized pnl: {} USDC", result.realized_pnl_usdc));
        }
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use rust_decimal_macros::dec;

    use super::*;
    use crate::types::TradeSide;

    #[test]
    fn telegram_text_includes_realized_pnl() {
        let intent = CopyIntent {
            intent_id: "intent-1".to_string(),
            leader_address: "0x2222222222222222222222222222222222222222".to_string(),
            trade_id: "trade-1".to_string(),
            mode: "paper".to_string(),
            side: TradeSide::Sell,
            market_id: None,
            token_id: Some("123".to_string()),
            target_price: Some(dec!(0.5)),
            notional_usdc: dec!(10),
            shares: Some(dec!(20)),
            verdict: IntentVerdict::Paper,
            reasons: Vec::new(),
            created_at: Utc::now(),
        };
        let result = PaperExecutionResult {
            opened_fills: 0,
            closed_lots: 1,
            realized_pnl_usdc: dec!(2.5),
        };
        let text = telegram_text(&intent, Some(&result));
        assert!(text.contains("realized pnl: 2.5 USDC"));
    }
}
