extern crate self as sqlx;

mod api;
mod config;
mod db;
mod error;
mod health;
mod jobs;
mod security;
mod sources;
mod sqlx_compat;

pub use sqlx_compat::{migrate, postgres, query, query_as, Error, FromRow, PgPool, Row};

use config::{create_pool, AppState, Config};
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "kb_freshness_detector=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting KB Freshness Detector");

    // Load config
    let config = Config::from_env().map_err(|e| format!("Failed to load configuration: {}", e))?;

    // Create database pool
    tracing::info!("Connecting to database...");
    let pool = create_pool(&config.database_url)
        .await
        .map_err(|e| format!("Failed to connect to database: {}", e))?;

    tracing::info!("Database connection established");

    // Run migrations
    tracing::info!("Running database migrations...");
    sqlx::migrate::Migrator::new(std::path::Path::new("./migrations"))
        .await
        .map_err(|e| format!("Failed to initialize migrations: {}", e))?
        .run(&pool)
        .await
        .map_err(|e| format!("Failed to run migrations: {}", e))?;

    tracing::info!("Migrations complete");

    // Create app state
    let state = AppState {
        db: pool.clone(),
        config: config.clone(),
    };

    // Set up job scheduler
    let scheduler = JobScheduler::new()
        .await
        .map_err(|e| format!("Failed to create scheduler: {}", e))?;

    let mut job_count = 0;

    // Phase 2: Daily link check at 2 AM
    let pool_clone = pool.clone();
    let config_clone = config.clone();
    scheduler
        .add(Job::new_async("0 0 2 * * *", move |_uuid, _lock| {
            let pool = pool_clone.clone();
            let config = config_clone.clone();
            Box::pin(async move {
                tracing::info!("Starting scheduled freshness scan");
                const MAX_RETRIES: u32 = 3;
                for attempt in 1..=MAX_RETRIES {
                    match jobs::freshness_scan::run_full_scan(&pool, &config).await {
                        Ok(stats) => {
                            tracing::info!(
                                "Freshness scan succeeded: {} articles, {} links checked",
                                stats.articles_scanned,
                                stats.links_checked
                            );
                            break;
                        }
                        Err(e) => {
                            if attempt < MAX_RETRIES {
                                tracing::warn!(
                                    "Freshness scan failed (attempt {}/{}): {}. Retrying in 5 minutes...",
                                    attempt,
                                    MAX_RETRIES,
                                    e
                                );
                                tokio::time::sleep(tokio::time::Duration::from_secs(300)).await;
                            } else {
                                tracing::error!(
                                    "Freshness scan failed after {} attempts: {}",
                                    MAX_RETRIES,
                                    e
                                );
                            }
                        }
                    }
                }
            })
        })?)
        .await
        .map_err(|e| format!("Failed to add link check job: {}", e))?;
    job_count += 1;

    // Phase 3: Daily screenshot cleanup at 3 AM
    let pool_clone = pool.clone();
    scheduler
        .add(Job::new_async("0 0 3 * * *", move |_uuid, _lock| {
            let pool = pool_clone.clone();
            #[cfg(not(feature = "screenshots"))]
            let _ = &pool;
            Box::pin(async move {
                tracing::info!("Starting scheduled screenshot cleanup");
                #[cfg(feature = "screenshots")]
                {
                    match jobs::screenshot_cleanup::cleanup_old_screenshots(&pool).await {
                        Ok(count) => tracing::info!("Cleaned up {} old screenshots", count),
                        Err(e) => tracing::error!("Screenshot cleanup failed: {}", e),
                    }
                }
                #[cfg(not(feature = "screenshots"))]
                {
                    tracing::debug!("Screenshot cleanup skipped (feature not enabled)");
                }
            })
        })?)
        .await
        .map_err(|e| format!("Failed to add cleanup job: {}", e))?;
    job_count += 1;

    // Phase 3: Weekly screenshot capture on Mondays at 4 AM
    let pool_clone = pool.clone();
    scheduler
        .add(Job::new_async("0 0 4 * * MON", move |_uuid, _lock| {
            let pool = pool_clone.clone();
            Box::pin(async move {
                tracing::info!("Starting scheduled screenshot capture");
                match jobs::freshness_scan::run_screenshot_scan(&pool).await {
                    Ok(count) => tracing::info!("Captured {} screenshots", count),
                    Err(e) => tracing::error!("Screenshot scan failed: {}", e),
                }
            })
        })?)
        .await
        .map_err(|e| format!("Failed to add screenshot job: {}", e))?;
    job_count += 1;

    // Phase 4: Weekly ticket analysis on Mondays at 5 AM
    let pool_clone = pool.clone();
    let config_clone = config.clone();
    scheduler
        .add(Job::new_async("0 0 5 * * MON", move |_uuid, _lock| {
            let pool = pool_clone.clone();
            let config = config_clone.clone();
            Box::pin(async move {
                tracing::info!("Starting scheduled ticket analysis");
                match jobs::ticket_analyzer::run_ticket_analysis(&pool, &config).await {
                    Ok(count) => tracing::info!("Created {} ticket patterns", count),
                    Err(e) => tracing::error!("Ticket analysis failed: {}", e),
                }
            })
        })?)
        .await
        .map_err(|e| format!("Failed to add ticket analysis job: {}", e))?;
    job_count += 1;

    scheduler
        .start()
        .await
        .map_err(|e| format!("Failed to start scheduler: {}", e))?;

    tracing::info!("Scheduler started with {} jobs", job_count);

    // Build router
    let app = api::create_router(state);

    // Start server
    let bind_address =
        std::env::var("BIND_ADDRESS").unwrap_or_else(|_| "127.0.0.1:3001".to_string());

    let listener = tokio::net::TcpListener::bind(&bind_address)
        .await
        .map_err(|e| format!("Failed to bind to {}: {}", bind_address, e))?;

    tracing::info!("Server listening on http://{}", bind_address);

    axum::serve(listener, app)
        .await
        .map_err(|e| format!("Server error: {}", e))?;

    Ok(())
}
