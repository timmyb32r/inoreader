WITH candidates AS (
 SELECT id FROM articles WHERE $3 IN ('all','news') AND document::jsonb #>> '{key,title}' ILIKE $2 ESCAPE '\'
 UNION SELECT id FROM articles WHERE $3 IN ('all','news') AND document::jsonb #>> '{key,description}' ILIKE $2 ESCAPE '\'
 UNION SELECT o.workspace_id||'/'||o.article_id FROM search_content c JOIN library_origins o ON o.source_record_id=c.record_id WHERE $3 IN ('all','news') AND c.text ILIKE $2 ESCAPE '\'
), hits AS (
 SELECT 'news' AS kind, a.workspace_key AS container, a.article_key AS id,
 a.document::jsonb #>> '{key,title}' AS title, w.document::jsonb->>'name' AS context,
 coalesce(a.document::jsonb #>> '{key,description}','') AS text,
 a.document::jsonb->>'first_arrived_at' AS updated_at
 FROM candidates c JOIN articles a ON a.id=c.id JOIN workspaces w ON w.id=a.workspace_key
 WHERE $3 IN ('all','news') AND w.document::jsonb->>'owner'=$1
 AND ($5::text IS NULL OR EXISTS(SELECT 1 FROM library_origins o WHERE o.workspace_id=a.workspace_key AND o.article_id=a.article_key AND o.subscription_id=$5))
 UNION ALL
 SELECT 'wiki',p.namespace::text,p.id::text,p.name,n.name,p.markdown,p.updated_at::text
 FROM wiki_pages p JOIN wiki_namespaces n ON n.id=p.namespace
 WHERE $3 IN ('all','wiki') AND NOT p.deleted AND ($4::uuid IS NULL OR p.namespace=$4)
 AND (n.owner=$1 OR EXISTS(SELECT 1 FROM wiki_members m WHERE m.namespace=n.id AND m.account=$1))
 AND (p.name ILIKE $2 ESCAPE '\' OR p.markdown ILIKE $2 ESCAPE '\')
), chosen AS MATERIALIZED (
 SELECT *, CASE WHEN lower(title)=lower($6) THEN 0 WHEN title ILIKE $2 ESCAPE '\' THEN 1 ELSE 2 END AS rank
 FROM hits ORDER BY rank,updated_at DESC,kind,container,id LIMIT $8 OFFSET $9
), excerpts AS (
 SELECT h.kind,h.container,h.id,h.title,h.context,h.updated_at,h.rank,coalesce(body.text,h.text) AS text
 FROM chosen h
 LEFT JOIN LATERAL (SELECT sc.text FROM library_origins o JOIN search_content sc ON sc.record_id=o.source_record_id
 WHERE h.kind='news' AND o.workspace_id=h.container AND o.article_id=h.id AND sc.text ILIKE $2 ESCAPE '\' ORDER BY o.source_record_id LIMIT 1) body ON true
), positioned AS (
 SELECT *,greatest(1,strpos(lower(text),lower($6))-$7::integer/3) AS excerpt_start FROM excerpts
)
SELECT kind,container,id,title,context,updated_at,
 CASE WHEN excerpt_start>1 THEN '…' ELSE '' END || substring(text FROM excerpt_start FOR $7::integer) || CASE WHEN length(text)>=excerpt_start+$7::integer THEN '…' ELSE '' END AS excerpt
FROM positioned ORDER BY rank,updated_at DESC,kind,container,id
