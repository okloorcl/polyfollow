use chrono::Utc;

use crate::types::{CopyIntent, IntentVerdict, PaperFill};

pub fn paper_fill_for(intent: &CopyIntent) -> Option<PaperFill> {
    if intent.verdict != IntentVerdict::Paper {
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
