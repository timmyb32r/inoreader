-- User-selected visibility changes never delete articles, source data or ratings.
CREATE TABLE reading_preferences (
 workspace TEXT PRIMARY KEY REFERENCES workspaces(id),
 skip_duplicates BOOLEAN NOT NULL DEFAULT false,
 hide_snowflake_spanish BOOLEAN NOT NULL DEFAULT false,
 hide_snowflake_french BOOLEAN NOT NULL DEFAULT false,
 hide_reddit_career BOOLEAN NOT NULL DEFAULT false,
 excluded_urls TEXT[] NOT NULL DEFAULT ARRAY[]::text[]
);
CREATE INDEX articles_exact_url_title ON articles(workspace_key,(document::jsonb#>>'{key,location,Url,exact}'),(document::jsonb#>>'{key,title}'),arrival_order,id);
CREATE INDEX articles_exact_title ON articles(workspace_key,(document::jsonb#>>'{key,title}'));
-- The final redirect URL is observation, not an identity rewrite. Two exact
-- titles targeting the same final page are one visible story under this opt-in.
CREATE FUNCTION reader_display_url(article_row articles) RETURNS TEXT LANGUAGE sql STABLE AS $$
 SELECT COALESCE((SELECT m.document::jsonb->>'final_url' FROM library_origins o JOIN content_manifests m ON m.id=o.source_record_id WHERE o.workspace_id=article_row.workspace_key AND o.article_id=article_row.article_key ORDER BY (m.document::jsonb->>'fetched_at')::timestamptz DESC,m.id DESC LIMIT 1),article_row.document::jsonb#>>'{key,location,Url,exact}')
$$;
CREATE FUNCTION reader_article_visible(article_row articles) RETURNS BOOLEAN LANGUAGE sql STABLE AS $$
 SELECT NOT EXISTS (
  SELECT 1 FROM reading_preferences p WHERE p.workspace=article_row.workspace_key AND (
   (article_row.document::jsonb#>>'{key,location,Url,exact}')=ANY(p.excluded_urls)
   OR (p.skip_duplicates AND EXISTS(SELECT 1 FROM articles prior WHERE prior.workspace_key=article_row.workspace_key AND reader_display_url(prior)=reader_display_url(article_row) AND prior.document::jsonb#>>'{key,title}'=article_row.document::jsonb#>>'{key,title}' AND (prior.is_read OR (prior.arrival_order,prior.id)<(article_row.arrival_order,article_row.id)) AND prior.id<>article_row.id))
   OR (p.hide_snowflake_spanish AND (article_row.document::jsonb#>>'{key,location,Url,exact}') ~ '^https://(www\.)?snowflake\.com/(content/snowflake-site/global/)?(es|es-la|es-es)(/|$)')
   OR (p.hide_snowflake_french AND (article_row.document::jsonb#>>'{key,location,Url,exact}') ~ '^https://(www\.)?snowflake\.com/(content/snowflake-site/global/)?fr(/|$)')
   OR (p.hide_reddit_career AND (article_row.document::jsonb#>>'{key,location,Url,exact}') ~ '^https://(www\.|old\.)?reddit\.com/' AND EXISTS(SELECT 1 FROM library_origins o JOIN source_records r ON r.id=o.source_record_id LEFT JOIN content_manifests m ON m.id=o.source_record_id WHERE o.workspace_id=article_row.workspace_key AND o.article_id=article_row.article_key AND (m.document::jsonb->>'reddit_flair'='Career' OR (r.document::jsonb->'categories') ? 'Career')))
  )
 )
$$;

-- Durable per-source failures prevent one oversized article starving the stream.
-- A new source revision or prompt version deliberately permits a new attempt.
-- Audit lifetime is independent of credentials: removing an API key must work
-- without deleting diagnostic history. Repository writes enforce ownership.
CREATE TABLE ai_terms_failures (
 owner UUID NOT NULL,
 workspace UUID NOT NULL,
 article UUID NOT NULL,
 source_revision TEXT NOT NULL,
 prompt_version TEXT NOT NULL,
 reason TEXT NOT NULL,
 created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 PRIMARY KEY(owner,workspace,article,source_revision,prompt_version)
);
