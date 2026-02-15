export interface Article {
  id: string;
  title: string;
  url: string;
  source: 'confluence' | 'notion' | 'url';
  source_id: string | null;
  space_key: string | null;
  last_modified_at: string;
  last_modified_by: string | null;
  version_number: number;
  effective_age_days: number;
  broken_link_count: number;
  health: 'green' | 'yellow' | 'red';
  manually_flagged: boolean;
  reviewed_at: string | null;
  reviewed_by: string | null;
  created_at: string;
  updated_at: string;
}

export interface ArticleDetail extends Article {
  ticket_patterns: TicketPattern[];
}

export interface ArticleListResponse {
  articles: Article[];
  total: number;
  page: number;
  limit: number;
}

export interface ScanRun {
  id: string;
  scan_type: string;
  started_at: string;
  completed_at: string | null;
  articles_scanned: number;
  links_checked: number;
  broken_links_found: number;
  status: 'running' | 'completed' | 'failed';
  error_message: string | null;
}

export interface LinkCheck {
  id: string;
  article_id: string;
  url: string;
  status_code: number | null;
  is_broken: boolean;
  error_message: string | null;
  checked_at: string;
}

export interface Screenshot {
  id: string;
  article_id: string;
  perceptual_hash: string | null;
  captured_at: string;
  drift_distance: number | null;
  needs_update: boolean;
}

export interface TicketPattern {
  id: string;
  ticket_category: string;
  ticket_count: number;
  related_article_id: string | null;
  keywords: string[];
  suggested_update: string;
  detected_at: string;
}
