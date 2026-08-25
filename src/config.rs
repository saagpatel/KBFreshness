use sqlx::{postgres::PgPoolOptions, PgPool};
use std::time::Duration;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Config,
}

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub background_automation_enabled: bool,
    pub confluence_base_url: Option<String>,
    pub confluence_email: Option<String>,
    pub confluence_api_token: Option<String>,
    pub notion_api_key: Option<String>,
    pub jira_base_url: Option<String>,
    pub jira_email: Option<String>,
    pub jira_api_token: Option<String>,
    pub ollama_url: Option<String>,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        dotenvy::dotenv().ok(); // Don't fail if .env doesn't exist

        let database_url = std::env::var("DATABASE_URL")
            .map_err(|_| "DATABASE_URL environment variable is required".to_string())?;

        Ok(Config {
            database_url,
            background_automation_enabled: parse_background_automation_enabled(
                std::env::var("BACKGROUND_AUTOMATION_ENABLED")
                    .ok()
                    .as_deref(),
            ),
            confluence_base_url: std::env::var("CONFLUENCE_BASE_URL").ok(),
            confluence_email: std::env::var("CONFLUENCE_EMAIL").ok(),
            confluence_api_token: std::env::var("CONFLUENCE_API_TOKEN").ok(),
            notion_api_key: std::env::var("NOTION_API_KEY").ok(),
            jira_base_url: std::env::var("JIRA_BASE_URL").ok(),
            jira_email: std::env::var("JIRA_EMAIL").ok(),
            jira_api_token: std::env::var("JIRA_API_TOKEN").ok(),
            ollama_url: std::env::var("OLLAMA_URL").ok(),
        })
    }
}

fn parse_background_automation_enabled(value: Option<&str>) -> bool {
    matches!(
        value.map(str::trim).map(str::to_ascii_lowercase).as_deref(),
        Some("1" | "true" | "yes")
    )
}

pub async fn create_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(10)
        .min_connections(2)
        .acquire_timeout(Duration::from_secs(5))
        .idle_timeout(Duration::from_secs(300))
        .max_lifetime(Duration::from_secs(1800))
        .connect(database_url)
        .await
}

#[cfg(test)]
mod tests {
    use super::parse_background_automation_enabled;

    #[test]
    fn background_automation_is_fail_closed() {
        assert!(!parse_background_automation_enabled(None));
        assert!(!parse_background_automation_enabled(Some("0")));
        assert!(!parse_background_automation_enabled(Some("unexpected")));
        assert!(parse_background_automation_enabled(Some("true")));
        assert!(parse_background_automation_enabled(Some("1")));
    }
}
