use crate::error::AppError;
use scraper::{Html, Selector};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Get max concurrent requests from env var or use default
fn get_max_concurrent_requests() -> usize {
    std::env::var("MAX_CONCURRENT_LINK_CHECKS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(10)
}

#[derive(Debug, Clone)]
pub struct LinkCheckResult {
    pub url: String,
    pub status_code: Option<i32>,
    pub is_broken: bool,
    pub error_message: Option<String>,
}

/// Extract all hyperlinks from XHTML content
pub fn extract_links(xhtml_body: &str, base_url: &str) -> Vec<String> {
    let document = Html::parse_fragment(xhtml_body);
    let selector = Selector::parse("a[href]").unwrap();

    let mut links = HashSet::new();

    for element in document.select(&selector) {
        if let Some(href) = element.value().attr("href") {
            // Skip anchors, mailto, javascript, etc.
            if href.starts_with('#')
                || href.starts_with("mailto:")
                || href.starts_with("javascript:")
                || href.starts_with("tel:")
            {
                continue;
            }

            // Resolve relative URLs
            let resolved_url = if href.starts_with("http://") || href.starts_with("https://") {
                href.to_string()
            } else if href.starts_with('/') {
                // Absolute path on same domain
                format!("{}{}", base_url, href)
            } else {
                // Relative path - skip for now as we'd need the article's base path
                continue;
            };

            links.insert(resolved_url);
        }
    }

    links.into_iter().collect()
}

/// Check multiple links concurrently
pub async fn check_links(
    urls: Vec<String>,
    client: &reqwest::Client,
) -> Vec<LinkCheckResult> {
    let max_concurrent = get_max_concurrent_requests();
    let semaphore = Arc::new(Semaphore::new(max_concurrent));
    let mut handles = Vec::new();

    for url in urls {
        let permit = match semaphore.clone().acquire_owned().await {
            Ok(permit) => permit,
            Err(e) => {
                tracing::error!("Failed to acquire semaphore for {}: {}", url, e);
                continue;
            }
        };
        let client = client.clone();
        let url_clone = url.clone();

        handles.push((url, tokio::spawn(async move {
            let result = check_single_link(&url_clone, &client).await;
            drop(permit);
            result
        })));
    }

    let mut results = Vec::new();
    for (url, handle) in handles {
        match handle.await {
            Ok(result) => results.push(result),
            Err(e) => {
                tracing::error!("Task join error for {}: {}", url, e);
                // Add failed task as broken link result with error details
                results.push(LinkCheckResult {
                    url,
                    status_code: None,
                    is_broken: true,
                    error_message: Some(format!("Task failed: {}", e)),
                });
            }
        }
    }

    results
}

async fn check_single_link(url: &str, client: &reqwest::Client) -> LinkCheckResult {
    tracing::debug!("Checking link: {}", url);

    // Try HEAD request first (faster)
    match client
        .head(url)
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
    {
        Ok(response) => {
            let status = response.status().as_u16() as i32;

            // If HEAD returns 405 (Method Not Allowed), retry with GET
            if status == 405 {
                return check_with_get(url, client).await;
            }

            // Link is broken if it's neither success nor redirect
            let is_broken = !response.status().is_success() && !response.status().is_redirection();

            LinkCheckResult {
                url: url.to_string(),
                status_code: Some(status),
                is_broken,
                error_message: if is_broken {
                    Some(format!("HTTP {}", status))
                } else {
                    None
                },
            }
        }
        Err(e) => {
            // HEAD failed, try GET
            if e.is_timeout() {
                LinkCheckResult {
                    url: url.to_string(),
                    status_code: None,
                    is_broken: true,
                    error_message: Some("Connection timeout".to_string()),
                }
            } else {
                check_with_get(url, client).await
            }
        }
    }
}

async fn check_with_get(url: &str, client: &reqwest::Client) -> LinkCheckResult {
    match client
        .get(url)
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
    {
        Ok(response) => {
            let status = response.status().as_u16() as i32;
            // Link is broken if it's neither success nor redirect
            let is_broken = !response.status().is_success() && !response.status().is_redirection();

            LinkCheckResult {
                url: url.to_string(),
                status_code: Some(status),
                is_broken,
                error_message: if is_broken {
                    Some(format!("HTTP {}", status))
                } else {
                    None
                },
            }
        }
        Err(e) => {
            let error_msg = if e.is_timeout() {
                "Connection timeout".to_string()
            } else if e.is_connect() {
                "Connection failed".to_string()
            } else {
                format!("{}", e)
            };

            LinkCheckResult {
                url: url.to_string(),
                status_code: None,
                is_broken: true,
                error_message: Some(error_msg),
            }
        }
    }
}

/// Store link check results in the database
pub async fn store_link_results(
    pool: &sqlx::PgPool,
    article_id: uuid::Uuid,
    results: Vec<LinkCheckResult>,
) -> Result<(), AppError> {
    for result in results {
        sqlx::query(
            "INSERT INTO link_checks (article_id, url, status_code, is_broken, error_message, checked_at)
             VALUES ($1, $2, $3, $4, $5, NOW())"
        )
        .bind(article_id)
        .bind(&result.url)
        .bind(result.status_code)
        .bind(result.is_broken)
        .bind(&result.error_message)
        .execute(pool)
        .await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_links_from_html() {
        let xhtml = r##"
            <div>
                <a href="https://example.com/page1">Link 1</a>
                <a href="http://example.org/page2">Link 2</a>
                <a href="/relative/path">Relative</a>
                <a href="#anchor">Anchor</a>
                <a href="mailto:test@example.com">Email</a>
                <a href="javascript:void(0)">JavaScript</a>
            </div>
        "##;

        let links = extract_links(xhtml, "https://confluence.example.com");

        // Should extract absolute URLs and resolve relative paths
        assert!(links.contains(&"https://example.com/page1".to_string()));
        assert!(links.contains(&"http://example.org/page2".to_string()));
        assert!(links.contains(&"https://confluence.example.com/relative/path".to_string()));

        // Should skip anchors, mailto, javascript
        assert!(!links.iter().any(|l| l.contains("#anchor")));
        assert!(!links.iter().any(|l| l.contains("mailto:")));
        assert!(!links.iter().any(|l| l.contains("javascript:")));
    }

    #[test]
    fn test_extract_links_deduplication() {
        let xhtml = r##"
            <a href="https://example.com">Link</a>
            <a href="https://example.com">Same link</a>
        "##;

        let links = extract_links(xhtml, "https://base.com");
        assert_eq!(links.len(), 1);
    }
}
