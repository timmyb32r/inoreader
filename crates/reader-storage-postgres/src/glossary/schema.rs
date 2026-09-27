pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS glossary_channels (
 owner UUID NOT NULL, workspace UUID NOT NULL, binding TEXT, bot_id BIGINT UNIQUE,
 encrypted_token BYTEA, poll_cursor BIGINT, poll_lease UUID, poll_until TIMESTAMPTZ,
 next_poll TIMESTAMPTZ NOT NULL DEFAULT now(), last_poll TIMESTAMPTZ, poll_error TEXT,
 history_before BIGINT, history_lease UUID, history_until TIMESTAMPTZ,
 next_history TIMESTAMPTZ NOT NULL DEFAULT now(), last_history TIMESTAMPTZ, history_error TEXT,
 coverage_note TEXT NOT NULL DEFAULT 'Public history is incomplete; deletions are not observable',
 import_complete BOOLEAN NOT NULL DEFAULT false, revision BIGINT NOT NULL DEFAULT 0,
 PRIMARY KEY(owner,workspace)
);
CREATE TABLE IF NOT EXISTS glossary_receipts (
 id UUID PRIMARY KEY, owner UUID NOT NULL, workspace UUID NOT NULL,
 kind TEXT NOT NULL, source_key TEXT NOT NULL, raw TEXT NOT NULL,
 received_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS glossary_receipt_source ON glossary_receipts(owner,workspace,kind,source_key);
CREATE TABLE IF NOT EXISTS glossary_events (
 owner UUID NOT NULL, workspace UUID NOT NULL, update_id BIGINT NOT NULL,
 receipt UUID NOT NULL REFERENCES glossary_receipts(id), document TEXT NOT NULL,
 applied BOOLEAN NOT NULL DEFAULT false, error TEXT, conflict BOOLEAN NOT NULL DEFAULT false,
 PRIMARY KEY(owner,workspace,update_id)
);
CREATE INDEX IF NOT EXISTS glossary_events_pending ON glossary_events(applied,update_id) WHERE error IS NULL;
CREATE TABLE IF NOT EXISTS glossary_posts (
 owner UUID NOT NULL, workspace UUID NOT NULL, message_id BIGINT NOT NULL,
 published_at TIMESTAMPTZ, edited_at BIGINT, kind TEXT NOT NULL,
 receipt UUID NOT NULL REFERENCES glossary_receipts(id), document TEXT NOT NULL, source_document TEXT NOT NULL,
 parser_version TEXT NOT NULL, projection_error TEXT, conflicted BOOLEAN NOT NULL DEFAULT false,
 PRIMARY KEY(owner,workspace,message_id)
);
CREATE TABLE IF NOT EXISTS glossary_definitions (
 owner UUID NOT NULL, workspace UUID NOT NULL, message_id BIGINT NOT NULL,
 position BIGINT NOT NULL, term TEXT COLLATE "C" NOT NULL, term_hash TEXT NOT NULL, document TEXT NOT NULL,
 PRIMARY KEY(owner,workspace,message_id,position),
 FOREIGN KEY(owner,workspace,message_id) REFERENCES glossary_posts(owner,workspace,message_id)
);
CREATE INDEX IF NOT EXISTS glossary_definitions_lookup ON glossary_definitions(owner,workspace,term_hash,message_id DESC,position DESC);
"#;
