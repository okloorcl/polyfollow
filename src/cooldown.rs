use anyhow::Result;
use serde::Serialize;

use crate::config::AppConfig;
use crate::storage::Storage;

#[derive(Debug, Serialize)]
pub struct CooldownReport {
    pub threshold: usize,
    pub apply: bool,
    pub candidates: Vec<CooldownCandidate>,
}

#[derive(Debug, Serialize)]
pub struct CooldownCandidate {
    pub leader_address: String,
    pub blocked_intents: usize,
    pub disabled: bool,
}

pub fn audit_and_apply(
    cfg: &mut AppConfig,
    storage: &Storage,
    threshold: usize,
    apply: bool,
) -> Result<CooldownReport> {
    let mut candidates = Vec::new();
    for row in storage.blocked_counts_by_leader()? {
        if row.blocked_intents < threshold {
            continue;
        }
        let mut disabled = false;
        if apply
            && let Some(leader) = cfg
                .leaders
                .iter_mut()
                .find(|leader| leader.address.eq_ignore_ascii_case(&row.leader_address))
        {
            leader.enabled = false;
            disabled = true;
        }
        candidates.push(CooldownCandidate {
            leader_address: row.leader_address,
            blocked_intents: row.blocked_intents,
            disabled,
        });
    }
    Ok(CooldownReport {
        threshold,
        apply,
        candidates,
    })
}
