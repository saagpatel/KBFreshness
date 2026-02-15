-- Initial schema for KB Freshness Detector

CREATE TYPE source_type AS ENUM ('confluence', 'notion', 'url');

CREATE TABLE articles (
    id                       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title                    TEXT NOT NULL,
    url                      TEXT NOT NULL UNIQUE,
    source                   source_type NOT NULL,
    source_id                TEXT,          -- Confluence page ID
    space_key                TEXT,          -- Confluence space key
    last_modified_at         TIMESTAMPTZ NOT NULL,
    last_modified_by         TEXT,
    version_number           INTEGER DEFAULT 1,
    freshness_threshold_days INTEGER,       -- NULL = global default (90)
    manually_flagged         BOOLEAN NOT NULL DEFAULT FALSE,
    reviewed_at              TIMESTAMPTZ,   -- "still accurate" manual confirmation
    reviewed_by              TEXT,
    created_at               TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at               TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE link_checks (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    article_id      UUID NOT NULL REFERENCES articles(id) ON DELETE CASCADE,
    url             TEXT NOT NULL,
    status_code     INTEGER,        -- NULL if connection failed entirely
    is_broken       BOOLEAN NOT NULL,
    error_message   TEXT,
    checked_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE screenshots (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    article_id      UUID NOT NULL REFERENCES articles(id) ON DELETE CASCADE,
    image_data      BYTEA NOT NULL,
    perceptual_hash TEXT,            -- for visual drift comparison (Phase 3)
    captured_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE scan_runs (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    scan_type          TEXT NOT NULL DEFAULT 'full',  -- full, links_only, screenshots_only
    started_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at       TIMESTAMPTZ,
    articles_scanned   INTEGER DEFAULT 0,
    links_checked      INTEGER DEFAULT 0,
    broken_links_found INTEGER DEFAULT 0,
    status             TEXT NOT NULL DEFAULT 'running',  -- running, completed, failed
    error_message      TEXT
);

CREATE TABLE ticket_patterns (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ticket_category    TEXT NOT NULL,
    ticket_count       INTEGER NOT NULL,
    related_article_id UUID REFERENCES articles(id) ON DELETE SET NULL,
    keywords           JSONB NOT NULL,
    suggested_update   TEXT NOT NULL,
    detected_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes
CREATE INDEX idx_articles_source ON articles(source);
CREATE INDEX idx_articles_last_modified ON articles(last_modified_at);
CREATE INDEX idx_articles_space ON articles(space_key);
CREATE INDEX idx_link_checks_article ON link_checks(article_id, checked_at DESC);
CREATE INDEX idx_link_checks_broken ON link_checks(article_id) WHERE is_broken = TRUE;
CREATE INDEX idx_screenshots_article ON screenshots(article_id, captured_at DESC);
CREATE INDEX idx_scan_runs_started ON scan_runs(started_at DESC);
CREATE INDEX idx_ticket_patterns_article ON ticket_patterns(related_article_id);
