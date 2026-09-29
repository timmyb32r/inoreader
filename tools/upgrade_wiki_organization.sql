-- Run offline after a verified backup; psql working directory must be tools/.
\set ON_ERROR_STOP on
BEGIN;
DO $$ BEGIN
 IF NOT EXISTS(SELECT 1 FROM schema_releases WHERE version=9 AND release='ai-model-preferences-2026-09-29') OR EXISTS(SELECT 1 FROM schema_releases WHERE version>9) THEN
  RAISE EXCEPTION 'expected schema release 9';
 END IF;
END $$;
\ir ../crates/reader-storage-postgres/src/wiki/organization.sql
INSERT INTO schema_releases(version,release) VALUES(10,'wiki-organization-2026-09-29');
COMMIT;
