CREATE TABLE wiki_namespaces (
 id UUID PRIMARY KEY, owner TEXT NOT NULL REFERENCES accounts(id), name TEXT NOT NULL
);
CREATE TABLE wiki_members (
 namespace UUID NOT NULL REFERENCES wiki_namespaces(id), account TEXT NOT NULL REFERENCES accounts(id),
 role TEXT NOT NULL CHECK(role IN ('reader','editor')), PRIMARY KEY(namespace,account)
);
CREATE TABLE wiki_pages (
 namespace UUID NOT NULL REFERENCES wiki_namespaces(id), id UUID NOT NULL,
 revision UUID NOT NULL, name TEXT NOT NULL, markdown TEXT NOT NULL, deleted BOOLEAN NOT NULL,
 author TEXT NOT NULL REFERENCES accounts(id), updated_at TIMESTAMPTZ NOT NULL,
 PRIMARY KEY(namespace,id)
);
CREATE TABLE wiki_page_names (
 namespace UUID NOT NULL, name TEXT NOT NULL, page UUID NOT NULL,
 PRIMARY KEY(namespace,name), FOREIGN KEY(namespace,page) REFERENCES wiki_pages(namespace,id)
);
CREATE TABLE wiki_revisions (
 namespace UUID NOT NULL, page UUID NOT NULL, revision UUID NOT NULL,
 name TEXT NOT NULL, markdown TEXT NOT NULL, deleted BOOLEAN NOT NULL,
 author TEXT NOT NULL REFERENCES accounts(id), created_at TIMESTAMPTZ NOT NULL, action TEXT NOT NULL,
 PRIMARY KEY(namespace,page,revision), FOREIGN KEY(namespace,page) REFERENCES wiki_pages(namespace,id)
);
ALTER TABLE wiki_pages ADD CONSTRAINT wiki_current_revision FOREIGN KEY(namespace,id,revision) REFERENCES wiki_revisions(namespace,page,revision) DEFERRABLE INITIALLY DEFERRED;
CREATE TABLE wiki_drafts (
 namespace UUID NOT NULL REFERENCES wiki_namespaces(id), author TEXT NOT NULL REFERENCES accounts(id),
 id UUID NOT NULL, revision UUID NOT NULL, page UUID, base_revision UUID, name TEXT NOT NULL, markdown TEXT NOT NULL,
 PRIMARY KEY(namespace,author,id),
 FOREIGN KEY(namespace,page) REFERENCES wiki_pages(namespace,id),
 FOREIGN KEY(namespace,page,base_revision) REFERENCES wiki_revisions(namespace,page,revision),
 CHECK ((page IS NULL) = (base_revision IS NULL))
);
CREATE TABLE wiki_links (
 namespace UUID NOT NULL, source UUID NOT NULL, target_name TEXT NOT NULL,
 PRIMARY KEY(namespace,source,target_name), FOREIGN KEY(namespace,source) REFERENCES wiki_pages(namespace,id)
);
CREATE TABLE subscription_wiki_links (
 owner TEXT NOT NULL REFERENCES accounts(id), subscription_id TEXT PRIMARY KEY REFERENCES subscriptions(id) ON DELETE CASCADE,
 namespace UUID NOT NULL, page UUID NOT NULL, FOREIGN KEY(namespace,page) REFERENCES wiki_pages(namespace,id)
);
CREATE TABLE wiki_operations (
 namespace UUID NOT NULL REFERENCES wiki_namespaces(id), actor TEXT NOT NULL REFERENCES accounts(id), operation UUID NOT NULL,
 request JSONB NOT NULL, response JSONB NOT NULL, PRIMARY KEY(namespace,actor,operation)
);
CREATE INDEX wiki_pages_listing ON wiki_pages(namespace,deleted,name,id);
CREATE INDEX wiki_revisions_listing ON wiki_revisions(namespace,page,created_at DESC,revision);
CREATE INDEX wiki_members_account ON wiki_members(account,namespace);
CREATE INDEX wiki_namespaces_owner ON wiki_namespaces(owner,id);

CREATE EXTENSION IF NOT EXISTS pg_trgm WITH SCHEMA public;
CREATE INDEX wiki_pages_name_search ON wiki_pages USING GIN(name public.gin_trgm_ops) WITH (fastupdate=off) WHERE NOT deleted;
CREATE INDEX wiki_pages_body_search ON wiki_pages USING GIN(markdown public.gin_trgm_ops) WITH (fastupdate=off) WHERE NOT deleted;
