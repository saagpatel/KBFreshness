use crate::config::AppState;
use crate::db::scan_runs;
use crate::error::AppError;
use crate::jobs::freshness_scan;
use axum::{
    extract::{Query, State},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_scans))
        .route("/trigger", post(trigger_scan))
}

#[derive(Deserialize)]
struct ListQuery {
    #[serde(default = "default_limit")]
    limit: i64,
}

fn default_limit() -> i64 {
    10
}

async fn list_scans(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> Result<impl IntoResponse, AppError> {
    // Validate limit
    if query.limit < 1 || query.limit > 100 {
        return Err(AppError::BadRequest(
            "limit must be between 1 and 100".into(),
        ));
    }

    let scans = scan_runs::list_recent(&state.db, query.limit).await?;
    Ok(Json(scans))
}

async fn trigger_scan(State(state): State<AppState>) -> Result<Json<serde_json::Value>, AppError> {
    // Atomically reserve a scan slot before spawning work.
    let scan_id = scan_runs::try_create_run(&state.db, "full")
        .await?
        .ok_or_else(|| AppError::Conflict("A scan is already in progress".into()))?;

    // Spawn scan task in background
    let pool = state.db.clone();
    let config = state.config.clone();

    tokio::spawn(async move {
        if let Err(e) = freshness_scan::run_full_scan_with_run_id(&pool, &config, scan_id).await {
            tracing::error!("Background scan failed: {}", e);
        }
    });

    Ok(Json(serde_json::json!({
        "message": "Scan triggered successfully",
        "status": "running"
    })))
}
