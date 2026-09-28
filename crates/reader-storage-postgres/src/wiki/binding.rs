use super::*;
use reader_wiki::{Binding, PageSummary};
async fn subscription_owner(
    tx: &mut Transaction<'_, Postgres>,
    a: Uuid,
    s: Uuid,
) -> Result<(), Error> {
    let exists:Option<String>=sqlx::query_scalar("SELECT s.id FROM subscriptions s JOIN workspaces w ON w.id=s.document::jsonb->>'workspace_id' WHERE s.id=$1 AND w.document::jsonb->>'owner'=$2 FOR SHARE OF s,w").bind(s.to_string()).bind(a.to_string()).fetch_optional(&mut **tx).await.map_err(storage)?;
    if exists.is_none() {
        return Err(Error::NotFound);
    }
    Ok(())
}
impl PostgresWikiStore {
    pub(super) async fn read_binding(&self, a: Uuid, s: Uuid) -> Result<Binding, Error> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        subscription_owner(&mut tx, a, s).await?;
        let row=sqlx::query("SELECT namespace,page FROM subscription_wiki_links WHERE subscription_id=$1 AND owner=$2").bind(s.to_string()).bind(a.to_string()).fetch_optional(&mut *tx).await.map_err(storage)?;
        let mut result = Binding {
            linked: row.is_some(),
            page: None,
            namespace: None,
        };
        if let Some(row) = row {
            let n: Uuid = row.try_get("namespace").map_err(storage)?;
            let p: Uuid = row.try_get("page").map_err(storage)?;
            match authorize(&mut tx, a, n, false).await {
                Ok((_, role)) => match load_page(&mut tx, n, p, role).await {
                    Ok(p) if !p.deleted => {
                        result.namespace = Some(n);
                        result.page = Some(PageSummary {
                            excerpt: String::new(),
                            excerpt_truncated: false,
                            id: p.id,
                            name: p.name,
                            updated_at: p.updated_at,
                        });
                    }
                    Ok(_) | Err(Error::NotFound) => (),
                    Err(e) => return Err(e),
                },
                Err(Error::NotFound) => (),
                Err(e) => return Err(e),
            }
        }
        tx.commit().await.map_err(storage)?;
        Ok(result)
    }
    pub(super) async fn set_binding(
        &self,
        a: Uuid,
        s: Uuid,
        target: Option<(Uuid, Uuid)>,
    ) -> Result<(), Error> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        subscription_owner(&mut tx, a, s).await?;
        if let Some((n, p)) = target {
            let (_, role) = authorize(&mut tx, a, n, false).await?;
            let page = load_page(&mut tx, n, p, role).await?;
            if page.deleted {
                return Err(Error::NotFound);
            }
            sqlx::query("INSERT INTO subscription_wiki_links(owner,subscription_id,namespace,page) VALUES($1,$2,$3,$4) ON CONFLICT(subscription_id) DO UPDATE SET namespace=excluded.namespace,page=excluded.page").bind(a.to_string()).bind(s.to_string()).bind(n).bind(p).execute(&mut *tx).await.map_err(storage)?;
        } else {
            sqlx::query(
                "DELETE FROM subscription_wiki_links WHERE owner=$1 AND subscription_id=$2",
            )
            .bind(a.to_string())
            .bind(s.to_string())
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        }
        tx.commit().await.map_err(storage)
    }
}
