ALTER TABLE wiki_pages ADD COLUMN parent UUID;
ALTER TABLE wiki_pages ADD CONSTRAINT wiki_parent FOREIGN KEY(namespace,parent) REFERENCES wiki_pages(namespace,id);
ALTER TABLE wiki_pages ADD CONSTRAINT wiki_no_self_parent CHECK(parent IS DISTINCT FROM id);
ALTER TABLE wiki_revisions ADD COLUMN parent UUID;
CREATE INDEX wiki_children ON wiki_pages(namespace,parent,name,id);
CREATE TABLE wiki_favorites (
 namespace UUID NOT NULL, page UUID NOT NULL, account TEXT NOT NULL REFERENCES accounts(id),
 PRIMARY KEY(namespace,account,page), FOREIGN KEY(namespace,page) REFERENCES wiki_pages(namespace,id)
);
CREATE TABLE wiki_subscription_roots (
 namespace UUID PRIMARY KEY REFERENCES wiki_namespaces(id), page UUID NOT NULL,
 FOREIGN KEY(namespace,page) REFERENCES wiki_pages(namespace,id)
);
