-- Existing events have no evidence of how they were read. Never infer a rating.
ALTER TABLE article_read_events ADD COLUMN method TEXT NOT NULL DEFAULT 'unknown'
    CHECK(method IN ('reader','single','bulk','unknown'));
ALTER TABLE article_read_events ALTER COLUMN method DROP DEFAULT;
