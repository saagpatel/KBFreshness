use kb_freshness_detector::{
    db::ticket_patterns::{get_for_article, insert_pattern},
    migrate::Migrator,
    postgres::PgPoolOptions,
    query, query_as,
};
use uuid::Uuid;

#[tokio::test]
async fn ticket_patterns_round_trip_jsonb_against_postgres() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping PostgreSQL integration test: DATABASE_URL is not set");
        return;
    };
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await
        .expect("connect to PostgreSQL integration database");

    Migrator::new(std::path::Path::new("./migrations"))
        .await
        .expect("load migrations")
        .run(&pool)
        .await
        .expect("apply migrations");

    let article_id = Uuid::new_v4();
    let source_id = article_id.to_string();
    query(
        "INSERT INTO articles (id, title, url, source, source_id, space_key, last_modified_at)
         VALUES ($1, $2, $3, 'confluence', $4, $5, NOW())",
    )
    .bind(article_id)
    .bind("PostgreSQL integration article")
    .bind(format!("https://example.test/articles/{article_id}"))
    .bind(source_id)
    .bind("TEST")
    .execute(&pool)
    .await
    .expect("insert integration article");

    let pattern_id = insert_pattern(
        &pool,
        Some(article_id),
        3,
        vec!["sqlx".into(), "jsonb".into()],
        "Document PostgreSQL JSON support.".into(),
    )
    .await
    .expect("insert ticket pattern with JSONB keywords");

    let patterns = get_for_article(&pool, article_id)
        .await
        .expect("read ticket pattern with JSONB keywords");
    assert_eq!(patterns.len(), 1);
    assert_eq!(patterns[0].id, pattern_id);
    assert_eq!(patterns[0].keywords, serde_json::json!(["sqlx", "jsonb"]));

    let deleted = query("DELETE FROM articles WHERE id = $1")
        .bind(article_id)
        .execute(&pool)
        .await
        .expect("clean up integration article");
    assert_eq!(deleted.rows_affected(), 1);

    let detached: (i64,) = query_as(
        "SELECT COUNT(*) FROM ticket_patterns
         WHERE id = $1 AND related_article_id IS NULL",
    )
    .bind(pattern_id)
    .fetch_one(&pool)
    .await
    .expect("verify article deletion detaches the ticket pattern");
    assert_eq!(detached.0, 1);

    let deleted_pattern = query("DELETE FROM ticket_patterns WHERE id = $1")
        .bind(pattern_id)
        .execute(&pool)
        .await
        .expect("clean up integration ticket pattern");
    assert_eq!(deleted_pattern.rows_affected(), 1);
}
