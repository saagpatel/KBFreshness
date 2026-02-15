use crate::error::AppError;
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Serialize, sqlx::FromRow)]
pub struct Screenshot {
    pub id: Uuid,
    pub article_id: Uuid,
    pub perceptual_hash: Option<String>,
    pub captured_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct ScreenshotWithDrift {
    pub id: Uuid,
    pub article_id: Uuid,
    pub perceptual_hash: Option<String>,
    pub captured_at: DateTime<Utc>,
    pub drift_distance: Option<u32>,
    pub needs_update: bool,
}

/// Get screenshots for an article with drift analysis
pub async fn get_timeline_with_drift(
    pool: &PgPool,
    article_id: Uuid,
    drift_threshold: u32,
) -> Result<Vec<ScreenshotWithDrift>, AppError> {
    let screenshots = sqlx::query_as::<_, Screenshot>(
        "SELECT id, article_id, perceptual_hash, captured_at
         FROM screenshots
         WHERE article_id = $1
         ORDER BY captured_at DESC
         LIMIT 10"
    )
    .bind(article_id)
    .fetch_all(pool)
    .await?;

    if screenshots.is_empty() {
        return Ok(vec![]);
    }

    let mut result = Vec::new();

    for (i, screenshot) in screenshots.iter().enumerate() {
        let drift_distance = if i < screenshots.len() - 1 {
            // Compare with previous screenshot
            let current_hash = screenshot.perceptual_hash.as_ref();
            let prev_hash = screenshots[i + 1].perceptual_hash.as_ref();

            match (current_hash, prev_hash) {
                (Some(_h1), Some(_h2)) => {
                    // Compile-time feature check: use real hash comparison if screenshots feature enabled,
                    // otherwise return 0 (no drift) as a placeholder
                    #[cfg(feature = "screenshots")]
                    {
                        crate::jobs::screenshot_capture::ScreenshotJob::compare_hashes(h1, h2).ok()
                    }
                    #[cfg(not(feature = "screenshots"))]
                    {
                        Some(0)
                    }
                }
                _ => None,
            }
        } else {
            None
        };

        let needs_update = drift_distance
            .map(|d| d > drift_threshold)
            .unwrap_or(false);

        result.push(ScreenshotWithDrift {
            id: screenshot.id,
            article_id: screenshot.article_id,
            perceptual_hash: screenshot.perceptual_hash.clone(),
            captured_at: screenshot.captured_at,
            drift_distance,
            needs_update,
        });
    }

    Ok(result)
}

/// Get screenshot image data by ID
pub async fn get_image_data(pool: &PgPool, screenshot_id: Uuid) -> Result<Vec<u8>, AppError> {
    let row: Option<(Vec<u8>,)> = sqlx::query_as(
        "SELECT image_data FROM screenshots WHERE id = $1"
    )
    .bind(screenshot_id)
    .fetch_optional(pool)
    .await?;

    match row {
        Some((data,)) => Ok(data),
        None => Err(AppError::NotFound(format!("screenshot with id {}", screenshot_id))),
    }
}
