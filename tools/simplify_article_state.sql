-- Explicit one-time conversion authorized by the user: Saved -> Read later;
-- Trash -> read, retained articles. Stop the app and back up both tables first.
-- Fail closed if deletion rules exist: their semantics need a separate decision.
BEGIN;
LOCK TABLE articles, library_dedup, rules IN SHARE ROW EXCLUSIVE MODE;
DO $$ BEGIN
    IF EXISTS (SELECT 1 FROM rules WHERE document::jsonb->>'action'='MoveToTrash') THEN
        RAISE EXCEPTION 'Deletion rules exist; decide their replacement before proceeding';
    END IF;
END $$;
CREATE TEMP TABLE article_inventory ON COMMIT DROP AS
SELECT (SELECT COUNT(*) FROM articles) AS articles,
       (SELECT COUNT(*) FROM library_dedup) AS dedup;

UPDATE articles SET
    document = jsonb_set(jsonb_set(document::jsonb, '{state}',
        ((document::jsonb->'state') - 'saved' - 'trashed' - 'protect_restored') ||
        jsonb_build_object(
            'read', (document::jsonb #>> '{state,read}')::boolean OR COALESCE((document::jsonb #>> '{state,trashed}')::boolean,false),
            'later', (document::jsonb #>> '{state,later}')::boolean OR COALESCE((document::jsonb #>> '{state,saved}')::boolean,false)
        )), '{revision}', to_jsonb(revision+1))::text,
    revision = revision+1
WHERE (document::jsonb->'state') ?| ARRAY['saved','trashed','protect_restored'];

DO $$ BEGIN
    IF (SELECT articles FROM article_inventory) <> (SELECT COUNT(*) FROM articles)
       OR (SELECT dedup FROM article_inventory) <> (SELECT COUNT(*) FROM library_dedup) THEN
        RAISE EXCEPTION 'Article count changed during state conversion';
    END IF;
END $$;
COMMIT;
