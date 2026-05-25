use std::path::PathBuf;

use serde::Serialize;

use crate::config::ExecutionMode;
use crate::polyalpha::PolyAlphaCandidate;

#[derive(Debug, Serialize)]
pub(super) struct SetupResponse {
    pub config_path: PathBuf,
    pub db_path: PathBuf,
    pub mode: ExecutionMode,
}

#[derive(Debug, Serialize)]
pub(super) struct ConfigPathResponse {
    pub config_path: PathBuf,
    pub db_path: PathBuf,
}

#[derive(Debug, Serialize)]
pub(super) struct StatusResponse {
    pub mode: ExecutionMode,
    pub kill_switch: bool,
    pub configured_leaders: usize,
    pub enabled_leaders: usize,
    pub storage: crate::storage::StorageStatus,
}

#[derive(Debug, Serialize)]
pub(super) struct DoctorResponse {
    pub ok: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(super) struct PolyAlphaImportResponse {
    pub dry_run: bool,
    pub candidates: Vec<PolyAlphaCandidate>,
    pub imported: Vec<PolyAlphaCandidate>,
    pub skipped_existing: usize,
}

#[derive(Debug, Serialize)]
pub(super) struct RunResponse {
    pub mode: ExecutionMode,
    pub once: bool,
    pub enabled_leaders: usize,
    pub cycles: usize,
    pub failed_cycles: usize,
    pub fetched_trades: usize,
    pub new_trades: usize,
    pub blocked_intents: usize,
    pub paper_fills: usize,
    pub message: String,
}

#[derive(Debug, Default)]
pub(super) struct RunStats {
    pub cycles: usize,
    pub failed_cycles: usize,
    pub fetched_trades: usize,
    pub new_trades: usize,
    pub blocked_intents: usize,
    pub paper_fills: usize,
}
