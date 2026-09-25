CREATE TABLE IF NOT EXISTS workspaces (id Utf8 NOT NULL, revision Uint64 NOT NULL, document Utf8 NOT NULL, PRIMARY KEY (id));
CREATE TABLE IF NOT EXISTS subscriptions (id Utf8 NOT NULL, revision Uint64 NOT NULL, document Utf8 NOT NULL, PRIMARY KEY (id));
CREATE TABLE IF NOT EXISTS articles (id Utf8 NOT NULL, revision Uint64 NOT NULL, document Utf8 NOT NULL, PRIMARY KEY (id));
CREATE TABLE IF NOT EXISTS rules (id Utf8 NOT NULL, revision Uint64 NOT NULL, document Utf8 NOT NULL, PRIMARY KEY (id));
CREATE TABLE IF NOT EXISTS content_manifests (id Utf8 NOT NULL, revision Uint64 NOT NULL, document Utf8 NOT NULL, PRIMARY KEY (id));
CREATE TABLE IF NOT EXISTS staged_content_chunks (record_id Utf8 NOT NULL, refresh_id Utf8 NOT NULL, representation Utf8 NOT NULL, ordinal Uint32 NOT NULL, bytes Utf8 NOT NULL, PRIMARY KEY (record_id, refresh_id, representation, ordinal));
CREATE TABLE IF NOT EXISTS library_origins (workspace_id Utf8 NOT NULL, article_id Utf8 NOT NULL, subscription_id Utf8 NOT NULL, source_record_id Utf8 NOT NULL, PRIMARY KEY (workspace_id, article_id, subscription_id, source_record_id));
CREATE TABLE IF NOT EXISTS ingest_jobs (id Utf8 NOT NULL, status Utf8 NOT NULL, run_at_ms Int64 NOT NULL, first_attempt_ms Int64 NOT NULL, origin_key Utf8 NOT NULL, attempt Uint32 NOT NULL, lease_token Utf8, lease_deadline_ms Int64, item Utf8 NOT NULL, revision Uint64 NOT NULL, diagnostic Utf8, PRIMARY KEY (id));

UPSERT INTO workspaces (id, revision, document) VALUES ('workspace-a', 7u, '{"name":"Archive","state":"active"}');
UPSERT INTO subscriptions (id, revision, document) VALUES ('subscription-a', 4u, '{"workspace":"workspace-a","state":"paused","reason":"maintenance"}');
UPSERT INTO articles (id, revision, document) VALUES ('workspace-a/article-a', 9u, '{"title":"Durable article","read":true,"saved":true}');
UPSERT INTO rules (id, revision, document) VALUES ('workspace-a/rule-a', 3u, '{"field":"both","action":"markRead","enabled":true}');
UPSERT INTO content_manifests (id, revision, document) VALUES ('record-a', 2u, '{"record_id":"record-a","refresh_id":"refresh-a","source_revision":11}');
UPSERT INTO staged_content_chunks (record_id, refresh_id, representation, ordinal, bytes) VALUES ('record-a', 'refresh-a', 'safe', 0u, '[68,117,114,97,98,108,101,32,102,117,108,108,116,101,120,116]');
UPSERT INTO library_origins (workspace_id, article_id, subscription_id, source_record_id) VALUES ('workspace-a', 'article-a', 'subscription-a', 'record-a');
UPSERT INTO ingest_jobs (id, status, run_at_ms, first_attempt_ms, origin_key, attempt, lease_token, lease_deadline_ms, item, revision, diagnostic) VALUES ('job-expired-lease', 'leased', 0, 0, 'https://example.test', 2u, 'old-token', 1, '{"PollSource":{"source_id":"00000000-0000-0000-0000-000000000001"}}', 5u, NULL);
