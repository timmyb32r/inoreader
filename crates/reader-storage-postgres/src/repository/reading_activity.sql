-- One snapshot, one timezone and one day range for both series.
WITH bounds AS (
    SELECT (((now() AT TIME ZONE $2)::date - 363)::timestamp AT TIME ZONE $2) AS start_at,
           (((now() AT TIME ZONE $2)::date + 1)::timestamp AT TIME ZONE $2) AS end_at
), arrivals AS MATERIALIZED (
    SELECT document::jsonb ->> 'first_arrived_at' AS arrived_at
    FROM articles, bounds
    WHERE workspace_key=$1
      AND arrival_order >= reader_arrival_order(to_char(start_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"'))
      AND arrival_order < reader_arrival_order(to_char(end_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"'))
), arrived_days AS (
    -- Calendar grouping needs whole seconds only. PostgreSQL's timestamp cast of
    -- nanoseconds would round 23:59:59.999999999 into the next day. This projection
    -- floors fractional seconds BEFORE the cast; exact source timestamps stay intact.
    SELECT ((substring(arrived_at FROM 1 FOR 19)::timestamp AT TIME ZONE 'UTC') AT TIME ZONE $2)::date::text AS day,
           count(*) AS arrived
    FROM arrivals GROUP BY 1
), read_days AS (
    SELECT (occurred_at AT TIME ZONE $2)::date::text AS day,
           count(DISTINCT article_id) AS count
    FROM article_read_events e
    JOIN articles a ON a.workspace_key=e.workspace_id AND a.article_key=e.article_id AND a.is_read
    CROSS JOIN bounds
    WHERE e.workspace_id=$1 AND occurred_at >= start_at AND occurred_at < end_at
    GROUP BY 1
)
SELECT COALESCE(r.day,a.day), COALESCE(r.count,0), COALESCE(a.arrived,0)
FROM read_days r FULL OUTER JOIN arrived_days a ON a.day=r.day
ORDER BY 1
