use crate::config::Config;
use crate::db::{articles, scan_runs};
use crate::error::AppError;
use crate::jobs::link_checker;
use crate::sources::confluence::ConfluenceClient;
use sqlx::PgPool;

#[cfg(feature = "screenshots")]
use crate::jobs::screenshot_capture;

pub struct ScanStats {
    pub articles_scanned: i32,
    pub links_checked: i32,
    pub broken_links_found: i32,
    pub screenshots_captured: i32,
}

/// Run a full freshness scan: sync articles from Confluence, then check links
pub async fn run_full_scan(pool: &PgPool, config: &Config) -> Result<ScanStats, AppError> {
    tracing::info!("Starting full freshness scan");

    // Create scan run record
    let scan_id = scan_runs::create_run(pool, "full").await?;

    match run_scan_inner(pool, config).await {
        Ok(stats) => {
            scan_runs::complete_run(
                pool,
                scan_id,
                stats.articles_scanned,
                stats.links_checked,
                stats.broken_links_found,
            )
            .await?;

            tracing::info!(
                "Scan completed: {} articles, {} links checked, {} broken",
                stats.articles_scanned,
                stats.links_checked,
                stats.broken_links_found
            );

            Ok(stats)
        }
        Err(e) => {
            let error_msg = format!("{}", e);
            scan_runs::fail_run(pool, scan_id, &error_msg).await?;
            Err(e)
        }
    }
}

async fn run_scan_inner(pool: &PgPool, config: &Config) -> Result<ScanStats, AppError> {
    let mut total_articles = 0;
    let mut total_links = 0;
    let mut total_broken = 0;
    let total_screenshots = 0;

    // Sync Confluence articles if configured
    if let (Some(base_url), Some(email), Some(token)) = (
        &config.confluence_base_url,
        &config.confluence_email,
        &config.confluence_api_token,
    ) {
        tracing::info!("Syncing articles from Confluence");

        let client = ConfluenceClient::new(
            base_url.clone(),
            email.clone(),
            token.clone(),
        );

        // Space key is required when Confluence is configured
        let space_key = match std::env::var("CONFLUENCE_SPACE_KEY") {
            Ok(key) => key,
            Err(_) => {
                return Err(AppError::Internal(
                    "CONFLUENCE_SPACE_KEY environment variable is required when Confluence credentials are configured".to_string()
                ));
            }
        };

        let pages = client.fetch_all_pages(&space_key).await?;

        tracing::info!("Fetched {} pages from Confluence", pages.len());

        // HTTP client for link checking
        let http_client = reqwest::Client::new();

        for page in pages {
            // Upsert article
            let article_id = articles::upsert_from_source(
                pool,
                articles::InsertArticle {
                    title: page.title.clone(),
                    url: page.url.clone(),
                    source: articles::SourceType::Confluence,
                    source_id: Some(page.id.clone()),
                    space_key: Some(page.space_key.clone()),
                    last_modified_at: page.last_modified_at,
                    last_modified_by: page.last_modified_by.clone(),
                    version_number: page.version_number,
                },
            )
            .await?;

            total_articles += 1;

            // Extract and check links
            let links = link_checker::extract_links(&page.body_storage_format, base_url);

            if !links.is_empty() {
                tracing::debug!("Checking {} links for article: {}", links.len(), page.title);

                let results = link_checker::check_links(links.clone(), &http_client).await;

                // Count broken links
                let broken_count = results.iter().filter(|r| r.is_broken).count();
                total_broken += broken_count as i32;
                total_links += results.len() as i32;

                // Store results
                link_checker::store_link_results(pool, article_id, results).await?;

                if broken_count > 0 {
                    tracing::warn!("Found {} broken links in article: {}", broken_count, page.title);
                }
            }
        }
    } else {
        tracing::warn!("Confluence credentials not configured, skipping article sync");
    }

    Ok(ScanStats {
        articles_scanned: total_articles,
        links_checked: total_links,
        broken_links_found: total_broken,
        screenshots_captured: total_screenshots,
    })
}

/// Run screenshot capture for all articles (weekly job)
#[cfg(feature = "screenshots")]
pub async fn run_screenshot_scan(pool: &PgPool) -> Result<i32, AppError> {
    tracing::info!("Starting screenshot capture scan");

    let screenshot_job = screenshot_capture::ScreenshotJob::new().await?;
    let mut captured = 0;
    let page_size = 100;
    let mut page = 1;

    // Paginate through all articles
    loop {
        let (articles, total) = articles::list_articles_with_health(
            pool,
            None,
            None,
            "age",
            "desc",
            page,
            page_size,
        )
        .await?;

        if articles.is_empty() {
            break;
        }

        tracing::info!("Processing page {} ({} articles)", page, articles.len());

        for article in &articles {
            tracing::info!("Capturing screenshot for: {}", article.title);

            match screenshot_job.capture_screenshot(&article.url).await {
                Ok(image_bytes) => {
                    // Calculate perceptual hash
                    let hash = screenshot_capture::ScreenshotJob::calculate_hash(&image_bytes)?;

                    // Store screenshot
                    screenshot_capture::store_screenshot(
                        pool,
                        article.id,
                        image_bytes,
                        hash,
                    )
                    .await?;

                    captured += 1;
                    tracing::info!("Screenshot captured for: {}", article.title);
                }
                Err(e) => {
                    tracing::warn!("Failed to capture screenshot for {}: {}", article.title, e);
                    // Continue with next article instead of failing entire scan
                }
            }

            // Small delay to avoid overwhelming the browser - configurable via SCREENSHOT_DELAY_MS
            let delay_ms = std::env::var("SCREENSHOT_DELAY_MS")
                .ok()
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(500);
            tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
        }

        // Check if we've processed all articles
        if (page * page_size) >= total {
            break;
        }

        page += 1;
    }

    tracing::info!("Screenshot scan complete: {} captured", captured);
    Ok(captured)
}

#[cfg(not(feature = "screenshots"))]
pub async fn run_screenshot_scan(_pool: &PgPool) -> Result<i32, AppError> {
    tracing::warn!("Screenshot feature not enabled. Build with --features screenshots");
    Ok(0)
}
