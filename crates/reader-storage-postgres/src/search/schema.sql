CREATE TABLE search_content (
 record_id TEXT PRIMARY KEY REFERENCES content_manifests(id) ON DELETE CASCADE,
 text TEXT NOT NULL
);
