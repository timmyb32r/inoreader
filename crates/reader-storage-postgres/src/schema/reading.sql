-- Observed explicit unread -> read transitions. No invented historical backfill.
-- Deliberately independent of article lifetime; deleting source data cannot erase activity.
CREATE TABLE article_read_events (
    workspace_id TEXT NOT NULL,
    article_id TEXT NOT NULL,
    revision BIGINT NOT NULL CHECK (revision >= 0),
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT statement_timestamp(),
    PRIMARY KEY (workspace_id, article_id, revision)
);
CREATE INDEX article_read_events_by_workspace_time ON article_read_events(workspace_id, occurred_at);
