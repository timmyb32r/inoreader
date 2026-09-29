-- One snapshot for unique arrivals and per-source attribution. Multiple source
-- records from one subscription still contribute only once per article.
WITH bounds AS (
 SELECT ($3::date::timestamp AT TIME ZONE $2) AS start_at,
        (($4::date + 1)::timestamp AT TIME ZONE $2) AS end_at
), arrivals AS MATERIALIZED (
 SELECT article_key,
   ((substring(document::jsonb->>'first_arrived_at' FROM 1 FOR 19)::timestamp AT TIME ZONE 'UTC') AT TIME ZONE $2)::date::text AS day
 FROM articles, bounds
 WHERE workspace_key=$1
   AND arrival_order >= reader_arrival_order(to_char(start_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"'))
   AND arrival_order < reader_arrival_order(to_char(end_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"'))
), totals AS (
 SELECT day, count(*) AS total FROM arrivals GROUP BY day
), attribution AS (
 SELECT DISTINCT a.day, a.article_key, o.subscription_id
 FROM arrivals a LEFT JOIN library_origins o ON o.workspace_id=$1 AND o.article_id=a.article_key
)
SELECT a.day, t.total, a.subscription_id,
       CASE WHEN a.subscription_id IS NULL THEN 'Unattributed' ELSE COALESCE(s.document::jsonb->>'custom_name',s.document::jsonb->>'title') END,
       COALESCE(s.present,false), count(*)
FROM attribution a JOIN totals t ON t.day=a.day
LEFT JOIN article_subscription_provenance s ON s.id=a.subscription_id
GROUP BY a.day,t.total,a.subscription_id,s.document,s.present
ORDER BY a.day,a.subscription_id
