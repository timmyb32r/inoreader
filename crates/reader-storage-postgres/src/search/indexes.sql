CREATE INDEX search_content_text ON search_content USING gin(text public.gin_trgm_ops) WITH (fastupdate=off);
CREATE INDEX search_article_title ON articles USING gin ((document::jsonb #>> '{key,title}') public.gin_trgm_ops) WITH (fastupdate=off);
CREATE INDEX search_article_description ON articles USING gin ((document::jsonb #>> '{key,description}') public.gin_trgm_ops) WITH (fastupdate=off);
CREATE INDEX search_origins_record ON library_origins(source_record_id,workspace_id,article_id);
