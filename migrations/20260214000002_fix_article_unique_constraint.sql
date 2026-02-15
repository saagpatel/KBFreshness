-- Remove UNIQUE constraint from url column
-- Add unique constraint on (source, source_id) instead
-- This allows different sources to reference the same URL

-- Drop the existing UNIQUE constraint on url
ALTER TABLE articles DROP CONSTRAINT IF EXISTS articles_url_key;

-- Add unique constraint on (source, source_id) to prevent duplicate imports
-- This ensures we don't import the same Confluence page twice, etc.
CREATE UNIQUE INDEX articles_source_source_id_key ON articles(source, source_id)
WHERE source_id IS NOT NULL;
