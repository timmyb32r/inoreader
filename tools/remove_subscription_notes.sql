-- Explicit offline retirement, after the owner has approved and completed a
-- note-to-wiki transfer. Stop all application writers first. Take a backup.
-- Refuse to remove even one nonempty note without an exact, owner-private wiki
-- copy bound to that subscription. No fuzzy matching or whitespace normalization.
BEGIN;
LOCK TABLE subscriptions, removed_subscriptions, subscription_wiki_links,
    wiki_pages, wiki_namespaces, wiki_members, workspaces IN ACCESS EXCLUSIVE MODE;
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM article_subscription_provenance s
        WHERE s.document::jsonb ? 'personal_note'
          AND jsonb_typeof(s.document::jsonb->'personal_note') IS DISTINCT FROM 'string'
    ) THEN
        RAISE EXCEPTION 'invalid subscription note type; retirement aborted';
    END IF;
    IF EXISTS (
        SELECT 1 FROM article_subscription_provenance s
        WHERE coalesce(s.document::jsonb->>'personal_note','') <> ''
          AND NOT EXISTS (
            SELECT 1 FROM subscription_wiki_links l
            JOIN wiki_pages p ON (p.namespace,p.id)=(l.namespace,l.page)
            JOIN wiki_namespaces n ON n.id=p.namespace
            JOIN workspaces w ON w.id=s.document::jsonb->>'workspace_id'
            WHERE l.subscription_id=s.id AND NOT p.deleted
              AND p.markdown=s.document::jsonb->>'personal_note'
              AND n.owner=w.document::jsonb->>'owner' AND l.owner=n.owner
              AND NOT EXISTS (SELECT 1 FROM wiki_members m WHERE m.namespace=n.id)
          )
    ) THEN
        RAISE EXCEPTION 'subscription note has no exact private wiki copy; retirement aborted';
    END IF;
END $$;
UPDATE subscriptions SET document=(document::jsonb-'personal_note')::text
WHERE document::jsonb ? 'personal_note';
UPDATE removed_subscriptions SET document=(document::jsonb-'personal_note')::text
WHERE document::jsonb ? 'personal_note';
COMMIT;
