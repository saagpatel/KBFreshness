use crate::config::Config;
use crate::db::{articles, ticket_patterns};
use crate::error::AppError;
use serde::Deserialize;
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

#[cfg(feature = "tickets")]
use strsim::jaro_winkler;

/// Jira ticket from search API
#[derive(Debug, Deserialize)]
struct JiraSearchResponse {
    issues: Vec<JiraIssue>,
    #[serde(rename = "startAt")]
    start_at: i32,
    #[serde(rename = "maxResults")]
    max_results: i32,
    total: i32,
}

#[derive(Debug, Deserialize)]
struct JiraIssue {
    key: String,
    fields: JiraFields,
}

#[derive(Debug, Deserialize)]
struct JiraFields {
    summary: String,
    updated: String,
}

/// Jira REST client
pub struct JiraClient {
    base_url: String,
    email: String,
    api_token: String,
    client: reqwest::Client,
}

impl JiraClient {
    pub fn new(base_url: String, email: String, api_token: String) -> Self {
        Self {
            base_url,
            email,
            api_token,
            client: reqwest::Client::new(),
        }
    }

    /// Fetch recent tickets (last 30 days)
    pub async fn fetch_recent_tickets(&self) -> Result<Vec<Ticket>, AppError> {
        let jql = "updated >= -30d ORDER BY updated DESC";
        let max_results = 100;
        let mut start_at = 0;
        let mut all_tickets = Vec::new();

        loop {
            let url = format!(
                "{}/rest/api/2/search?jql={}&startAt={}&maxResults={}",
                self.base_url,
                urlencoding::encode(jql),
                start_at,
                max_results
            );

            tracing::debug!("Fetching Jira tickets: {}", url);

            let response = self
                .client
                .get(&url)
                .basic_auth(&self.email, Some(&self.api_token))
                .send()
                .await
                .map_err(|e| AppError::ExternalApi(e))?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                return Err(AppError::Internal(format!(
                    "Jira API returned {}: {}",
                    status,
                    body.chars().take(200).collect::<String>()
                )));
            }

            let search_response: JiraSearchResponse = response
                .json()
                .await
                .map_err(|e| AppError::Internal(format!("Failed to parse Jira response: {}", e)))?;

            for issue in search_response.issues {
                all_tickets.push(Ticket {
                    key: issue.key,
                    summary: issue.fields.summary,
                    updated: issue.fields.updated,
                });
            }

            // Check if we need to paginate
            if start_at + max_results >= search_response.total {
                break;
            }

            start_at += max_results;
        }

        tracing::info!("Fetched {} tickets from Jira", all_tickets.len());
        Ok(all_tickets)
    }
}

#[derive(Debug, Clone)]
pub struct Ticket {
    pub key: String,
    pub summary: String,
    pub updated: String,
}

// Common English stopwords to filter out from ticket keywords
const STOPWORDS: &[&str] = &[
    "the", "be", "to", "of", "and", "a", "in", "that", "have", "i", "it", "for", "not", "on",
    "with", "he", "as", "you", "do", "at", "this", "but", "his", "by", "from", "they", "we", "say",
    "her", "she", "or", "an", "will", "my", "one", "all", "would", "there", "their", "what", "so",
    "up", "out", "if", "about", "who", "get", "which", "go", "me", "when", "make", "can", "like",
    "time", "no", "just", "him", "know", "take", "people", "into", "year", "your", "good", "some",
    "could", "them", "see", "other", "than", "then", "now", "look", "only", "come", "its", "over",
    "think", "also", "back", "after", "use", "two", "how", "our", "work", "first", "well", "way",
    "even", "new", "want", "because", "any", "these", "give", "day", "most", "us", "is", "was",
    "are", "been", "has", "had", "were", "said", "did", "having", "may", "should", "does", "am",
];

/// Extract keywords from ticket summaries
pub fn extract_keywords(tickets: &[Ticket]) -> HashMap<String, usize> {
    let mut keyword_counts = HashMap::new();

    for ticket in tickets {
        // Split summary into words, lowercase, filter stopwords
        let words: Vec<String> = ticket
            .summary
            .split_whitespace()
            .map(|w| {
                w.to_lowercase()
                    .trim_matches(|c: char| !c.is_alphanumeric())
                    .to_string()
            })
            .filter(|w| w.len() > 3 && !STOPWORDS.contains(&w.as_str()))
            .collect();

        for word in words {
            *keyword_counts.entry(word).or_insert(0) += 1;
        }
    }

    keyword_counts
}

