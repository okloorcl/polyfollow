use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::config::AppConfig;
use crate::storage::Storage;

#[derive(Clone)]
struct ApiState {
    config: Arc<AppConfig>,
    db_path: PathBuf,
}

#[derive(Debug, Deserialize)]
struct LimitQuery {
    limit: Option<usize>,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
}

type ApiResult<T> = std::result::Result<Json<T>, (StatusCode, String)>;

pub async fn serve(config: AppConfig, db_path: PathBuf, addr: &str) -> Result<()> {
    let addr = addr
        .parse::<SocketAddr>()
        .with_context(|| format!("invalid bind address: {addr}"))?;
    let state = ApiState {
        config: Arc::new(config),
        db_path,
    };
    let app = Router::new()
        .route("/health", get(health))
        .route("/status", get(status))
        .route("/leaders", get(leaders))
        .route("/orders", get(orders))
        .route("/logs", get(logs))
        .route("/pnl", get(pnl))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("failed to bind {addr}"))?;
    tracing::info!(%addr, "polyfollow api listening");
    axum::serve(listener, app)
        .await
        .context("api server failed")
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

async fn status(State(state): State<ApiState>) -> ApiResult<serde_json::Value> {
    let mut storage = open_storage(&state)?;
    storage
        .sync_leaders(&state.config.leaders)
        .map_err(internal_error)?;
    let status = storage.status().map_err(internal_error)?;
    Ok(Json(serde_json::json!({
        "mode": state.config.global.mode,
        "kill_switch": state.config.global.kill_switch,
        "storage": status,
    })))
}

async fn leaders(State(state): State<ApiState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "leaders": state.config.leaders,
    }))
}

async fn orders(
    State(state): State<ApiState>,
    Query(query): Query<LimitQuery>,
) -> ApiResult<serde_json::Value> {
    let storage = open_storage(&state)?;
    let rows = storage
        .recent_intents(query.limit.unwrap_or(20))
        .map_err(internal_error)?;
    Ok(Json(serde_json::json!({ "orders": rows })))
}

async fn logs(
    State(state): State<ApiState>,
    Query(query): Query<LimitQuery>,
) -> ApiResult<serde_json::Value> {
    let storage = open_storage(&state)?;
    let rows = storage
        .recent_logs(query.limit.unwrap_or(20))
        .map_err(internal_error)?;
    Ok(Json(serde_json::json!({ "logs": rows })))
}

async fn pnl(State(state): State<ApiState>) -> ApiResult<serde_json::Value> {
    let storage = open_storage(&state)?;
    let pnl = storage.pnl_summary().map_err(internal_error)?;
    Ok(Json(serde_json::json!({ "pnl": pnl })))
}

fn open_storage(state: &ApiState) -> std::result::Result<Storage, (StatusCode, String)> {
    Storage::open(&state.db_path).map_err(internal_error)
}

fn internal_error(error: anyhow::Error) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
}
