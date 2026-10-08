-- Ingest may replace an article row within a transaction while retaining its ID.
-- Validate references at commit; never cascade ratings or receipts away.
-- Personal ratings are separate from ingest-owned article documents.
CREATE TABLE article_ratings (
 article_key TEXT PRIMARY KEY REFERENCES articles(id) DEFERRABLE INITIALLY DEFERRED,
 rating SMALLINT CHECK(rating BETWEEN 1 AND 10),
 rated_at TIMESTAMPTZ NOT NULL,
 reason TEXT
);
-- Exact commands and outcomes retain replay/undo evidence; no cascading deletion.
CREATE TABLE reading_completions (
 owner TEXT NOT NULL REFERENCES accounts(id),
 id UUID NOT NULL,
 article_key TEXT NOT NULL REFERENCES articles(id) DEFERRABLE INITIALLY DEFERRED,
 document TEXT NOT NULL,
 PRIMARY KEY(owner,id)
);
