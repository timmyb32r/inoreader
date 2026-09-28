//! Unified read projection; all access predicates execute inside PostgreSQL.
pub(crate) mod projection;
use reader_application::{
    RepositoryError, SearchHit, SearchKind, SearchLimits, SearchPage, SearchPort, SearchRequest,
    SearchTarget,
};
use sqlx::{PgPool, Row};
use uuid::Uuid;
pub struct PostgresSearchStore {
    pool: PgPool,
    limits: SearchLimits,
}
impl PostgresSearchStore {
    pub fn new(pool: PgPool, limits: SearchLimits) -> Self {
        Self { pool, limits }
    }
}
fn storage(_: impl std::fmt::Display) -> RepositoryError {
    RepositoryError::Storage("search storage failure".into())
}
#[async_trait::async_trait]
impl SearchPort for PostgresSearchStore {
    async fn search(&self, owner: Uuid, q: SearchRequest) -> Result<SearchPage, RepositoryError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        // ACL changes take an exclusive namespace lock. Recheck access in the query
        // after obtaining these locks, so a concurrent revocation cannot leak a page.
        let namespaces:Vec<Uuid>=sqlx::query_scalar("SELECT id FROM wiki_namespaces n WHERE (owner=$1 OR EXISTS(SELECT 1 FROM wiki_members m WHERE m.namespace=n.id AND m.account=$1)) AND ($2::uuid IS NULL OR id=$2) ORDER BY id FOR SHARE").bind(owner.to_string()).bind(q.namespace()).fetch_all(&mut *tx).await.map_err(storage)?;
        if q.namespace().is_some_and(|ns| !namespaces.contains(&ns)) {
            return Err(RepositoryError::NotFound);
        }
        if let Some(id) = q.subscription() {
            let allowed:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM subscriptions s JOIN workspaces w ON w.id=(s.document::jsonb->>'workspace_id') WHERE s.id=$1 AND w.document::jsonb->>'owner'=$2)").bind(id.to_string()).bind(owner.to_string()).fetch_one(&mut *tx).await.map_err(storage)?;
            if !allowed {
                return Err(RepositoryError::NotFound);
            }
        }
        if q.query().is_empty() {
            return Ok(SearchPage {
                items: vec![],
                has_more: false,
            });
        }
        let pattern = format!(
            "%{}%",
            q.query()
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        );
        let kind = match q.kind() {
            SearchKind::All => "all",
            SearchKind::News => "news",
            SearchKind::Wiki => "wiki",
        };
        let rows = sqlx::query(include_str!("query.sql"))
            .bind(owner.to_string())
            .bind(pattern)
            .bind(kind)
            .bind(q.namespace())
            .bind(q.subscription().map(|id| id.to_string()))
            .bind(q.query())
            .bind(self.limits.input().excerpt_characters as i32)
            .bind(i64::from(self.limits.input().page_size) + 1)
            .bind(i64::from(q.offset()))
            .fetch_all(&mut *tx)
            .await
            .map_err(storage)?;
        let has_more = rows.len() > self.limits.input().page_size as usize;
        let mut items = Vec::new();
        for row in rows
            .into_iter()
            .take(self.limits.input().page_size as usize)
        {
            let container =
                Uuid::parse_str(row.try_get("container").map_err(storage)?).map_err(storage)?;
            let id = Uuid::parse_str(row.try_get("id").map_err(storage)?).map_err(storage)?;
            let target = match row.try_get::<&str, _>("kind").map_err(storage)? {
                "news" => SearchTarget::News {
                    workspace: container,
                    article: id,
                },
                "wiki" => SearchTarget::Wiki {
                    namespace: container,
                    page: id,
                },
                _ => return Err(storage("invalid kind")),
            };
            items.push(SearchHit {
                target,
                title: row.try_get("title").map_err(storage)?,
                context: row.try_get("context").map_err(storage)?,
                excerpt: row.try_get("excerpt").map_err(storage)?,
                updated_at: row.try_get("updated_at").map_err(storage)?,
            });
        }
        tx.commit().await.map_err(storage)?;
        Ok(SearchPage { items, has_more })
    }
}
