WITH counts AS (
                   SELECT origin.subscription_id,
                          COUNT(DISTINCT origin.article_id)::bigint AS article_count,
                          COUNT(DISTINCT origin.article_id) FILTER (
                              WHERE NOT COALESCE(article.is_read, false)
                          )::bigint AS unread_count
                   FROM library_origins origin
                   LEFT JOIN articles article ON article.id=$2 || '/' || origin.article_id
                   WHERE origin.workspace_id=$2 AND origin.subscription_id=ANY($1::text[])
                   GROUP BY origin.subscription_id
               )
               SELECT requested.subscription_id,
                      health.document,
                      COALESCE((health.document::jsonb->>'consecutive_failures')::bigint > 0
                        AND (health.document::jsonb->>'last_error_ms')::bigint::numeric - health.failure_since_ms::numeric >= $3::bigint, false),
                      source.document,
                      recipe.id IS NOT NULL,
                      icon.data_url,
                      COALESCE(counts.article_count,0)::bigint,
                      COALESCE(counts.unread_count,0)::bigint
               FROM unnest($1::text[]) AS requested(subscription_id)
               LEFT JOIN subscription_sources mapping ON mapping.subscription_id=requested.subscription_id
               LEFT JOIN sources source ON source.id=mapping.source_id
               LEFT JOIN source_health health ON health.source_id=mapping.source_id
               LEFT JOIN web_feed_recipes recipe ON recipe.id=requested.subscription_id
               LEFT JOIN subscription_icons icon ON icon.subscription_id=requested.subscription_id
               LEFT JOIN counts ON counts.subscription_id=requested.subscription_id
