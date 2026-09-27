-- Explicit deployment step for the schema before architecture-consistency.
-- Stop the application and verify a restorable backup first. The articles table
-- is authoritative; no article, source bytes, user state or identity is changed.
-- Retain the backup of obsolete derived snapshots for forensic comparison.
BEGIN;
LOCK TABLE articles, library_dedup IN ACCESS EXCLUSIVE MODE;
DO $$ BEGIN
    IF EXISTS (
        SELECT 1 FROM library_dedup d LEFT JOIN articles a
          ON a.id=d.workspace_id || '/' || d.article_id
        WHERE a.id IS NULL OR a.document::jsonb->'key' <> d.dedup_key::jsonb
    ) THEN
        RAISE EXCEPTION 'Dedup index has missing or inconsistent article identities; repair explicitly before upgrade';
    END IF;
END $$;
ALTER TABLE library_dedup DROP COLUMN revision, DROP COLUMN document;
COMMIT;
