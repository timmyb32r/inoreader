-- Explicit offline upgrade from the preceding unversioned release.
-- Executed inside upgrade-schema transaction; never run standalone.
-- Stop the application and verify a restorable backup first.
-- Invalid bytes abort the transaction. No source records or values are dropped.
SELECT pg_advisory_xact_lock(492871020);
LOCK TABLE staged_content_chunks, ai_chats IN ACCESS EXCLUSIVE MODE;
DO $$ BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.tables WHERE table_schema=current_schema() AND table_name='schema_releases') THEN
        RAISE EXCEPTION 'database already versioned; refusing an ambiguous upgrade';
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_schema=current_schema() AND table_name='ai_chats' AND column_name='inputs' AND data_type='text') THEN
        RAISE EXCEPTION 'unexpected preceding schema: AI pinned inputs are missing';
    END IF;
END $$;
-- Native Rust validates and converts keyset batches within this transaction.
CREATE TABLE schema_releases (
    version BIGINT PRIMARY KEY CHECK(version>0),
    release TEXT NOT NULL,
    applied_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

