-- Explicit offline schema upgrade; run after a verified backup with app stopped.
-- JSON values (including exact source/prompt strings) are retained losslessly.
BEGIN;
LOCK TABLE ai_chats IN ACCESS EXCLUSIVE MODE;
ALTER TABLE ai_chats ADD COLUMN inputs TEXT;
DO $$ BEGIN
    IF EXISTS (SELECT 1 FROM ai_chats WHERE NOT document::jsonb ?& ARRAY['snapshot','system_prompt','generation_mode','cost_rates','max_output_tokens','limits','review']) THEN
        RAISE EXCEPTION 'AI chat has incomplete pinned inputs; preserve and repair before upgrade';
    END IF;
END $$;
UPDATE ai_chats SET inputs=(SELECT jsonb_object_agg(key,value)::text FROM jsonb_each(document::jsonb) WHERE key=ANY(ARRAY['snapshot','system_prompt','generation_mode','cost_rates','max_output_tokens','limits','review'])),
 document=(document::jsonb - ARRAY['snapshot','system_prompt','generation_mode','cost_rates','max_output_tokens','limits','review'])::text;
ALTER TABLE ai_chats ALTER COLUMN inputs SET NOT NULL;
COMMIT;
