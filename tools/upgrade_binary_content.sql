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
CREATE FUNCTION pg_temp.reader_exact_chunk(value TEXT) RETURNS BYTEA
LANGUAGE plpgsql IMMUTABLE STRICT AS $$
DECLARE parsed JSON; hex TEXT;
BEGIN
    parsed := value::json;
    IF json_typeof(parsed) <> 'array' THEN
        RAISE EXCEPTION 'invalid content chunk: expected a byte array';
    END IF;
    IF EXISTS (SELECT 1 FROM json_array_elements(parsed) AS a(item)
        WHERE CASE WHEN json_typeof(item)='number' AND item::text ~ '^(0|[1-9][0-9]{0,2})$'
            THEN item::text::integer > 255 ELSE true END) THEN
        RAISE EXCEPTION 'invalid content chunk: expected integer bytes in 0..255';
    END IF;
    SELECT string_agg(lpad(to_hex(item::text::integer),2,'0'),'' ORDER BY ordinal)
        INTO hex FROM json_array_elements(parsed) WITH ORDINALITY AS a(item,ordinal);
    RETURN decode(COALESCE(hex,''),'hex');
END $$;
ALTER TABLE staged_content_chunks ALTER COLUMN bytes TYPE BYTEA USING pg_temp.reader_exact_chunk(bytes);
CREATE TABLE schema_releases (
    version BIGINT PRIMARY KEY CHECK(version>0),
    release TEXT NOT NULL,
    applied_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

