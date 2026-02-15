use crate::error::AppError;
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Serialize, sqlx::FromRow)]
pub struct LinkCheck {
    pub id: Uuid,
    pub article_id: Uuid,
    pub url: String,
    pub status_code: Option<i32>,
    pub is_broken: bool,
    pub error_message: Option<String>,
    pub checked_at: DateTime<Utc>,
}

/// Get all link checks for an article (most recent check per URL)
pub async fn get_for_article(pool: &PgPool, article_id: Uuid) -> Result<Vec<LinkCheck>, AppError> {
    let links = sqlx::query_as::<_, LinkCheck>(
        r#"
        SELECT DISTINCT ON (url) id, article_id, url, status_code, is_broken, error_message, checked_at
        FROM link_checks
        WHERE article_id = $1
        ORDER BY url, checked_at DESC
        "#
    )
    .bind(article_id)
    .fetch_all(pool)
    .await?;

    Ok(links)
}

/// Get only broken links for an article
pub async fn get_broken_for_article(pool: &PgPool, article_id: Uuid) -> Result<Vec<LinkCheck>, AppError> {
    let links = sqlx::query_as::<_, LinkCheck>(
        r#"
        SELECT DISTINCT ON (url) id, article_id, url, status_code, is_broken, error_message, checked_at
        FROM link_checks
        WHERE article_id = $1 AND is_broken = TRUE
        ORDER BY url, checked_at DESC
        "#
    )
    .bind(article_id)
    .fetch_all(pool)
    .await?;

    Ok(links)
}
