use super::*;
use reader_wiki::*;
impl PostgresWikiStore {
    pub(super) async fn read_namespaces(
        &self,
        actor: Uuid,
        offset: u32,
    ) -> Result<NamespaceList, Error> {
        let rows=sqlx::query("SELECT n.id,n.name,CASE WHEN n.owner=$1 THEN 'owner' ELSE m.role END AS role FROM wiki_namespaces n LEFT JOIN wiki_members m ON m.namespace=n.id AND m.account=$1 WHERE n.owner=$1 OR m.account=$1 ORDER BY n.name,n.id LIMIT $2 OFFSET $3").bind(actor.to_string()).bind(self.limit()+1).bind(i64::from(offset)).fetch_all(&self.pool).await.map_err(storage)?;
        let has_more = rows.len() > self.limit() as usize;
        let items = rows
            .iter()
            .take(self.limit() as usize)
            .map(|r| {
                Ok(Namespace {
                    id: r.try_get("id").map_err(storage)?,
                    name: r.try_get("name").map_err(storage)?,
                    role: match r.try_get::<&str, _>("role").map_err(storage)? {
                        "owner" => Role::Owner,
                        "editor" => Role::Editor,
                        "reader" => Role::Reader,
                        _ => return Err(Error::Storage),
                    },
                })
            })
            .collect::<Result<_, Error>>()?;
        Ok(NamespaceList { items, has_more })
    }
    pub(super) async fn read_pages(
        &self,
        actor: Uuid,
        ns: Uuid,
        search: &str,
        trash: bool,
        offset: u32,
    ) -> Result<PageList, Error> {
        self.limits.search(search)?;
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let (_, role) = authorize(&mut tx, actor, ns, false).await?;
        if trash {
            editor(role)?;
        }
        let pattern = format!(
            "%{}%",
            search
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        );
        let rows = sqlx::query(include_str!("pages.sql"))
            .bind(ns)
            .bind(trash)
            .bind(search)
            .bind(pattern)
            .bind(self.limit() + 1)
            .bind(i64::from(offset))
            .bind(self.limits.input().search_excerpt_characters as i32)
            .fetch_all(&mut *tx)
            .await
            .map_err(storage)?;
        let has_more = rows.len() > self.limit() as usize;
        let items = rows
            .iter()
            .take(self.limit() as usize)
            .map(|r| {
                Ok(PageSummary {
                    excerpt: r.try_get("excerpt").map_err(storage)?,
                    excerpt_truncated: r.try_get("excerpt_truncated").map_err(storage)?,
                    id: r.try_get("id").map_err(storage)?,
                    name: r.try_get("name").map_err(storage)?,
                    updated_at: r.try_get("updated_at").map_err(storage)?,
                })
            })
            .collect::<Result<_, Error>>()?;
        tx.commit().await.map_err(storage)?;
        Ok(PageList { items, has_more })
    }
    pub(super) async fn read_history(
        &self,
        actor: Uuid,
        ns: Uuid,
        id: Uuid,
        offset: u32,
    ) -> Result<RevisionList, Error> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let (_, role) = authorize(&mut tx, actor, ns, false).await?;
        load_page(&mut tx, ns, id, role).await?;
        let rows=sqlx::query("SELECT r.revision,r.name,r.author,r.created_at,r.action,a.document::jsonb->>'username' AS author_name FROM wiki_revisions r JOIN accounts a ON a.id=r.author WHERE r.namespace=$1 AND r.page=$2 ORDER BY r.created_at DESC,r.revision LIMIT $3 OFFSET $4").bind(ns).bind(id).bind(self.limit()+1).bind(i64::from(offset)).fetch_all(&mut *tx).await.map_err(storage)?;
        let has_more = rows.len() > self.limit() as usize;
        let items = rows
            .iter()
            .take(self.limit() as usize)
            .map(|r| {
                Ok(RevisionSummary {
                    revision: r.try_get("revision").map_err(storage)?,
                    name: r.try_get("name").map_err(storage)?,
                    author: Uuid::parse_str(r.try_get("author").map_err(storage)?)
                        .map_err(storage)?,
                    created_at: r.try_get("created_at").map_err(storage)?,
                    action: r.try_get("action").map_err(storage)?,
                    author_name: r.try_get("author_name").map_err(storage)?,
                })
            })
            .collect::<Result<_, Error>>()?;
        tx.commit().await.map_err(storage)?;
        Ok(RevisionList { items, has_more })
    }
}
