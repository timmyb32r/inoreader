use super::*;
use async_trait::async_trait;
use reader_wiki::*;
#[async_trait]
impl Store for PostgresWikiStore {
    async fn revision(&self, a: Uuid, n: Uuid, p: Uuid, v: Uuid) -> Result<Revision, Error> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let (_, role) = authorize(&mut tx, a, n, false).await?;
        load_page(&mut tx, n, p, role).await?;
        let row=sqlx::query("SELECT r.namespace,r.page AS id,r.revision,r.name,r.markdown,r.deleted,r.author,r.created_at AS updated_at,r.action,a.document::jsonb->>'username' AS author_name FROM wiki_revisions r JOIN accounts a ON a.id=r.author WHERE r.namespace=$1 AND r.page=$2 AND r.revision=$3").bind(n).bind(p).bind(v).fetch_optional(&mut *tx).await.map_err(storage)?.ok_or(Error::NotFound)?;
        let result = Revision {
            page: page_row(&row)?,
            action: row.try_get("action").map_err(storage)?,
            author_name: row.try_get("author_name").map_err(storage)?,
        };
        tx.commit().await.map_err(storage)?;
        Ok(result)
    }

    async fn links(&self, a: Uuid, n: Uuid, p: Uuid) -> Result<Vec<Link>, Error> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let (_, role) = authorize(&mut tx, a, n, false).await?;
        load_page(&mut tx, n, p, role).await?;
        let rows=sqlx::query("SELECT l.target_name,p.id FROM wiki_links l LEFT JOIN wiki_page_names names ON names.namespace=l.namespace AND names.name=l.target_name LEFT JOIN wiki_pages p ON p.namespace=names.namespace AND p.id=names.page AND NOT p.deleted WHERE l.namespace=$1 AND l.source=$2 ORDER BY l.target_name").bind(n).bind(p).fetch_all(&mut *tx).await.map_err(storage)?;
        let result = rows
            .iter()
            .map(|r| {
                Ok(Link {
                    name: r.try_get("target_name").map_err(storage)?,
                    page: r.try_get("id").map_err(storage)?,
                })
            })
            .collect::<Result<Vec<_>, Error>>()?;
        tx.commit().await.map_err(storage)?;
        Ok(result)
    }

    async fn namespaces(&self, a: Uuid, o: u32) -> Result<NamespaceList, Error> {
        self.read_namespaces(a, o).await
    }
    async fn create_namespace(&self, a: Uuid, id: Uuid, name: &str) -> Result<Namespace, Error> {
        self.limits.name(name)?;
        let mut tx = self.pool.begin().await.map_err(storage)?;
        sqlx::query("INSERT INTO wiki_namespaces(id,owner,name) VALUES($1,$2,$3) ON CONFLICT(id) DO NOTHING").bind(id).bind(a.to_string()).bind(name).execute(&mut *tx).await.map_err(storage)?;
        let (stored, role) = authorize(&mut tx, a, id, true).await?;
        if role != Role::Owner || stored != name {
            return Err(Error::Conflict);
        }
        tx.commit().await.map_err(storage)?;
        Ok(Namespace {
            id,
            name: stored,
            role,
        })
    }
    async fn namespace(&self, a: Uuid, n: Uuid) -> Result<Namespace, Error> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let (name, role) = authorize(&mut tx, a, n, false).await?;
        tx.commit().await.map_err(storage)?;
        Ok(Namespace { id: n, name, role })
    }
    async fn pages(&self, a: Uuid, n: Uuid, s: &str, t: bool, o: u32) -> Result<PageList, Error> {
        self.read_pages(a, n, s, t, o).await
    }
    async fn page(&self, a: Uuid, n: Uuid, p: Uuid) -> Result<Page, Error> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let (_, role) = authorize(&mut tx, a, n, false).await?;
        let page = load_page(&mut tx, n, p, role).await?;
        tx.commit().await.map_err(storage)?;
        Ok(page)
    }
    async fn resolve(&self, a: Uuid, n: Uuid, name: &str) -> Result<Page, Error> {
        self.limits.name(name)?;
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let (_, role) = authorize(&mut tx, a, n, false).await?;
        let id: Uuid =
            sqlx::query_scalar("SELECT page FROM wiki_page_names WHERE namespace=$1 AND name=$2")
                .bind(n)
                .bind(name)
                .fetch_optional(&mut *tx)
                .await
                .map_err(storage)?
                .ok_or(Error::NotFound)?;
        let page = load_page(&mut tx, n, id, role).await?;
        tx.commit().await.map_err(storage)?;
        Ok(page)
    }
    async fn write(&self, a: Uuid, n: Uuid, c: Write) -> Result<Page, Error> {
        self.change_page(a, n, c).await
    }
    async fn history(&self, a: Uuid, n: Uuid, p: Uuid, o: u32) -> Result<RevisionList, Error> {
        self.read_history(a, n, p, o).await
    }
    async fn draft(&self, a: Uuid, n: Uuid, id: Uuid) -> Result<Option<Draft>, Error> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let (_, role) = authorize(&mut tx, a, n, false).await?;
        editor(role)?;
        let row=sqlx::query("SELECT id,revision,page,base_revision,name,markdown FROM wiki_drafts WHERE namespace=$1 AND author=$2 AND id=$3").bind(n).bind(a.to_string()).bind(id).fetch_optional(&mut *tx).await.map_err(storage)?;
        let draft = row
            .map(|r| {
                Ok(Draft {
                    revision: Some(r.try_get("revision").map_err(storage)?),
                    id: r.try_get("id").map_err(storage)?,
                    page: r.try_get("page").map_err(storage)?,
                    base_revision: r.try_get("base_revision").map_err(storage)?,
                    name: r.try_get("name").map_err(storage)?,
                    markdown: r.try_get("markdown").map_err(storage)?,
                })
            })
            .transpose()?;
        tx.commit().await.map_err(storage)?;
        Ok(draft)
    }
    async fn save_draft(&self, a: Uuid, n: Uuid, mut d: Draft) -> Result<Draft, Error> {
        // Empty names are allowed only in a draft; publication validates names.
        if !d.name.is_empty() {
            self.limits.name(&d.name)?;
        }
        self.limits.markdown(&d.markdown)?;
        if d.page.is_some() != d.base_revision.is_some() {
            return Err(Error::Invalid(
                "draft page and base revision must be supplied together".into(),
            ));
        }
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let (_, role) = authorize(&mut tx, a, n, true).await?;
        editor(role)?;
        if let Some(p) = d.page {
            load_page(&mut tx, n, p, role).await?;
        }
        let previous=sqlx::query("SELECT revision,page,base_revision,name,markdown FROM wiki_drafts WHERE namespace=$1 AND author=$2 AND id=$3").bind(n).bind(a.to_string()).bind(d.id).fetch_optional(&mut *tx).await.map_err(storage)?;
        let current = previous
            .as_ref()
            .map(|r| r.try_get::<Uuid, _>("revision"))
            .transpose()
            .map_err(storage)?;
        if current != d.revision {
            let same = if let Some(r) = previous.as_ref() {
                r.try_get::<Option<Uuid>, _>("page").map_err(storage)? == d.page
                    && r.try_get::<Option<Uuid>, _>("base_revision")
                        .map_err(storage)?
                        == d.base_revision
                    && r.try_get::<String, _>("name").map_err(storage)? == d.name
                    && r.try_get::<String, _>("markdown").map_err(storage)? == d.markdown
            } else {
                false
            };
            if !same {
                return Err(Error::Conflict);
            }
            d.revision = current;
            tx.commit().await.map_err(storage)?;
            return Ok(d);
        }
        d.revision = Some(Uuid::new_v4());
        sqlx::query("INSERT INTO wiki_drafts(namespace,author,id,revision,page,base_revision,name,markdown) VALUES($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT(namespace,author,id) DO UPDATE SET revision=excluded.revision,page=excluded.page,base_revision=excluded.base_revision,name=excluded.name,markdown=excluded.markdown").bind(n).bind(a.to_string()).bind(d.id).bind(d.revision).bind(d.page).bind(d.base_revision).bind(&d.name).bind(&d.markdown).execute(&mut *tx).await.map_err(storage)?;
        tx.commit().await.map_err(storage)?;
        Ok(d)
    }
    async fn discard_draft(
        &self,
        a: Uuid,
        n: Uuid,
        id: Uuid,
        revision: Option<Uuid>,
    ) -> Result<(), Error> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let (_, role) = authorize(&mut tx, a, n, true).await?;
        editor(role)?;
        let current: Option<Uuid> = sqlx::query_scalar(
            "SELECT revision FROM wiki_drafts WHERE namespace=$1 AND author=$2 AND id=$3",
        )
        .bind(n)
        .bind(a.to_string())
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(storage)?;
        if current.is_some() && current != revision {
            return Err(Error::Conflict);
        }
        sqlx::query("DELETE FROM wiki_drafts WHERE namespace=$1 AND author=$2 AND id=$3")
            .bind(n)
            .bind(a.to_string())
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        tx.commit().await.map_err(storage)
    }
    async fn members(&self, a: Uuid, n: Uuid, o: u32) -> Result<MemberList, Error> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let (_, role) = authorize(&mut tx, a, n, false).await?;
        if role != Role::Owner {
            return Err(Error::Forbidden);
        }
        let rows=sqlx::query("SELECT m.account,m.role,a.document::jsonb->>'username' AS username FROM wiki_members m JOIN accounts a ON a.id=m.account WHERE m.namespace=$1 ORDER BY m.account LIMIT $2 OFFSET $3").bind(n).bind(self.limit()+1).bind(i64::from(o)).fetch_all(&mut *tx).await.map_err(storage)?;
        let has_more = rows.len() > self.limit() as usize;
        let items = rows
            .iter()
            .take(self.limit() as usize)
            .map(|r| {
                Ok(Member {
                    account: Uuid::parse_str(r.try_get("account").map_err(storage)?)
                        .map_err(storage)?,
                    username: r.try_get("username").map_err(storage)?,
                    role: match r.try_get::<&str, _>("role").map_err(storage)? {
                        "reader" => Role::Reader,
                        "editor" => Role::Editor,
                        _ => return Err(Error::Storage),
                    },
                })
            })
            .collect::<Result<_, Error>>()?;
        tx.commit().await.map_err(storage)?;
        Ok(MemberList { items, has_more })
    }
    async fn set_member(
        &self,
        a: Uuid,
        n: Uuid,
        username: &str,
        role: Option<Role>,
    ) -> Result<(), Error> {
        if role == Some(Role::Owner) {
            return Err(Error::Invalid("ownership transfer is not supported".into()));
        }
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let (_, own) = authorize(&mut tx, a, n, true).await?;
        if own != Role::Owner {
            return Err(Error::Forbidden);
        }
        let account: String =
            sqlx::query_scalar("SELECT id FROM accounts WHERE document::jsonb->>'username'=$1")
                .bind(username)
                .fetch_optional(&mut *tx)
                .await
                .map_err(storage)?
                .ok_or(Error::NotFound)?;
        if account == a.to_string() {
            return Err(Error::Invalid("owner membership cannot be changed".into()));
        }
        if let Some(role) = role {
            sqlx::query("INSERT INTO wiki_members(namespace,account,role) VALUES($1,$2,$3) ON CONFLICT(namespace,account) DO UPDATE SET role=excluded.role").bind(n).bind(account).bind(if role==Role::Reader{"reader"}else{"editor"}).execute(&mut *tx).await.map_err(storage)?;
        } else {
            sqlx::query("DELETE FROM wiki_members WHERE namespace=$1 AND account=$2")
                .bind(n)
                .bind(account)
                .execute(&mut *tx)
                .await
                .map_err(storage)?;
        }
        tx.commit().await.map_err(storage)
    }
    async fn binding(&self, a: Uuid, s: Uuid) -> Result<Binding, Error> {
        self.read_binding(a, s).await
    }
    async fn bind(&self, a: Uuid, s: Uuid, t: Option<(Uuid, Uuid)>) -> Result<(), Error> {
        self.set_binding(a, s, t).await
    }
}
