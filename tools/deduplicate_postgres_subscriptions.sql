\set ON_ERROR_STOP on
BEGIN;

LOCK TABLE subscriptions IN ACCESS EXCLUSIVE MODE;
LOCK TABLE library_origins, subscription_sources, subscription_icons,
    subscription_activity, workspace_feed_urls, web_feed_recipes IN SHARE ROW EXCLUSIVE MODE;

CREATE TEMP TABLE subscription_dedup_map ON COMMIT DROP AS
WITH candidates AS (
    SELECT subscription.id,
           subscription.document::jsonb AS document,
           count(origin.article_id) AS article_count,
           coalesce(bool_or(feed_url.subscription_id = subscription.id), false) AS owns_url
    FROM subscriptions AS subscription
    LEFT JOIN library_origins AS origin ON origin.subscription_id = subscription.id
    LEFT JOIN workspace_feed_urls AS feed_url ON feed_url.subscription_id = subscription.id
    GROUP BY subscription.id, subscription.document
), ranked AS (
    SELECT id,
           first_value(id) OVER (
               PARTITION BY document ->> 'workspace_id', document ->> 'source_url'
               ORDER BY owns_url DESC, article_count DESC, id
           ) AS canonical_id,
           count(*) OVER (
               PARTITION BY document ->> 'workspace_id', document ->> 'source_url'
           ) AS copies
    FROM candidates
)
SELECT id AS duplicate_id, canonical_id
FROM ranked
WHERE copies > 1 AND id <> canonical_id;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM subscription_dedup_map AS mapping
        JOIN subscriptions AS duplicate ON duplicate.id = mapping.duplicate_id
        JOIN subscriptions AS canonical ON canonical.id = mapping.canonical_id
        WHERE coalesce(duplicate.document::jsonb ->> 'personal_note', '')
                <> coalesce(canonical.document::jsonb ->> 'personal_note', '')
           OR coalesce(duplicate.document::jsonb ->> 'custom_name', '')
                <> coalesce(canonical.document::jsonb ->> 'custom_name', '')
           OR duplicate.document::jsonb -> 'status' <> canonical.document::jsonb -> 'status'
           OR coalesce(duplicate.document::jsonb -> 'history', '[]'::jsonb)
                <> coalesce(canonical.document::jsonb -> 'history', '[]'::jsonb)
    ) THEN
        RAISE EXCEPTION 'duplicate subscriptions contain conflicting user-owned state';
    END IF;
    IF EXISTS (
        SELECT 1
        FROM subscription_dedup_map AS mapping
        JOIN subscription_sources AS duplicate ON duplicate.subscription_id = mapping.duplicate_id
        JOIN subscription_sources AS canonical ON canonical.subscription_id = mapping.canonical_id
        WHERE duplicate.source_id <> canonical.source_id
    ) THEN
        RAISE EXCEPTION 'duplicate subscriptions point to different physical sources';
    END IF;
    IF EXISTS (
        SELECT 1
        FROM rules AS rule
        JOIN subscription_dedup_map AS mapping
          ON rule.document::jsonb ->> 'subscription_id' = mapping.duplicate_id
    ) THEN
        RAISE EXCEPTION 'duplicate subscriptions have rules that require an explicit merge';
    END IF;
    IF EXISTS (
        SELECT mapping.canonical_id
        FROM subscription_dedup_map AS mapping
        JOIN web_feed_recipes AS recipe
          ON recipe.id IN (mapping.duplicate_id, mapping.canonical_id)
        GROUP BY mapping.canonical_id
        HAVING count(DISTINCT recipe.document) > 1
    ) THEN
        RAISE EXCEPTION 'duplicate subscriptions have different extraction recipes';
    END IF;
END $$;

INSERT INTO library_origins(workspace_id, article_id, subscription_id, source_record_id)
SELECT origin.workspace_id, origin.article_id, mapping.canonical_id, origin.source_record_id
FROM library_origins AS origin
JOIN subscription_dedup_map AS mapping ON mapping.duplicate_id = origin.subscription_id
ON CONFLICT DO NOTHING;
DELETE FROM library_origins AS origin
USING subscription_dedup_map AS mapping
WHERE origin.subscription_id = mapping.duplicate_id;

INSERT INTO subscription_activity(subscription_id, occurred_at_ms, id, document)
SELECT mapping.canonical_id, activity.occurred_at_ms, activity.id, activity.document
FROM subscription_activity AS activity
JOIN subscription_dedup_map AS mapping ON mapping.duplicate_id = activity.subscription_id
ON CONFLICT DO NOTHING;
DELETE FROM subscription_activity AS activity
USING subscription_dedup_map AS mapping
WHERE activity.subscription_id = mapping.duplicate_id;

INSERT INTO subscription_icons(subscription_id, data_url, fetched_at_ms)
SELECT DISTINCT ON (mapping.canonical_id)
       mapping.canonical_id, icon.data_url, icon.fetched_at_ms
FROM subscription_icons AS icon
JOIN subscription_dedup_map AS mapping ON mapping.duplicate_id = icon.subscription_id
ORDER BY mapping.canonical_id, icon.fetched_at_ms DESC
ON CONFLICT(subscription_id) DO UPDATE
SET data_url = CASE
        WHEN EXCLUDED.fetched_at_ms > subscription_icons.fetched_at_ms THEN EXCLUDED.data_url
        ELSE subscription_icons.data_url
    END,
    fetched_at_ms = greatest(subscription_icons.fetched_at_ms, EXCLUDED.fetched_at_ms);
DELETE FROM subscription_icons AS icon
USING subscription_dedup_map AS mapping
WHERE icon.subscription_id = mapping.duplicate_id;

INSERT INTO web_feed_recipes(id, revision, document)
SELECT mapping.canonical_id, recipe.revision, recipe.document
FROM web_feed_recipes AS recipe
JOIN subscription_dedup_map AS mapping ON mapping.duplicate_id = recipe.id
ON CONFLICT(id) DO NOTHING;
DELETE FROM web_feed_recipes AS recipe
USING subscription_dedup_map AS mapping
WHERE recipe.id = mapping.duplicate_id;

UPDATE workspace_feed_urls AS feed_url
SET subscription_id = mapping.canonical_id
FROM subscription_dedup_map AS mapping
WHERE feed_url.subscription_id = mapping.duplicate_id;

DELETE FROM subscription_sources AS source
USING subscription_dedup_map AS mapping
WHERE source.subscription_id = mapping.duplicate_id;
DELETE FROM subscriptions AS subscription
USING subscription_dedup_map AS mapping
WHERE subscription.id = mapping.duplicate_id;

CREATE UNIQUE INDEX IF NOT EXISTS subscriptions_by_workspace_exact_url
ON subscriptions ((document::jsonb ->> 'workspace_id'), (document::jsonb ->> 'source_url'));

COMMIT;
