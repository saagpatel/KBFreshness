use crate::error::AppError;
use crate::health::{compute_health, HealthStatus};
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

const DEFAULT_FRESHNESS_THRESHOLD_DAYS: i64 = 90;

#[derive(Serialize)]
pub struct ArticleWithHealth {
    pub id: Uuid,
    pub title: String,
    pub url: String,
    pub source: String,
    pub source_id: Option<String>,
    pub space_key: Option<String>,
    pub last_modified_at: DateTime<Utc>,
    pub last_modified_by: Option<String>,
    pub version_number: i32,
    pub effective_age_days: i64,
    pub broken_link_count: i64,
    pub health: HealthStatus,
    pub manually_flagged: bool,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub reviewed_by: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(sqlx::Type, Serialize, Clone, Debug)]
#[sqlx(type_name = "source_type", rename_all = "lowercase")]
pub enum SourceType {
    Confluence,
    Notion,
    Url,
}

pub struct InsertArticle {
    pub title: String,
    pub url: String,
    pub source: SourceType,
    pub source_id: Option<String>,
    pub space_key: Option<String>,
    pub last_modified_at: DateTime<Utc>,
    pub last_modified_by: Option<String>,
    pub version_number: i32,
}

/// Insert a new article
pub async fn insert_article(pool: &PgPool, article: InsertArticle) -> Result<Uuid, AppError> {
    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO articles (title, url, source, source_id, space_key, last_modified_at, last_modified_by, version_number) VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING id"
    )
    .bind(&article.title)
    .bind(&article.url)
    .bind(&article.source)
    .bind(&article.source_id)
    .bind(&article.space_key)
    .bind(article.last_modified_at)
    .bind(&article.last_modified_by)
    .bind(article.version_number)
    .fetch_one(pool)
    .await?;

    Ok(row.0)
}

/// Upsert article from external source (Confluence/Notion)
/// Updates if source_id exists, inserts otherwise
pub async fn upsert_from_source(
    pool: &PgPool,
    article: InsertArticle,
) -> Result<Uuid, AppError> {
    // Try to find existing article by source_id
    if let Some(source_id) = &article.source_id {
        let existing: Option<(Uuid,)> = sqlx::query_as(
            "SELECT id FROM articles WHERE source_id = $1 AND source = $2"
        )
        .bind(source_id)
        .bind(&article.source)
        .fetch_optional(pool)
        .await?;

        if let Some((id,)) = existing {
            // Update existing article (preserve reviewed_at)
            sqlx::query(
                "UPDATE articles
                 SET title = $1, url = $2, space_key = $3, last_modified_at = $4,
                     last_modified_by = $5, version_number = $6, updated_at = NOW()
                 WHERE id = $7"
            )
            .bind(&article.title)
            .bind(&article.url)
            .bind(&article.space_key)
            .bind(article.last_modified_at)
            .bind(&article.last_modified_by)
            .bind(article.version_number)
            .bind(id)
            .execute(pool)
            .await?;

            return Ok(id);
        }
    }

    // Insert new article
    insert_article(pool, article).await
}

/// List articles with computed health status
pub async fn list_articles_with_health(
    pool: &PgPool,
    health_filter: Option<&str>,
    space_filter: Option<&str>,
    sort_by: &str,
    sort_order: &str,
    page: i64,
    limit: i64,
) -> Result<(Vec<ArticleWithHealth>, i64), AppError> {
    let offset = (page - 1) * limit;

    // Get total count first
    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM articles WHERE ($1::TEXT IS NULL OR space_key = $1)"
    )
    .bind(space_filter)
    .fetch_one(pool)
    .await?;
    let total = total.0;

    // Validate and whitelist sort_by parameter
    let order_by = match sort_by {
        "age" => "last_modified_at",
        "title" => "title",
        _ => return Err(AppError::BadRequest(format!("Invalid sort_by parameter: {}", sort_by))),
    };

    // Validate and whitelist direction parameter
    let direction = match sort_order.to_lowercase().as_str() {
        "asc" => "ASC",
        "desc" => "DESC",
        _ => return Err(AppError::BadRequest(format!("Invalid sort_order parameter: {}", sort_order))),
    };

    let query = format!(
        r#"
        SELECT
            a.id,
            a.title,
            a.url,
            a.source as "source: SourceType",
            a.source_id,
            a.space_key,
            a.last_modified_at,
            a.last_modified_by,
            a.version_number,
            a.freshness_threshold_days,
            a.manually_flagged,
            a.reviewed_at,
            a.reviewed_by,
            a.created_at,
            a.updated_at,
            COALESCE(lc.broken_count, 0) as broken_count
        FROM articles a
        LEFT JOIN (
            SELECT article_id, COUNT(*) as broken_count
            FROM link_checks
            WHERE is_broken = TRUE
            GROUP BY article_id
        ) lc ON a.id = lc.article_id
        WHERE ($1::TEXT IS NULL OR a.space_key = $1)
        ORDER BY {} {}
        LIMIT $2 OFFSET $3
        "#,
        order_by, direction
    );

    let rows = sqlx::query_as::<_, ArticleRow>(&query)
        .bind(space_filter)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

    let now = Utc::now();
    let mut articles: Vec<ArticleWithHealth> = rows
        .into_iter()
        .map(|row| {
            let threshold = row.freshness_threshold_days.unwrap_or(DEFAULT_FRESHNESS_THRESHOLD_DAYS);
            let effective_date = row.reviewed_at.unwrap_or(row.last_modified_at);
            let effective_age_days = (now - effective_date).num_days().max(0);

            let broken_count = row.broken_count.unwrap_or(0);
            let health = compute_health(
                effective_age_days,
                broken_count,
                threshold,
                row.manually_flagged,
            );

            ArticleWithHealth {
                id: row.id,
                title: row.title,
                url: row.url,
                source: format!("{:?}", row.source).to_lowercase(),
                source_id: row.source_id,
                space_key: row.space_key,
                last_modified_at: row.last_modified_at,
                last_modified_by: row.last_modified_by,
                version_number: row.version_number,
                effective_age_days,
                broken_link_count: broken_count,
                health,
                manually_flagged: row.manually_flagged,
                reviewed_at: row.reviewed_at,
                reviewed_by: row.reviewed_by,
                created_at: row.created_at,
                updated_at: row.updated_at,
            }
        })
        .collect();

    // Apply health filter post-query (since it's computed)
    let filtered_total = if let Some(health_str) = health_filter {
        let filter_health = match health_str.to_lowercase().as_str() {
            "green" => HealthStatus::Green,
            "yellow" => HealthStatus::Yellow,
            "red" => HealthStatus::Red,
            _ => return Err(AppError::BadRequest("Invalid health filter".into())),
        };
        articles.retain(|a| a.health == filter_health);
        articles.len() as i64
    } else {
        total
    };

    Ok((articles, filtered_total))
}

