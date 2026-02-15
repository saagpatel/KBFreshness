use crate::error::AppError;
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use sqlx::{FromRow, Row};
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct TicketPattern {
    pub id: Uuid,
    pub ticket_category: String,
    pub ticket_count: i32,
    pub related_article_id: Option<Uuid>,
    pub keywords: serde_json::Value,
    pub suggested_update: String,
    pub detected_at: DateTime<Utc>,
}

impl<'r> FromRow<'r, sqlx::postgres::PgRow> for TicketPattern {
    fn from_row(row: &'r sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            ticket_category: row.try_get("ticket_category")?,
            ticket_count: row.try_get("ticket_count")?,
            related_article_id: row.try_get("related_article_id")?,
            keywords: row.try_get("keywords")?,
            suggested_update: row.try_get("suggested_update")?,
            detected_at: row.try_get("detected_at")?,
        })
    }
}

/// Insert a ticket pattern
pub async fn insert_pattern(
    pool: &PgPool,
    article_id: Option<Uuid>,
    ticket_count: i32,
    keywords: Vec<String>,
    suggested_update: String,
) -> Result<Uuid, AppError> {
    let keywords_json = serde_json::json!(keywords);

    // Generate category from top keyword (or "General" if empty)
    let category = keywords
        .first()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "General".to_string());

    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO ticket_patterns (ticket_category, ticket_count, related_article_id, keywords, suggested_update)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING id"
    )
    .bind(category)
    .bind(ticket_count)
    .bind(article_id)
    .bind(keywords_json)
    .bind(suggested_update)
    .fetch_one(pool)
    .await?;

    Ok(row.0)
}

/// Get ticket patterns for an article
pub async fn get_for_article(
    pool: &PgPool,
    article_id: Uuid,
) -> Result<Vec<TicketPattern>, AppError> {
    let patterns = sqlx::query_as::<TicketPattern>(
        "SELECT id, ticket_category, ticket_count, related_article_id, keywords, suggested_update, detected_at
         FROM ticket_patterns
         WHERE related_article_id = $1
         ORDER BY detected_at DESC
         LIMIT 5"
    )
    .bind(article_id)
    .fetch_all(pool)
    .await?;

    Ok(patterns)
}

/// Delete old ticket patterns (older than 90 days)
pub async fn cleanup_old_patterns(pool: &PgPool) -> Result<u64, AppError> {
    let result = sqlx::query(
        "DELETE FROM ticket_patterns
         WHERE detected_at < NOW() - INTERVAL '90 days'",
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

/// Clear existing patterns for an article (before inserting new ones)
pub async fn clear_for_article(pool: &PgPool, article_id: Uuid) -> Result<u64, AppError> {
    let result = sqlx::query("DELETE FROM ticket_patterns WHERE related_article_id = $1")
        .bind(article_id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected())
}
