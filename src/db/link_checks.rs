use crate::error::AppError;
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use sqlx::{FromRow, Row};
use uuid::Uuid;

#[derive(Serialize)]
pub struct LinkCheck {
    pub id: Uuid,
    pub article_id: Uuid,
    pub url: String,
    pub status_code: Option<i32>,
    pub is_broken: bool,
    pub error_message: Option<String>,
    pub checked_at: DateTime<Utc>,
}

impl<'r> FromRow<'r, sqlx::postgres::PgRow> for LinkCheck {
    fn from_row(row: &'r sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            article_id: row.try_get("article_id")?,
            url: row.try_get("url")?,
            status_code: row.try_get("status_code")?,
            is_broken: row.try_get("is_broken")?,
            error_message: row.try_get("error_message")?,
            checked_at: row.try_get("checked_at")?,
        })
    }
}

/// Get all link checks for an article (most recent check per URL)
pub async fn get_for_article(pool: &PgPool, article_id: Uuid) -> Result<Vec<LinkCheck>, AppError> {
    let links = sqlx::query_as::<LinkCheck>(
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
pub async fn get_broken_for_article(
    pool: &PgPool,
    article_id: Uuid,
) -> Result<Vec<LinkCheck>, AppError> {
    let links = sqlx::query_as::<LinkCheck>(
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
