ALTER TABLE source_health ADD COLUMN failure_since_ms BIGINT;
-- Recover the start from retained failed attempts after the last success.
-- If history is unavailable, the latest recorded failure is the earliest
-- provable observation; never invent an outage start from creation time.
UPDATE source_health h SET failure_since_ms = COALESCE(
    (SELECT min(a.occurred_at_ms) FROM subscription_sources s
     JOIN subscription_activity a ON a.subscription_id=s.subscription_id
     WHERE s.source_id=h.source_id
       AND (a.document::jsonb->>'successful')::boolean=false
       AND a.occurred_at_ms > COALESCE((h.document::jsonb->>'last_success_ms')::bigint, '-9223372036854775808'::bigint)),
    (h.document::jsonb->>'last_error_ms')::bigint)
WHERE (h.document::jsonb->>'consecutive_failures')::bigint>0;
