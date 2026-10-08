-- One explicit deployment default, copied into each owned subscription. Existing
-- selections never change when the default is edited.
CREATE TABLE ai_bootstrap_policy (
 singleton BOOLEAN PRIMARY KEY CHECK(singleton),
 initial_articles INTEGER NOT NULL CHECK(initial_articles>0)
);
INSERT INTO ai_bootstrap_policy VALUES(true,10);
-- Immutable discovery projection, never sourced from mutable observed_at_ms.
CREATE TABLE ai_record_discovery (
 record TEXT PRIMARY KEY, source TEXT NOT NULL,
 sequence BIGINT GENERATED ALWAYS AS IDENTITY UNIQUE,
 publication_order NUMERIC,
 source_order BIGINT NOT NULL CHECK(source_order>=0)
);
CREATE INDEX ai_record_discovery_source ON ai_record_discovery(source,sequence);
CREATE INDEX ai_record_discovery_latest ON ai_record_discovery(source,publication_order DESC NULLS LAST,source_order,sequence);
CREATE TABLE subscription_ai_bootstrap (
 subscription TEXT NOT NULL, owner UUID NOT NULL, workspace TEXT NOT NULL,
 source TEXT NOT NULL, initial_articles INTEGER NOT NULL CHECK(initial_articles>0),
 cutoff BIGINT CHECK(cutoff>=0), ready BOOLEAN NOT NULL DEFAULT false,
 PRIMARY KEY(subscription,source),
 CHECK(NOT ready OR cutoff IS NOT NULL)
);
CREATE INDEX subscription_ai_bootstrap_source ON subscription_ai_bootstrap(source) WHERE NOT ready;
CREATE TABLE subscription_ai_initial_records (
 subscription TEXT NOT NULL, source TEXT NOT NULL,
 record TEXT NOT NULL REFERENCES ai_record_discovery(record),
 PRIMARY KEY(subscription,source,record),
 FOREIGN KEY(subscription,source) REFERENCES subscription_ai_bootstrap(subscription,source)
);
-- One representative record per selected library article. Keeping every duplicate
-- origin would permit a later regroup to expand the initial article allowance.
CREATE TABLE subscription_ai_selected (
 subscription TEXT NOT NULL, source TEXT NOT NULL,
 record TEXT NOT NULL REFERENCES ai_record_discovery(record),
 PRIMARY KEY(subscription,source,record),
 FOREIGN KEY(subscription,source) REFERENCES subscription_ai_bootstrap(subscription,source)
);
CREATE FUNCTION reader_ai_finalize_subscription(sub TEXT, src TEXT) RETURNS VOID LANGUAGE plpgsql AS $$
DECLARE b subscription_ai_bootstrap;
BEGIN
 SELECT * INTO b FROM subscription_ai_bootstrap WHERE subscription=sub AND source=src FOR UPDATE;
 IF NOT FOUND OR b.ready OR b.cutoff IS NULL THEN RETURN; END IF;
 IF NOT EXISTS(SELECT 1 FROM subscriptions s JOIN subscription_sources link ON link.subscription_id=s.id AND link.source_id=b.source JOIN workspaces w ON w.id=b.workspace
   WHERE s.id=sub AND s.document::jsonb->>'workspace_id'=w.id
   AND w.document::jsonb->>'owner'=b.owner::text) THEN RETURN; END IF;
 IF EXISTS(SELECT 1 FROM subscription_ai_initial_records r WHERE r.subscription=sub AND r.source=src
   AND NOT EXISTS(SELECT 1 FROM library_origins o WHERE o.workspace_id=b.workspace
     AND o.subscription_id=sub AND o.source_record_id=r.record)) THEN RETURN; END IF;
 INSERT INTO subscription_ai_selected(subscription,source,record)
 SELECT sub,src,record FROM (
   SELECT DISTINCT ON(a.article_key) a.article_key,d.record,d.publication_order,d.source_order,d.sequence
   FROM subscription_ai_initial_records r JOIN ai_record_discovery d ON d.record=r.record
   JOIN library_origins o ON o.subscription_id=sub AND o.source_record_id=r.record AND o.workspace_id=b.workspace
   JOIN articles a ON a.id=o.workspace_id||'/'||o.article_id
   WHERE r.subscription=sub AND r.source=src AND reader_article_visible(a)
   ORDER BY a.article_key,d.publication_order DESC NULLS LAST,d.source_order,d.sequence
 ) candidates ORDER BY publication_order DESC NULLS LAST,source_order,sequence
 LIMIT b.initial_articles;
 UPDATE subscription_ai_bootstrap SET ready=true WHERE subscription=sub AND source=src;
 -- Full text can precede delivery or selection. Queue the frozen selection here,
 -- rather than relying on a future content INSERT to reawaken the article.
 INSERT INTO ai_summary_queue(owner,workspace,article)
 SELECT b.owner,b.workspace::uuid,o.article_id::uuid
 FROM library_origins o JOIN ai_automatic_accounts p ON p.owner=b.owner
 WHERE o.subscription_id=sub AND o.workspace_id=b.workspace
 AND reader_ai_automatic_allowed(b.owner,b.workspace,o.article_id)
 ON CONFLICT DO NOTHING;