/// Get article by ID with health status
pub async fn get_article_by_id(pool: &PgPool, id: Uuid) -> Result<ArticleWithHealth, AppError> {
    let row = sqlx::query_as::<_, ArticleRow>(
        r#"
        SELECT
            a.id,
            a.title,
            a.url,
            a.source as "source: SourceType",
            a.source_id,
            a.space_key,
            a.last_modified_at,
            a.last_modified_by,
            a.version_number,
            a.freshness_threshold_days,
            a.manually_flagged,
            a.reviewed_at,
            a.reviewed_by,
            a.created_at,
            a.updated_at,
            COALESCE(lc.broken_count, 0) as broken_count
        FROM articles a
        LEFT JOIN (
            SELECT article_id, COUNT(*) as broken_count
            FROM link_checks
            WHERE is_broken = TRUE
            GROUP BY article_id
        ) lc ON a.id = lc.article_id
        WHERE a.id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("article with id {}", id)))?;

    let now = Utc::now();
    let threshold = row.freshness_threshold_days.unwrap_or(DEFAULT_FRESHNESS_THRESHOLD_DAYS);
    let effective_date = row.reviewed_at.unwrap_or(row.last_modified_at);
    let effective_age_days = (now - effective_date).num_days().max(0);
    let broken_count = row.broken_count.unwrap_or(0);

    let health = compute_health(
        effective_age_days,
        broken_count,
        threshold,
        row.manually_flagged,
    );

    Ok(ArticleWithHealth {
        id: row.id,
        title: row.title,
        url: row.url,
        source: format!("{:?}", row.source).to_lowercase(),
        source_id: row.source_id,
        space_key: row.space_key,
        last_modified_at: row.last_modified_at,
        last_modified_by: row.last_modified_by,
        version_number: row.version_number,
        effective_age_days,
        broken_link_count: broken_count,
        health,
        manually_flagged: row.manually_flagged,
        reviewed_at: row.reviewed_at,
        reviewed_by: row.reviewed_by,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

/// Mark article as reviewed
pub async fn mark_reviewed(
    pool: &PgPool,
    id: Uuid,
    reviewed_by: &str,
) -> Result<(), AppError> {
    let result = sqlx::query(
        "UPDATE articles SET reviewed_at = NOW(), reviewed_by = $1, updated_at = NOW() WHERE id = $2"
    )
    .bind(reviewed_by)
    .bind(id)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("article with id {}", id)));
    }

    Ok(())
}

/// Set manual flag on article
pub async fn set_manual_flag(pool: &PgPool, id: Uuid, flagged: bool) -> Result<(), AppError> {
    let result = sqlx::query(
        "UPDATE articles SET manually_flagged = $1, updated_at = NOW() WHERE id = $2"
    )
    .bind(flagged)
    .bind(id)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("article with id {}", id)));
    }

    Ok(())
}

#[derive(sqlx::FromRow)]
struct ArticleRow {
    id: Uuid,
    title: String,
    url: String,
    source: SourceType,
    source_id: Option<String>,
    space_key: Option<String>,
    last_modified_at: DateTime<Utc>,
    last_modified_by: Option<String>,
    version_number: i32,
    freshness_threshold_days: Option<i64>,
    manually_flagged: bool,
    reviewed_at: Option<DateTime<Utc>>,
    reviewed_by: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    broken_count: Option<i64>,
}