/// Correlate tickets with articles based on keyword similarity
#[cfg(feature = "tickets")]
pub fn correlate_with_articles(
    tickets: &[Ticket],
    articles: &[articles::ArticleWithHealth],
    similarity_threshold: f64,
) -> HashMap<Uuid, Vec<String>> {
    let mut correlations: HashMap<Uuid, Vec<String>> = HashMap::new();

    // Extract keywords per ticket
    let keywords = extract_keywords(tickets);

    // Get top keywords (frequency > 2)
    let top_keywords: Vec<String> = keywords
        .iter()
        .filter(|(_, &count)| count > 2)
        .map(|(word, _)| word.clone())
        .collect();

    for article in articles {
        let article_title_lower = article.title.to_lowercase();
        let mut matched_keywords = Vec::new();

        for keyword in &top_keywords {
            // Direct substring match (case-insensitive)
            if article_title_lower.contains(keyword) {
                matched_keywords.push(keyword.clone());
                continue;
            }

            // Fuzzy match using Jaro-Winkler distance
            // Split article title into words and check similarity
            for title_word in article_title_lower.split_whitespace() {
                let cleaned_word = title_word
                    .trim_matches(|c: char| !c.is_alphanumeric())
                    .to_string();

                if cleaned_word.len() > 3 {
                    let similarity = jaro_winkler(&cleaned_word, keyword);
                    if similarity > similarity_threshold {
                        matched_keywords.push(keyword.clone());
                        break;
                    }
                }
            }
        }

        if !matched_keywords.is_empty() {
            // Deduplicate keywords
            matched_keywords.sort();
            matched_keywords.dedup();
            correlations.insert(article.id, matched_keywords);
        }
    }

    correlations
}

#[cfg(not(feature = "tickets"))]
pub fn correlate_with_articles(
    _tickets: &[Ticket],
    _articles: &[articles::ArticleWithHealth],
    _similarity_threshold: f64,
) -> HashMap<Uuid, Vec<String>> {
    tracing::warn!("Ticket correlation feature not enabled. Build with --features tickets");
    HashMap::new()
}

/// Generate LLM suggestion for article update (optional)
pub async fn generate_suggestion(
    ollama_url: &str,
    article_title: &str,
    article_age_days: i64,
    ticket_count: usize,
    keywords: &[String],
    client: &reqwest::Client,
) -> Result<String, AppError> {
    let prompt = format!(
        "Article '{}' is {} days old. {} support tickets mention similar topics: {}. Suggest a specific update to this article in one sentence.",
        article_title,
        article_age_days,
        ticket_count,
        keywords.join(", ")
    );

    let request_body = serde_json::json!({
        "model": "llama3.2",
        "prompt": prompt,
        "stream": false,
    });

    let response = client
        .post(format!("{}/api/generate", ollama_url))
        .json(&request_body)
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("Ollama request failed: {}", e)))?;

    if !response.status().is_success() {
        return Err(AppError::Internal(format!(
            "Ollama returned status: {}",
            response.status()
        )));
    }

    #[derive(Deserialize)]
    struct OllamaResponse {
        response: String,
    }

    let ollama_response: OllamaResponse = response
        .json()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to parse Ollama response: {}", e)))?;

    Ok(ollama_response.response.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_keywords_filters_stopwords() {
        let tickets = vec![
            Ticket {
                key: "TICKET-1".to_string(),
                summary: "The authentication system is broken".to_string(),
                updated: "2024-01-01T00:00:00Z".to_string(),
            },
            Ticket {
                key: "TICKET-2".to_string(),
                summary: "Authentication fails for users".to_string(),
                updated: "2024-01-01T00:00:00Z".to_string(),
            },
        ];

        let keywords = extract_keywords(&tickets);

        // "authentication" should appear twice
        assert_eq!(keywords.get("authentication"), Some(&2));

        // Stopwords like "the", "is", "for" should not appear
        assert_eq!(keywords.get("the"), None);
        assert_eq!(keywords.get("is"), None);
        assert_eq!(keywords.get("for"), None);
    }

    #[test]
    #[cfg(feature = "tickets")]
    fn test_correlate_exact_match() {
        let tickets = vec![
            Ticket {
                key: "TICKET-1".to_string(),
                summary: "Authentication system problems authentication login".to_string(),
                updated: "2024-01-01T00:00:00Z".to_string(),
            },
            Ticket {
                key: "TICKET-2".to_string(),
                summary: "User authentication failing authentication error".to_string(),
                updated: "2024-01-02T00:00:00Z".to_string(),
            },
        ];

        let articles = vec![articles::ArticleWithHealth {
            id: Uuid::new_v4(),
            title: "Authentication Guide".to_string(),
            url: "http://example.com".to_string(),
            source: "confluence".to_string(),
            source_id: None,
            space_key: None,
            last_modified_at: chrono::Utc::now(),
            last_modified_by: None,
            version_number: 1,
            effective_age_days: 100,
            broken_link_count: 0,
            health: crate::health::HealthStatus::Green,
            manually_flagged: false,
            reviewed_at: None,
            reviewed_by: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }];

        let correlations = correlate_with_articles(&tickets, &articles, 0.85);

        // Should correlate because "authentication" appears in title
        assert_eq!(correlations.len(), 1);
        assert!(correlations
            .values()
            .next()
            .unwrap()
            .contains(&"authentication".to_string()));
    }
}

