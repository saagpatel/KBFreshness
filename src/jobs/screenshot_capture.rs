use crate::error::AppError;
#[cfg(feature = "screenshots")]
use crate::security::validate_outbound_url;
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

#[cfg(feature = "screenshots")]
use chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotFormat;
#[cfg(feature = "screenshots")]
use chromiumoxide::{
    browser::{Browser, BrowserConfig},
    page::ScreenshotParams,
};
#[cfg(feature = "screenshots")]
use futures::stream::StreamExt;
#[cfg(feature = "screenshots")]
use image::{imageops::FilterType, DynamicImage};
#[cfg(feature = "screenshots")]
use std::sync::Arc;
#[cfg(feature = "screenshots")]
use tokio::sync::Mutex;

pub struct ScreenshotJob {
    #[cfg(feature = "screenshots")]
    browser: Arc<Mutex<Browser>>,
}

impl ScreenshotJob {
    #[cfg(feature = "screenshots")]
    pub async fn new() -> Result<Self, AppError> {
        let (browser, mut handler) = Browser::launch(
            BrowserConfig::builder()
                .disable_default_args()
                .args(vec![
                    "--headless",
                    "--disable-gpu",
                    "--no-sandbox",
                    "--disable-dev-shm-usage",
                ])
                .build()
                .map_err(|e| AppError::Browser(format!("Failed to configure browser: {}", e)))?,
        )
        .await
        .map_err(|e| AppError::Browser(format!("Failed to launch browser: {}", e)))?;

        // Spawn handler task to process events
        tokio::spawn(async move {
            while let Some(event) = handler.next().await {
                if let Err(e) = event {
                    tracing::error!("Browser handler error: {}", e);
                }
            }
        });

        Ok(Self {
            browser: Arc::new(Mutex::new(browser)),
        })
    }

    #[cfg(not(feature = "screenshots"))]
    pub async fn new() -> Result<Self, AppError> {
        Ok(Self {})
    }

    /// Capture screenshot of a URL
    #[cfg(feature = "screenshots")]
    pub async fn capture_screenshot(&self, url: &str) -> Result<Vec<u8>, AppError> {
        if let Err(reason) = validate_outbound_url(url) {
            return Err(AppError::BadRequest(format!(
                "Blocked unsafe screenshot URL: {}",
                reason
            )));
        }

        tracing::debug!("Capturing screenshot of: {}", url);

        let browser = self.browser.lock().await;
        let page = browser
            .new_page(url)
            .await
            .map_err(|e| AppError::Browser(format!("Failed to create page: {}", e)))?;

        // Wait for page to load - configurable via SCREENSHOT_WAIT_SECS env var (default: 3)
        let wait_secs = std::env::var("SCREENSHOT_WAIT_SECS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(3);
        tokio::time::sleep(tokio::time::Duration::from_secs(wait_secs)).await;

        let screenshot_bytes = page
            .screenshot(
                ScreenshotParams::builder()
                    .format(CaptureScreenshotFormat::Png)
                    .full_page(true)
                    .build(),
            )
            .await
            .map_err(|e| AppError::Browser(format!("Failed to capture screenshot: {}", e)))?;

        tracing::debug!("Screenshot captured: {} bytes", screenshot_bytes.len());
        Ok(screenshot_bytes)
    }

    #[cfg(not(feature = "screenshots"))]
    pub async fn capture_screenshot(&self, _url: &str) -> Result<Vec<u8>, AppError> {
        Err(AppError::Internal(
            "Screenshot feature not enabled. Build with --features screenshots".into(),
        ))
    }

    /// Calculate perceptual hash of image bytes
    #[cfg(feature = "screenshots")]
    pub fn calculate_hash(image_bytes: &[u8]) -> Result<String, AppError> {
        let img = image::load_from_memory(image_bytes)
            .map_err(|e| AppError::Internal(format!("Failed to decode image: {}", e)))?;

        Ok(average_hash_8x8(&img))
    }

    #[cfg(not(feature = "screenshots"))]
    pub fn calculate_hash(_image_bytes: &[u8]) -> Result<String, AppError> {
        Ok("no-hash-feature-disabled".to_string())
    }

    /// Compare two perceptual hashes and return similarity distance (0-64, lower = more similar)
    #[cfg(feature = "screenshots")]
    pub fn compare_hashes(hash1: &str, hash2: &str) -> Result<u32, AppError> {
        let h1 = u64::from_str_radix(hash1, 16)
            .map_err(|e| AppError::Internal(format!("Failed to parse hash1: {}", e)))?;
        let h2 = u64::from_str_radix(hash2, 16)
            .map_err(|e| AppError::Internal(format!("Failed to parse hash2: {}", e)))?;

        Ok((h1 ^ h2).count_ones())
    }

    #[cfg(not(feature = "screenshots"))]
    pub fn compare_hashes(_hash1: &str, _hash2: &str) -> Result<u32, AppError> {
        Ok(0)
    }
}

/// Store screenshot in database
pub async fn store_screenshot(
    pool: &PgPool,
    article_id: Uuid,
    image_data: Vec<u8>,
    perceptual_hash: String,
) -> Result<Uuid, AppError> {
    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO screenshots (article_id, image_data, perceptual_hash, captured_at)
         VALUES ($1, $2, $3, NOW())
         RETURNING id",
    )
    .bind(article_id)
    .bind(image_data)
    .bind(perceptual_hash)
    .fetch_one(pool)
    .await?;

