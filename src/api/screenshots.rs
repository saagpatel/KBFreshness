use crate::config::AppState;
use crate::db::screenshots;
use crate::error::AppError;
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/articles/:article_id/screenshots",
            get(get_screenshots),
        )
        .route(
            "/api/screenshots/:screenshot_id/image",
            get(get_screenshot_image),
        )
}

#[derive(Deserialize)]
struct ScreenshotsQuery {
    #[serde(default = "default_drift_threshold")]
    drift_threshold: u32,
}

fn default_drift_threshold() -> u32 {
    10 // Hamming distance threshold for "significant drift"
}

async fn get_screenshots(
    State(state): State<AppState>,
    Path(article_id): Path<Uuid>,
    Query(query): Query<ScreenshotsQuery>,
) -> Result<impl IntoResponse, AppError> {
    let screenshots =
        screenshots::get_timeline_with_drift(&state.db, article_id, query.drift_threshold).await?;

    Ok(Json(screenshots))
}

async fn get_screenshot_image(
    State(state): State<AppState>,
    Path(screenshot_id): Path<Uuid>,
) -> Result<Response, AppError> {
    let image_data = screenshots::get_image_data(&state.db, screenshot_id).await?;

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "image/png")
        .header(header::CACHE_CONTROL, "public, max-age=31536000")
        .body(Body::from(image_data))
        .map_err(|e| AppError::Internal(format!("Failed to build response: {}", e)))
}
