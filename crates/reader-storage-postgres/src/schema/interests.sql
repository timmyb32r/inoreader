CREATE TABLE interest_profiles (
 owner UUID PRIMARY KEY, prompt TEXT NOT NULL CHECK(length(btrim(prompt))>0),
 revision BIGINT NOT NULL CHECK(revision>0), training_count BIGINT NOT NULL CHECK(training_count>=0),
 updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE interest_profile_history (
 owner UUID NOT NULL,revision BIGINT NOT NULL,prompt TEXT NOT NULL,training_count BIGINT NOT NULL,
 recorded_at TIMESTAMPTZ NOT NULL DEFAULT now(), PRIMARY KEY(owner,revision)
);
CREATE FUNCTION reader_interest_profile_history() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 INSERT INTO interest_profile_history(owner,revision,prompt,training_count) VALUES(NEW.owner,NEW.revision,NEW.prompt,NEW.training_count);
 RETURN NEW;
END $$;
CREATE TRIGGER interest_profile_history AFTER INSERT OR UPDATE ON interest_profiles FOR EACH ROW EXECUTE FUNCTION reader_interest_profile_history();
CREATE TABLE interest_scores (
 owner UUID NOT NULL, workspace UUID NOT NULL, article UUID NOT NULL,
 profile_revision BIGINT NOT NULL CHECK(profile_revision>0),
 status TEXT NOT NULL CHECK(status IN ('working','scored','failed','budget')),
 score SMALLINT CHECK(score BETWEEN 1 AND 10), prediction TEXT, error TEXT, dirty BOOLEAN NOT NULL DEFAULT false,
 lease UUID NOT NULL, lease_until TIMESTAMPTZ NOT NULL,
 PRIMARY KEY(owner,workspace,article,profile_revision)
);
CREATE INDEX interest_scores_rank ON interest_scores(owner,workspace,profile_revision,score DESC) WHERE status='scored';
CREATE TABLE interest_responses (
 id UUID PRIMARY KEY, owner UUID NOT NULL, workspace UUID NOT NULL, article UUID NOT NULL,
 profile_revision BIGINT NOT NULL, input TEXT NOT NULL,
 response BYTEA, response_status INTEGER, interrupted BOOLEAN,
 created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE FUNCTION reader_interest_content_changed() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='UPDATE' AND OLD.document=NEW.document THEN RETURN NEW; END IF;
 UPDATE interest_scores s SET dirty=true FROM library_origins o WHERE o.source_record_id=NEW.id AND s.workspace::text=o.workspace_id AND s.article::text=o.article_id;
 RETURN NEW;
END $$;
CREATE TRIGGER interest_content_changed AFTER INSERT OR UPDATE ON content_manifests FOR EACH ROW EXECUTE FUNCTION reader_interest_content_changed();
ALTER TABLE ai_spending DROP CONSTRAINT ai_spending_mode_check;
ALTER TABLE ai_spending ADD CONSTRAINT ai_spending_mode_check CHECK(mode IN ('summary','verification','chat','translation','terms','ranking'));
