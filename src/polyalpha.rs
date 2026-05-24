use std::collections::HashSet;
use std::path::Path;
use std::str::FromStr;

use anyhow::{Context, Result};
use rusqlite::Connection;
use rust_decimal::Decimal;
use serde::Serialize;
use serde_json::Value;

use crate::validate::normalize_address;

#[derive(Debug, Clone, Serialize)]
pub struct PolyAlphaCandidate {
    pub address: String,
    pub label: String,
    pub score: Decimal,
    pub verdict: String,
    pub source: String,
}

pub fn load_candidates(
    path: &Path,
    min_score: Decimal,
    verdicts: &[String],
) -> Result<Vec<PolyAlphaCandidate>> {
    let accepted = accepted_verdicts(verdicts);
    let candidates = if is_sqlite_path(path) {
        load_sqlite_candidates(path)?
    } else {
        load_json_candidates(path)?
    };
    Ok(candidates
        .into_iter()
        .filter(|candidate| candidate.score >= min_score)
        .filter(|candidate| accepted.contains(&candidate.verdict.to_ascii_lowercase()))
        .collect())
}

fn load_sqlite_candidates(path: &Path) -> Result<Vec<PolyAlphaCandidate>> {
    let conn =
        Connection::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let mut stmt = conn
        .prepare(
            r#"
            SELECT account, score, overall_verdict
            FROM wallet_follow_scores
            ORDER BY CAST(score AS REAL) DESC
            "#,
        )
        .context("failed to read wallet_follow_scores; is this a PolyAlpha SQLite database?")?;
    let rows = stmt.query_map([], |row| {
        let account: String = row.get(0)?;
        let score: String = row.get(1)?;
        let verdict: String = row.get(2)?;
        Ok((account, score, verdict))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (account, score, verdict) = row?;
        if let Some(candidate) = candidate_from_parts(&account, &score, &verdict, "sqlite") {
            out.push(candidate);
        }
    }
    Ok(out)
}

fn load_json_candidates(path: &Path) -> Result<Vec<PolyAlphaCandidate>> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let value = serde_json::from_str::<Value>(&text)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    let mut out = Vec::new();
    collect_json_candidates(&value, &mut out);
    Ok(out)
}

fn collect_json_candidates(value: &Value, out: &mut Vec<PolyAlphaCandidate>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_json_candidates(item, out);
            }
        }
        Value::Object(object) => {
            if let Some(candidate) = candidate_from_json(value) {
                out.push(candidate);
            }
            for key in [
                "leaders",
                "matched_accounts",
                "accounts",
                "wallets",
                "candidates",
            ] {
                if let Some(child) = object.get(key) {
                    collect_json_candidates(child, out);
                }
            }
        }
        _ => {}
    }
}

fn candidate_from_json(value: &Value) -> Option<PolyAlphaCandidate> {
    let address = first_string(value, &["account", "address", "wallet"])?;
    let score = first_decimal(value, &["score", "follow_score", "alpha_score"])?;
    let verdict = first_string(
        value,
        &["overall_verdict", "verdict", "worth_following", "tier"],
    )
    .unwrap_or_else(|| "unknown".to_string());
    candidate_from_parts(&address, &score.to_string(), &verdict, "json")
}

fn candidate_from_parts(
    address: &str,
    score: &str,
    verdict: &str,
    source: &str,
) -> Option<PolyAlphaCandidate> {
    let address = normalize_address(address).ok()?;
    let score = Decimal::from_str(score).ok()?;
    let verdict = verdict.to_ascii_lowercase();
    Some(PolyAlphaCandidate {
        label: format!("polyalpha:{verdict}:{score}"),
        address,
        score,
        verdict,
        source: source.to_string(),
    })
}

fn first_string(value: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        let found = value.pointer(&format!("/{key}")).and_then(value_to_string);
        if found.is_some() {
            return found;
        }
    }
    None
}

fn first_decimal(value: &Value, keys: &[&str]) -> Option<Decimal> {
    for key in keys {
        let found = value
            .pointer(&format!("/{key}"))
            .and_then(value_to_string)
            .and_then(|text| Decimal::from_str(&text).ok());
        if found.is_some() {
            return found;
        }
    }
    None
}

fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    }
}

fn accepted_verdicts(values: &[String]) -> HashSet<String> {
    if values.is_empty() {
        return [
            "follow",
            "paper",
            "paper_only",
            "live",
            "worth_following",
            "true",
            "strong_follow",
        ]
        .into_iter()
        .map(str::to_string)
        .collect();
    }
    values
        .iter()
        .map(|value| value.to_ascii_lowercase())
        .collect()
}

fn is_sqlite_path(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("sqlite" | "sqlite3" | "db")
    )
}

#[cfg(test)]
mod tests {
    use rust_decimal_macros::dec;

    use super::*;

    #[test]
    fn loads_candidates_from_json_export() {
        let path = std::env::temp_dir().join(format!(
            "polyalpha-candidates-{}.json",
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        std::fs::write(
            &path,
            r#"[{"account":"0x2222222222222222222222222222222222222222","score":"0.91","overall_verdict":"paper_only"}]"#,
        )
        .unwrap();
        let candidates = load_candidates(&path, dec!(0.8), &[]).unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].verdict, "paper_only");
        let _ = std::fs::remove_file(path);
    }
}