/// Run full ticket analysis job (weekly)
pub async fn run_ticket_analysis(pool: &PgPool, config: &Config) -> Result<i32, AppError> {
    tracing::info!("Starting ticket analysis job");

    // Check if Jira is configured
    let (base_url, email, api_token) = match (
        &config.jira_base_url,
        &config.jira_email,
        &config.jira_api_token,
    ) {
        (Some(url), Some(email), Some(token)) => (url, email, token),
        _ => {
            tracing::warn!("Jira credentials not configured, skipping ticket analysis");
            return Ok(0);
        }
    };

    // Fetch tickets from Jira
    let client = JiraClient::new(base_url.clone(), email.clone(), api_token.clone());
    let tickets = match client.fetch_recent_tickets().await {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("Failed to fetch Jira tickets: {}", e);
            return Err(e);
        }
    };

    if tickets.is_empty() {
        tracing::info!("No recent tickets found");
        return Ok(0);
    }

    tracing::info!("Fetched {} tickets from Jira", tickets.len());

    // Get all articles
    let (all_articles, _) =
        articles::list_articles_with_health(pool, None, None, "age", "desc", 1, 1000).await?;

    // Correlate tickets with articles
    let correlations = correlate_with_articles(&tickets, &all_articles, 0.8);

    tracing::info!("Found correlations for {} articles", correlations.len());

    // Create HTTP client for LLM requests (reused across all suggestions)
    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| AppError::Internal(format!("Failed to build HTTP client: {}", e)))?;

    let mut patterns_created = 0;

    for (article_id, keywords) in correlations {
        // Find the article
        let article = all_articles
            .iter()
            .find(|a| a.id == article_id)
            .ok_or_else(|| AppError::Internal("Article not found in list".into()))?;

        // Count tickets matching these keywords
        let ticket_count = tickets
            .iter()
            .filter(|t| {
                let summary_lower = t.summary.to_lowercase();
                keywords.iter().any(|k| summary_lower.contains(k))
            })
            .count();

        // Generate suggestion (optional)
        let suggestion = if let Some(ollama_url) = &config.ollama_url {
            match generate_suggestion(
                ollama_url,
                &article.title,
                article.effective_age_days,
                ticket_count,
                &keywords,
                &http_client,
            )
            .await
            {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!("Failed to generate LLM suggestion: {}", e);
                    format!(
                        "Consider updating this article - {} related tickets mention: {}",
                        ticket_count,
                        keywords.join(", ")
                    )
                }
            }
        } else {
            format!(
                "Consider updating this article - {} related tickets mention: {}",
                ticket_count,
                keywords.join(", ")
            )
        };

        // Clear old patterns for this article
        ticket_patterns::clear_for_article(pool, article_id).await?;

        // Insert new pattern
        ticket_patterns::insert_pattern(
            pool,
            Some(article_id),
            ticket_count as i32,
            keywords.clone(),
            suggestion,
        )
        .await?;

        patterns_created += 1;

        tracing::info!(
            "Created pattern for article '{}': {} tickets, keywords: {:?}",
            article.title,
            ticket_count,
            keywords
        );
    }

    tracing::info!(
        "Ticket analysis complete: {} patterns created",
        patterns_created
    );
    Ok(patterns_created)
}
