-- Explicit per-workspace opt-in. NULL disables age-based reading; days are
-- calendar dates using PublicationDate::day, strictly older than the threshold.
ALTER TABLE reading_preferences ADD COLUMN IF NOT EXISTS auto_read_after_days INTEGER CHECK(auto_read_after_days>0);
ALTER TABLE article_read_events DROP CONSTRAINT IF EXISTS article_read_events_method_check;
ALTER TABLE article_read_events ADD CONSTRAINT article_read_events_method_check CHECK(method IN ('reader','single','bulk','unknown','automatic_age'));
CREATE OR REPLACE FUNCTION reader_ai_automatic_allowed(account UUID, ws TEXT, article TEXT)
RETURNS BOOLEAN LANGUAGE sql STABLE AS $$
 SELECT EXISTS(
  SELECT 1 FROM workspaces w JOIN articles a ON a.workspace_key=w.id
  JOIN library_origins o ON o.workspace_id=w.id AND o.article_id=a.article_key
  JOIN subscriptions s ON s.id=o.subscription_id AND s.document::jsonb->>'workspace_id'=w.id
  JOIN subscription_sources link ON link.subscription_id=s.id
  JOIN subscription_ai_bootstrap b ON b.subscription=s.id AND b.source=link.source_id AND b.owner=account AND b.workspace=w.id
  JOIN ai_record_discovery d ON d.record=o.source_record_id AND d.source=b.source
  WHERE w.id=ws AND w.document::jsonb->>'owner'=account::text AND a.id=ws||'/'||article
    AND b.ready AND reader_article_visible(a)
    AND NOT (a.is_read AND EXISTS(SELECT 1 FROM article_read_events e WHERE e.workspace_id=w.id AND e.article_id=a.article_key AND e.method='automatic_age' AND e.revision=(SELECT max(latest.revision) FROM article_read_events latest WHERE latest.workspace_id=w.id AND latest.article_id=a.article_key))) AND
    (d.sequence>b.cutoff OR EXISTS(SELECT 1 FROM subscription_ai_selected selected
       WHERE selected.subscription=b.subscription AND selected.source=b.source AND selected.record=d.record))
 )
$$;
