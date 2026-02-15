use crate::error::AppError;
use sqlx::PgPool;

/// Delete screenshots older than 30 days
pub async fn cleanup_old_screenshots(pool: &PgPool) -> Result<u64, AppError> {
    tracing::info!("Starting screenshot cleanup (>30 days old)");

    let result = sqlx::query(
        "DELETE FROM screenshots WHERE captured_at < NOW() - INTERVAL '30 days'"
    )
    .execute(pool)
    .await?;

    let deleted = result.rows_affected();
    tracing::info!("Deleted {} old screenshots", deleted);

    Ok(deleted)
}
