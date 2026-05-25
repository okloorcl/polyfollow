use std::path::PathBuf;

use anyhow::Result;
use rust_decimal::Decimal;

use crate::cli::{LeaderAddArgs, LeaderCommand, LeaderUpdateArgs, PolyAlphaImportArgs};
use crate::config::{
    self, AppConfig, CopyConfig, CopyMode, LeaderConfig, LeaderRiskConfig, MarketFilters,
};
use crate::polyalpha::{PolyAlphaCandidate, load_candidates};
use crate::validate::normalize_address;

use super::responses::PolyAlphaImportResponse;
use super::support::{parse_decimal, print_response};

pub(super) fn handle_leader(json: bool, path: PathBuf, command: LeaderCommand) -> Result<()> {
    let mut cfg = config::load_or_default(&path)?;
    match command {
        LeaderCommand::Add(args) => {
            let leader = leader_from_add(args)?;
            cfg.add_leader(leader)?;
            config::save(&path, &cfg)?;
            print_leaders(json, &cfg)
        }
        LeaderCommand::List => print_leaders(json, &cfg),
        LeaderCommand::Remove(args) => {
            let removed = cfg.remove_leader(&args.address)?;
            config::save(&path, &cfg)?;
            print_response(json, &removed, || {
                println!("Removed leader {}", removed.address);
            })
        }
        LeaderCommand::Update(args) => {
            let leader = cfg.leader_mut(&args.address)?;
            apply_leader_update(leader, args)?;
            let leader = leader.clone();
            config::save(&path, &cfg)?;
            print_response(json, &leader, || {
                println!("Updated leader {}", leader.address);
            })
        }
        LeaderCommand::ImportPolyalpha(args) => import_polyalpha(json, path, &mut cfg, args),
    }
}

fn import_polyalpha(
    json: bool,
    path: PathBuf,
    cfg: &mut AppConfig,
    args: PolyAlphaImportArgs,
) -> Result<()> {
    let min_score = parse_decimal(&args.min_score, "min-score")?;
    let copy_ratio = parse_decimal(&args.copy_ratio, "copy-ratio")?;
    let max_order = parse_decimal(&args.max_order, "max-order")?;
    let max_daily = parse_decimal(&args.max_daily, "max-daily")?;
    let candidates = load_candidates(&args.input, min_score, &args.verdict)?;
    let (new_candidates, skipped_existing) = split_new_candidates(cfg, &candidates);
    let mut imported = Vec::new();

    if !args.dry_run {
        for candidate in &new_candidates {
            let leader = leader_from_candidate(candidate, copy_ratio, max_order, max_daily);
            cfg.add_leader(leader)?;
            imported.push(candidate.clone());
        }
        config::save(&path, cfg)?;
    }

    let response = PolyAlphaImportResponse {
        dry_run: args.dry_run,
        candidates,
        imported,
        skipped_existing,
    };
    print_response(json, &response, || {
        println!("PolyAlpha candidates: {}", response.candidates.len());
        if response.dry_run {
            println!("Dry run: config was not changed.");
        } else {
            println!("Imported leaders: {}", response.imported.len());
            println!("Skipped existing: {}", response.skipped_existing);
        }
    })
}

fn split_new_candidates(
    cfg: &AppConfig,
    candidates: &[PolyAlphaCandidate],
) -> (Vec<PolyAlphaCandidate>, usize) {
    let mut new_candidates = Vec::new();
    let mut skipped_existing = 0usize;
    for candidate in candidates {
        if cfg
            .leaders
            .iter()
            .any(|leader| leader.address.eq_ignore_ascii_case(&candidate.address))
        {
            skipped_existing += 1;
        } else {
            new_candidates.push(candidate.clone());
        }
    }
    (new_candidates, skipped_existing)
}

fn leader_from_candidate(
    candidate: &PolyAlphaCandidate,
    copy_ratio: Decimal,
    max_order: Decimal,
    max_daily: Decimal,
) -> LeaderConfig {
    LeaderConfig {
        address: candidate.address.clone(),
        label: Some(candidate.label.clone()),
        account_name: None,
        enabled: true,
        copy: CopyConfig {
            mode: CopyMode::Ratio,
            ratio: copy_ratio,
            fixed_order_usdc: CopyConfig::default().fixed_order_usdc,
        },
        risk: LeaderRiskConfig {
            max_order_usdc: max_order,
            max_daily_usdc: max_daily,
            ..LeaderRiskConfig::default()
        },
        filters: Default::default(),
    }
}

