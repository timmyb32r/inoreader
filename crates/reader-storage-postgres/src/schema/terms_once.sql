-- One automatic attempt per owned article/prompt version. Technical refreshes
-- never reset admission; retries/regeneration remain explicit manual work.
CREATE INDEX ai_definitions_article_prompt ON ai_definitions(owner,workspace,article,created_at,id);
CREATE INDEX ai_terms_failures_article_prompt ON ai_terms_failures(owner,workspace,article,prompt_version);
-- Pending legacy duplicates are superseded by a terminal/active attempt or
-- the oldest queued attempt. Manual requests are never suppressed. Malformed
-- retained records fail closed without poisoning unrelated queue claims.
CREATE FUNCTION reader_ai_terms_superseded(job UUID) RETURNS BOOLEAN LANGUAGE sql STABLE AS $$
 SELECT COALESCE((SELECT CASE WHEN current.automatic THEN
  CASE WHEN NOT pg_input_is_valid(current.document,'jsonb') THEN TRUE ELSE EXISTS(
   SELECT 1 FROM ai_definitions previous
   WHERE previous.owner=current.owner AND previous.workspace=current.workspace
    AND previous.article=current.article AND previous.id<>current.id AND previous.status<>'cancelled'
    AND CASE WHEN previous.status='quarantined' OR NOT pg_input_is_valid(previous.document,'jsonb') THEN TRUE ELSE
     previous.document::jsonb#>>'{job,promptVersion}'=current.document::jsonb#>>'{job,promptVersion}'
     AND (previous.status<>'queued' OR (previous.created_at,previous.id)<(current.created_at,current.id))
    END
  ) END ELSE FALSE END FROM ai_definitions current WHERE current.id=job),FALSE)
$$;
