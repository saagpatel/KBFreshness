use crate::error::AppError;
use chrono::{DateTime, Utc};
use serde::Deserialize;

pub struct ConfluenceClient {
    base_url: String,
    email: String,
    api_token: String,
    client: reqwest::Client,
}

#[derive(Debug)]
pub struct ConfluencePage {
    pub id: String,
    pub title: String,
    pub url: String,
    pub space_key: String,
    pub last_modified_at: DateTime<Utc>,
    pub last_modified_by: Option<String>,
    pub version_number: i32,
    /// Body content in Confluence storage format (XHTML-based)
    pub body_storage_format: String,
}

#[derive(Deserialize)]
struct ConfluenceResponse {
    results: Vec<ConfluencePageJson>,
    #[serde(rename = "_links")]
    links: Option<ConfluenceLinks>,
}

#[derive(Deserialize)]
struct ConfluenceLinks {
    next: Option<String>,
}

#[derive(Deserialize)]
struct ConfluencePageJson {
    id: String,
    title: String,
    #[serde(rename = "type")]
    page_type: String,
    space: ConfluenceSpace,
    version: ConfluenceVersion,
    body: Option<ConfluenceBody>,
    #[serde(rename = "_links")]
    links: ConfluencePageLinks,
}

#[derive(Deserialize)]
struct ConfluenceSpace {
    key: String,
}

#[derive(Deserialize)]
struct ConfluenceVersion {
    number: i32,
    when: String,
    by: Option<ConfluenceUser>,
}

#[derive(Deserialize)]
struct ConfluenceUser {
    #[serde(rename = "displayName")]
    display_name: Option<String>,
}

#[derive(Deserialize)]
struct ConfluenceBody {
    storage: Option<ConfluenceStorage>,
}

#[derive(Deserialize)]
struct ConfluenceStorage {
    value: String,
}

#[derive(Deserialize)]
struct ConfluencePageLinks {
    webui: String,
}

impl ConfluenceClient {
    pub fn new(base_url: String, email: String, api_token: String) -> Self {
        Self {
            base_url,
            email,
            api_token,
            client: reqwest::Client::new(),
        }
    }

    /// Fetch all pages from a Confluence space
    pub async fn fetch_all_pages(&self, space_key: &str) -> Result<Vec<ConfluencePage>, AppError> {
        let mut all_pages = Vec::new();
        let mut next_url: Option<String> = Some(format!(
            "{}/rest/api/content?spaceKey={}&limit=100&expand=version,body.storage,space",
            self.base_url, space_key
        ));

        while let Some(url) = next_url {
            tracing::info!("Fetching Confluence pages from: {}", url);

            let response = self
                .client
                .get(&url)
                .basic_auth(&self.email, Some(&self.api_token))
                .send()
                .await?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                // Limit error body to 500 chars to prevent log flooding
                let body_excerpt: String = body.chars().take(500).collect();
                tracing::error!("Confluence API error: {} - {}", status, body_excerpt);
                return Err(AppError::Internal(format!(
                    "Confluence API returned {}: {}",
                    status, body_excerpt
                )));
            }

            let confluence_response: ConfluenceResponse = response.json().await?;

            for page_json in confluence_response.results {
                // Only process actual pages, not blog posts or attachments
                if page_json.page_type != "page" {
                    continue;
                }

                let page = self.parse_page(page_json)?;
                all_pages.push(page);
            }

            // Check for next page
            next_url = confluence_response
                .links
                .and_then(|l| l.next)
                .map(|next| format!("{}{}", self.base_url, next));
        }

        tracing::info!("Fetched {} pages from space {}", all_pages.len(), space_key);
        Ok(all_pages)
    }

    /// Fetch a single page by ID
    pub async fn fetch_page(&self, page_id: &str) -> Result<ConfluencePage, AppError> {
        let url = format!(
            "{}/rest/api/content/{}?expand=version,body.storage,space",
            self.base_url, page_id
        );

        tracing::debug!("Fetching Confluence page: {}", page_id);

        let response = self
            .client
            .get(&url)
            .basic_auth(&self.email, Some(&self.api_token))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            return Err(AppError::Internal(format!(
                "Confluence API returned {}",
                status
            )));
        }

        let page_json: ConfluencePageJson = response.json().await?;
        self.parse_page(page_json)
    }

    fn parse_page(&self, page_json: ConfluencePageJson) -> Result<ConfluencePage, AppError> {
        // Parse the ISO 8601 timestamp from Confluence
        let last_modified_at = DateTime::parse_from_rfc3339(&page_json.version.when)
            .map_err(|e| AppError::Internal(format!("Failed to parse Confluence date: {}", e)))?
            .with_timezone(&Utc);

        let body_storage_format = page_json
            .body
            .and_then(|b| b.storage)
            .map(|s| s.value)
            .unwrap_or_default();

        let url = format!("{}{}", self.base_url, page_json.links.webui);

        Ok(ConfluencePage {
            id: page_json.id,
            title: page_json.title,
            url,
            space_key: page_json.space.key,
            last_modified_at,
            last_modified_by: page_json.version.by.and_then(|u| u.display_name),
            version_number: page_json.version.number,
            body_storage_format,
        })
    }
}
