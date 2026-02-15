-- Add default value for freshness_threshold_days
-- This allows articles to inherit the global default (90 days) unless overridden

ALTER TABLE articles ALTER COLUMN freshness_threshold_days SET DEFAULT 90;
