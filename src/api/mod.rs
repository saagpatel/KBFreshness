pub mod articles;
pub mod health;
pub mod scans;
pub mod screenshots;

use crate::config::AppState;
use axum::{
    http::{HeaderValue, Method},
    Router,
};
use tower_http::cors::CorsLayer;

pub fn create_router(state: AppState) -> Router {
    let cors_origin = std::env::var("CORS_ALLOWED_ORIGIN")
        .unwrap_or_else(|_| "http://localhost:5173".to_string());

    let cors = CorsLayer::new()
        .allow_origin(
            cors_origin
                .parse::<HeaderValue>()
                .unwrap_or_else(|_| {
                    tracing::warn!("Invalid CORS_ALLOWED_ORIGIN, falling back to default");
                    "http://localhost:5173".parse().unwrap()
                })
        )
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
        .allow_headers(tower_http::cors::Any);

    Router::new()
        .nest("/api/articles", articles::router())
        .merge(screenshots::router())
        .nest("/api/scans", scans::router())
        .nest("/health", health::router())
        .layer(cors)
        .with_state(state)
}
