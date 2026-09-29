-- Offline, with a verified backup. Run psql from tools/.
\set ON_ERROR_STOP on
BEGIN;
DO $$ BEGIN
 IF (SELECT max(version) FROM schema_releases) <> 10 OR NOT EXISTS(SELECT 1 FROM schema_releases WHERE version=10 AND release='wiki-organization-2026-09-29') THEN
  RAISE EXCEPTION 'expected schema release 10';
 END IF;
END $$;
\ir ../crates/reader-storage-postgres/src/schema/ai_reviews.sql
INSERT INTO schema_releases(version,release) VALUES(11,'targeted-ai-review-2026-09-29');
COMMIT;
