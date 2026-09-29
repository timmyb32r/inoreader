CREATE TABLE ai_spending (
 id UUID PRIMARY KEY, owner UUID NOT NULL, mode TEXT NOT NULL CHECK(mode IN ('summary','verification','chat','translation','terms')),
 created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 day DATE NOT NULL DEFAULT (clock_timestamp() AT TIME ZONE 'Europe/Moscow')::date,
 reserved NUMERIC NOT NULL CHECK(reserved>0), actual NUMERIC CHECK(actual>=0)
);
CREATE INDEX ai_spending_owner_day ON ai_spending(owner,day);
ALTER TABLE ai_chats ADD COLUMN priority_at TIMESTAMPTZ;
CREATE TABLE ai_automatic_accounts(owner UUID PRIMARY KEY, enrolled_at TIMESTAMPTZ NOT NULL DEFAULT now());
CREATE TABLE ai_summary_queue(
 owner UUID NOT NULL REFERENCES ai_automatic_accounts(owner),workspace UUID NOT NULL,article UUID NOT NULL,
 attempts INTEGER NOT NULL DEFAULT 0 CHECK(attempts>=0),scheduled_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 dispatched BOOLEAN NOT NULL DEFAULT false,error TEXT,
 PRIMARY KEY(owner,workspace,article)
);
CREATE INDEX ai_summary_queue_pending ON ai_summary_queue(scheduled_at) WHERE NOT dispatched;
-- Candidate creation is transactional with delivery. Existing read articles are
-- eligible only when a full-text manifest is first published after enrollment.
CREATE FUNCTION reader_ai_article_candidate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NOT EXISTS(SELECT 1 FROM workspaces w JOIN ai_automatic_accounts p ON w.document::jsonb->>'owner'=p.owner::text WHERE w.id=NEW.workspace_key) THEN RETURN NEW; END IF;
 INSERT INTO ai_summary_queue(owner,workspace,article)
 SELECT p.owner,NEW.workspace_key::uuid,NEW.article_key::uuid FROM workspaces w
 JOIN ai_automatic_accounts p ON w.document::jsonb->>'owner'=p.owner::text
 WHERE w.id=NEW.workspace_key ON CONFLICT DO NOTHING;
 RETURN NEW;
END $$;
CREATE TRIGGER ai_new_article AFTER INSERT ON articles FOR EACH ROW EXECUTE FUNCTION reader_ai_article_candidate();
CREATE FUNCTION reader_ai_content_candidate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 INSERT INTO ai_summary_queue(owner,workspace,article)
 SELECT p.owner,o.workspace_id::uuid,o.article_id::uuid FROM library_origins o
 JOIN workspaces w ON w.id=o.workspace_id
 JOIN ai_automatic_accounts p ON w.document::jsonb->>'owner'=p.owner::text
 WHERE o.source_record_id=NEW.id ON CONFLICT DO NOTHING;
 RETURN NEW;
END $$;
CREATE TRIGGER ai_new_fulltext AFTER INSERT ON content_manifests FOR EACH ROW EXECUTE FUNCTION reader_ai_content_candidate();