END $$;
CREATE FUNCTION reader_ai_automatic_allowed(account UUID, ws TEXT, article TEXT)
RETURNS BOOLEAN LANGUAGE sql STABLE AS $$
 SELECT EXISTS(
  SELECT 1 FROM workspaces w JOIN articles a ON a.workspace_key=w.id
  JOIN library_origins o ON o.workspace_id=w.id AND o.article_id=a.article_key
  JOIN subscriptions s ON s.id=o.subscription_id AND s.document::jsonb->>'workspace_id'=w.id
  JOIN subscription_sources link ON link.subscription_id=s.id
  JOIN subscription_ai_bootstrap b ON b.subscription=s.id AND b.source=link.source_id AND b.owner=account AND b.workspace=w.id
  JOIN ai_record_discovery d ON d.record=o.source_record_id AND d.source=b.source
  WHERE w.id=ws AND w.document::jsonb->>'owner'=account::text AND a.id=ws||'/'||article
    AND b.ready AND reader_article_visible(a) AND
    (d.sequence>b.cutoff OR EXISTS(SELECT 1 FROM subscription_ai_selected selected
       WHERE selected.subscription=b.subscription AND selected.source=b.source AND selected.record=d.record))
 )
$$;
DROP TRIGGER ai_new_article ON articles;
DROP TRIGGER ai_new_fulltext ON content_manifests;
DROP FUNCTION reader_ai_article_candidate();
DROP FUNCTION reader_ai_content_candidate();
-- Origins, unlike article INSERT, already carry verified subscription ownership.
CREATE FUNCTION reader_ai_origin_candidate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 PERFORM reader_ai_finalize_subscription(NEW.subscription_id,(SELECT source_id FROM subscription_sources WHERE subscription_id=NEW.subscription_id));
 IF NOT EXISTS(SELECT 1 FROM workspaces w JOIN ai_automatic_accounts p ON w.document::jsonb->>'owner'=p.owner::text WHERE w.id=NEW.workspace_id AND reader_ai_automatic_allowed(p.owner,NEW.workspace_id,NEW.article_id)) THEN RETURN NEW; END IF;
 INSERT INTO ai_summary_queue(owner,workspace,article)
 SELECT p.owner,NEW.workspace_id::uuid,NEW.article_id::uuid FROM workspaces w JOIN ai_automatic_accounts p ON w.document::jsonb->>'owner'=p.owner::text
 WHERE w.id=NEW.workspace_id AND reader_ai_automatic_allowed(p.owner,NEW.workspace_id,NEW.article_id)
 ON CONFLICT DO NOTHING;
 RETURN NEW;
END $$;
CREATE TRIGGER ai_new_origin AFTER INSERT ON library_origins FOR EACH ROW EXECUTE FUNCTION reader_ai_origin_candidate();
ALTER TABLE ai_chats ADD COLUMN automatic BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE ai_definitions ADD COLUMN automatic BOOLEAN NOT NULL DEFAULT false;
CREATE INDEX ai_definitions_active_lease ON ai_definitions(owner,lease) WHERE lease IS NOT NULL;
CREATE INDEX interest_scores_active_lease ON interest_scores(owner,lease) WHERE lease IS NOT NULL;
-- Do not delete retained jobs or paid results. Only pending, unpromoted summaries
-- dispatched by the automatic queue can be identified as automatic retrospectively.
UPDATE ai_chats c SET automatic=true WHERE c.priority_at IS NULL AND c.status IN ('waiting_content','queued')
 AND EXISTS(SELECT 1 FROM ai_summary_queue q WHERE q.owner=c.owner AND q.workspace=c.workspace AND q.article=c.article AND q.dispatched);

-- Initial extraction previously had no manual/automatic discriminator. Explicit
-- regeneration operations remain manual; pending first-pass archive jobs adopt
-- the new admission policy. Their records and operations remain retained.
UPDATE ai_definitions d SET automatic=true WHERE d.status='queued'
 AND NOT EXISTS(SELECT 1 FROM ai_definition_operations o WHERE o.owner=d.owner AND o.job=d.id AND o.regenerate);
