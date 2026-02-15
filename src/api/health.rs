use crate::config::AppState;
use crate::error::AppError;
use axum::{extract::State, response::IntoResponse, routing::get, Json, Router};
use serde::Serialize;
use std::time::Instant;

static START_TIME: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

pub fn router() -> Router<AppState> {
    Router::new().route("/", get(health_check))
}

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    db_connected: bool,
    uptime_seconds: u64,
    version: String,
}

async fn health_check(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    // Check database connection pool health (avoids executing query on every request)
    let db_connected = !state.db.is_closed();

    // Calculate uptime
    let start_time = START_TIME.get_or_init(|| Instant::now());
    let uptime_seconds = start_time.elapsed().as_secs();

    let status = if db_connected { "ok" } else { "degraded" };

    Ok(Json(HealthResponse {
        status: status.to_string(),
        db_connected,
        uptime_seconds,
        version: env!("CARGO_PKG_VERSION").to_string(),
    }))
}
