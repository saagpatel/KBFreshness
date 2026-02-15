use crate::config::AppState;
use crate::db::{articles, link_checks, ticket_patterns};
use crate::error::AppError;
use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_articles))
        .route("/stats", get(get_article_stats))
        .route("/:id", get(get_article))
        .route("/:id/links", get(get_article_links))
        .route("/:id/review", post(review_article))
        .route("/:id/flag", post(flag_article))
}

#[derive(Deserialize)]
struct ListQuery {
    #[serde(default)]
    health: Option<String>,
    #[serde(default)]
    space: Option<String>,
    #[serde(default = "ListQuery::default_sort")]
    sort: String,
    #[serde(default = "ListQuery::default_order")]
    order: String,
    #[serde(default = "ListQuery::default_page")]
    page: i64,
    #[serde(default = "ListQuery::default_limit")]
    limit: i64,
}

impl ListQuery {
    fn default_sort() -> String {
        "age".to_string()
    }

    fn default_order() -> String {
        "desc".to_string()
    }

    fn default_page() -> i64 {
        1
    }

    fn default_limit() -> i64 {
        50
    }
}

#[derive(Serialize)]
struct ArticleListResponse {
    articles: Vec<articles::ArticleWithHealth>,
    total: i64,
    page: i64,
    limit: i64,
}

async fn list_articles(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> Result<impl IntoResponse, AppError> {
    // Validate pagination
    if query.page < 1 {
        return Err(AppError::BadRequest("page must be >= 1".into()));
    }
    if query.limit < 1 || query.limit > 100 {
        return Err(AppError::BadRequest(
            "limit must be between 1 and 100".into(),
        ));
    }

    let (articles_list, total) = articles::list_articles_with_health(
        &state.db,
        query.health.as_deref(),
        query.space.as_deref(),
        &query.sort,
        &query.order,
        query.page,
        query.limit,
    )
    .await?;

    Ok(Json(ArticleListResponse {
        articles: articles_list,
        total,
        page: query.page,
        limit: query.limit,
    }))
}

#[derive(Serialize)]
struct ArticleDetailResponse {
    #[serde(flatten)]
    article: articles::ArticleWithHealth,
    ticket_patterns: Vec<ticket_patterns::TicketPattern>,
}

async fn get_article(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let article = articles::get_article_by_id(&state.db, id).await?;
    let patterns = ticket_patterns::get_for_article(&state.db, id).await?;

    Ok(Json(ArticleDetailResponse {
        article,
        ticket_patterns: patterns,
    }))
}

#[derive(Deserialize)]
struct ReviewRequest {
    reviewed_by: String,
}

async fn review_article(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<ReviewRequest>,
) -> Result<impl IntoResponse, AppError> {
    let reviewed_by = validate_reviewer_name(&payload.reviewed_by)?;
    articles::mark_reviewed(&state.db, id, &reviewed_by).await?;
    let article = articles::get_article_by_id(&state.db, id).await?;
    Ok(Json(article))
}

#[derive(Deserialize)]
struct FlagRequest {
    flagged: bool,
}

async fn flag_article(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<FlagRequest>,
) -> Result<impl IntoResponse, AppError> {
    articles::set_manual_flag(&state.db, id, payload.flagged).await?;
    let article = articles::get_article_by_id(&state.db, id).await?;
    Ok(Json(article))
}

async fn get_article_links(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let links = link_checks::get_for_article(&state.db, id).await?;
    Ok(Json(links))
}

#[derive(Serialize)]
struct ArticleStatsResponse {
    total: i64,
    green: i64,
    yellow: i64,
    red: i64,
}

async fn get_article_stats(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> Result<impl IntoResponse, AppError> {
    // Get all articles with health filter applied
    let (articles_list, total) = articles::list_articles_with_health(
        &state.db,
        query.health.as_deref(),
        query.space.as_deref(),
        "age",
        "desc",
        1,
        10000, // High limit to get all articles for stats
    )
    .await?;

    // Calculate stats
    let mut green = 0;
    let mut yellow = 0;
    let mut red = 0;

    for article in &articles_list {
        match article.health {
            crate::health::HealthStatus::Green => green += 1,
            crate::health::HealthStatus::Yellow => yellow += 1,
            crate::health::HealthStatus::Red => red += 1,
        }
    }

    Ok(Json(ArticleStatsResponse {
        total,
        green,
        yellow,
        red,
    }))
}

fn validate_reviewer_name(name: &str) -> Result<String, AppError> {
    let trimmed = name.trim();

    if trimmed.is_empty() {
        return Err(AppError::BadRequest("reviewed_by cannot be empty".into()));
    }

    if trimmed.len() > 100 {
        return Err(AppError::BadRequest(
            "reviewed_by must be 100 characters or less".into(),
        ));
    }

    let is_valid = trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == ' ' || c == '-' || c == '_' || c == '.');

    if !is_valid {
        return Err(AppError::BadRequest(
            "reviewed_by contains invalid characters".into(),
        ));
    }

    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::validate_reviewer_name;

    #[test]
    fn validate_reviewer_name_accepts_expected_values() {
        assert_eq!(
            validate_reviewer_name("  Jane-Doe_1  ").unwrap(),
            "Jane-Doe_1"
        );
        assert_eq!(validate_reviewer_name("A.B").unwrap(), "A.B");
    }

    #[test]
    fn validate_reviewer_name_rejects_invalid_values() {
        assert!(validate_reviewer_name("   ").is_err());
        assert!(validate_reviewer_name("name!").is_err());
        assert!(validate_reviewer_name(&"a".repeat(101)).is_err());
    }
}