    Ok(row.0)
}

/// Get the most recent screenshot for an article
pub async fn get_latest_screenshot(
    pool: &PgPool,
    article_id: Uuid,
) -> Result<Option<Screenshot>, AppError> {
    let screenshot = sqlx::query_as::<_, Screenshot>(
        "SELECT id, article_id, perceptual_hash, captured_at
         FROM screenshots
         WHERE article_id = $1
         ORDER BY captured_at DESC
         LIMIT 1",
    )
    .bind(article_id)
    .fetch_optional(pool)
    .await?;

    Ok(screenshot)
}

/// Get screenshot timeline for an article
pub async fn get_screenshot_timeline(
    pool: &PgPool,
    article_id: Uuid,
) -> Result<Vec<Screenshot>, AppError> {
    let screenshots = sqlx::query_as::<_, Screenshot>(
        "SELECT id, article_id, perceptual_hash, captured_at
         FROM screenshots
         WHERE article_id = $1
         ORDER BY captured_at DESC
         LIMIT 10",
    )
    .bind(article_id)
    .fetch_all(pool)
    .await?;

    Ok(screenshots)
}

/// Get screenshot image data
pub async fn get_screenshot_image(
    pool: &PgPool,
    screenshot_id: Uuid,
) -> Result<Option<Vec<u8>>, AppError> {
    let row: Option<(Vec<u8>,)> =
        sqlx::query_as("SELECT image_data FROM screenshots WHERE id = $1")
            .bind(screenshot_id)
            .fetch_optional(pool)
            .await?;

    Ok(row.map(|r| r.0))
}

#[derive(sqlx::FromRow, serde::Serialize)]
pub struct Screenshot {
    pub id: Uuid,
    pub article_id: Uuid,
    pub perceptual_hash: Option<String>,
    pub captured_at: chrono::DateTime<Utc>,
}

#[cfg(feature = "screenshots")]
fn average_hash_8x8(img: &DynamicImage) -> String {
    let grayscale = img.resize_exact(8, 8, FilterType::Triangle).to_luma8();

    let mut sum: u32 = 0;
    for pixel in grayscale.pixels() {
        sum += pixel[0] as u32;
    }
    let avg = (sum / 64) as u8;

    let mut bits: u64 = 0;
    for (idx, pixel) in grayscale.pixels().enumerate() {
        if pixel[0] >= avg {
            bits |= 1u64 << idx;
        }
    }

    format!("{:016x}", bits)
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "screenshots")]
    use super::*;

    #[test]
    #[cfg(feature = "screenshots")]
    fn test_hash_identical_gives_zero_distance() {
        use image::{Rgba, RgbaImage};

        // Create a simple 1x1 black pixel image
        let img = RgbaImage::from_pixel(1, 1, Rgba([0, 0, 0, 255]));
        let dyn_img = image::DynamicImage::ImageRgba8(img);
        let hash1 = average_hash_8x8(&dyn_img);
        let hash2 = average_hash_8x8(&dyn_img);
        let distance = ScreenshotJob::compare_hashes(&hash1, &hash2).unwrap();

        // Identical hashes should have distance 0
        assert_eq!(distance, 0);
    }

    #[test]
    #[cfg(feature = "screenshots")]
    fn test_hash_distance_detects_image_difference() {
        use image::{GrayImage, Luma};

        let black = GrayImage::from_pixel(8, 8, Luma([0u8]));
        let mut almost_black = GrayImage::from_pixel(8, 8, Luma([0u8]));
        almost_black.put_pixel(0, 0, Luma([255u8]));

        let hash_black = average_hash_8x8(&DynamicImage::ImageLuma8(black));
        let hash_almost_black = average_hash_8x8(&DynamicImage::ImageLuma8(almost_black));

        let distance = ScreenshotJob::compare_hashes(&hash_black, &hash_almost_black).unwrap();
        assert!(distance > 0);
    }
}