fn leader_from_add(args: LeaderAddArgs) -> Result<LeaderConfig> {
    let mut copy = CopyConfig::default();
    if let Some(value) = args.fixed_order {
        copy.mode = CopyMode::Fixed;
        copy.fixed_order_usdc = parse_decimal(&value, "fixed-order")?;
    }
    if let Some(value) = args.copy_ratio {
        copy.mode = CopyMode::Ratio;
        copy.ratio = parse_decimal(&value, "copy-ratio")?;
    }

    let mut risk = LeaderRiskConfig::default();
    if let Some(value) = args.max_order {
        risk.max_order_usdc = parse_decimal(&value, "max-order")?;
    }
    if let Some(value) = args.max_daily {
        risk.max_daily_usdc = parse_decimal(&value, "max-daily")?;
    }
    if let Some(value) = args.max_position {
        risk.max_position_usdc = parse_decimal(&value, "max-position")?;
    }
    risk.support_buy = !args.no_buy;
    risk.support_sell = !args.no_sell;

    Ok(LeaderConfig {
        address: normalize_address(&args.address)?,
        label: args.label,
        account_name: args.account,
        enabled: true,
        copy,
        risk,
        filters: MarketFilters {
            allow: args.market_allow,
            block: args.market_block,
        },
    })
}

fn apply_leader_update(leader: &mut LeaderConfig, args: LeaderUpdateArgs) -> Result<()> {
    if let Some(label) = args.label {
        leader.label = Some(label);
    }
    if let Some(account) = args.account {
        leader.account_name = Some(account);
    }
    if let Some(enabled) = args.enabled {
        leader.enabled = enabled;
    }
    if let Some(value) = args.fixed_order {
        leader.copy.mode = CopyMode::Fixed;
        leader.copy.fixed_order_usdc = parse_decimal(&value, "fixed-order")?;
    }
    if let Some(value) = args.copy_ratio {
        leader.copy.mode = CopyMode::Ratio;
        leader.copy.ratio = parse_decimal(&value, "copy-ratio")?;
    }
    if let Some(value) = args.max_order {
        leader.risk.max_order_usdc = parse_decimal(&value, "max-order")?;
    }
    if let Some(value) = args.max_daily {
        leader.risk.max_daily_usdc = parse_decimal(&value, "max-daily")?;
    }
    if let Some(value) = args.max_position {
        leader.risk.max_position_usdc = parse_decimal(&value, "max-position")?;
    }
    if let Some(value) = args.support_buy {
        leader.risk.support_buy = value;
    }
    if let Some(value) = args.support_sell {
        leader.risk.support_sell = value;
    }
    Ok(())
}

fn print_leaders(json: bool, cfg: &AppConfig) -> Result<()> {
    print_response(json, &cfg.leaders, || {
        if cfg.leaders.is_empty() {
            println!("No leaders configured.");
            return;
        }
        for leader in &cfg.leaders {
            let label = leader.label.as_deref().unwrap_or("-");
            println!(
                "{} [{}] enabled={} mode={:?} max_order={} max_daily={} buy={} sell={}",
                leader.address,
                label,
                leader.enabled,
                leader.copy.mode,
                leader.risk.max_order_usdc,
                leader.risk.max_daily_usdc,
                leader.risk.support_buy,
                leader.risk.support_sell
            );
        }
    })
}

#[cfg(test)]
mod tests {
    use rust_decimal_macros::dec;

    use super::*;

    #[test]
    fn polyalpha_candidate_split_counts_existing_in_preview() {
        let existing = "0x2222222222222222222222222222222222222222";
        let fresh = "0x3333333333333333333333333333333333333333";
        let cfg = AppConfig {
            leaders: vec![LeaderConfig {
                address: existing.to_string(),
                label: None,
                account_name: None,
                enabled: true,
                copy: CopyConfig::default(),
                risk: LeaderRiskConfig::default(),
                filters: Default::default(),
            }],
            ..Default::default()
        };
        let candidates = vec![
            candidate(existing),
            candidate(&existing.to_ascii_uppercase()),
            candidate(fresh),
        ];

        let (new_candidates, skipped_existing) = split_new_candidates(&cfg, &candidates);

        assert_eq!(skipped_existing, 2);
        assert_eq!(new_candidates.len(), 1);
        assert_eq!(new_candidates[0].address, fresh);
    }

    fn candidate(address: &str) -> PolyAlphaCandidate {
        PolyAlphaCandidate {
            address: address.to_string(),
            label: "test".to_string(),
            score: dec!(0.9),
            verdict: "paper_only".to_string(),
            source: "test".to_string(),
        }
    }
}
