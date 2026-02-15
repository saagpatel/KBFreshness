use crate::error::AppError;
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Serialize, sqlx::FromRow)]
pub struct ScanRun {
    pub id: Uuid,
    pub scan_type: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub articles_scanned: i32,
    pub links_checked: i32,
    pub broken_links_found: i32,
    pub status: String,
    pub error_message: Option<String>,
}

/// Try to create a running scan record atomically.
/// Returns None when another scan is already marked as running.
pub async fn try_create_run(pool: &PgPool, scan_type: &str) -> Result<Option<Uuid>, AppError> {
    let row: Option<(Uuid,)> = sqlx::query_as(
        "INSERT INTO scan_runs (scan_type, status)
         SELECT $1, 'running'
         WHERE NOT EXISTS (
             SELECT 1 FROM scan_runs WHERE status = 'running'
         )
         RETURNING id",
    )
    .bind(scan_type)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| r.0))
}

/// Mark scan run as completed with stats
pub async fn complete_run(
    pool: &PgPool,
    id: Uuid,
    articles_scanned: i32,
    links_checked: i32,
    broken_links_found: i32,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE scan_runs SET completed_at = NOW(), status = 'completed', articles_scanned = $1, links_checked = $2, broken_links_found = $3 WHERE id = $4"
    )
    .bind(articles_scanned)
    .bind(links_checked)
    .bind(broken_links_found)
    .bind(id)
    .execute(pool)
    .await?;

    Ok(())
}

/// Mark scan run as failed with error message
pub async fn fail_run(pool: &PgPool, id: Uuid, error_message: &str) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE scan_runs SET completed_at = NOW(), status = 'failed', error_message = $1 WHERE id = $2"
    )
    .bind(error_message)
    .bind(id)
    .execute(pool)
    .await?;

    Ok(())
}

/// List recent scan runs
pub async fn list_recent(pool: &PgPool, limit: i64) -> Result<Vec<ScanRun>, AppError> {
    let runs = sqlx::query_as::<_, ScanRun>(
        "SELECT id, scan_type, started_at, completed_at, articles_scanned, links_checked, broken_links_found, status, error_message FROM scan_runs ORDER BY started_at DESC LIMIT $1"
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(runs)
}
