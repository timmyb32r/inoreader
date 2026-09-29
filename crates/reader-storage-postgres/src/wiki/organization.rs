use super::*;
use reader_wiki::{Collection, Organization, PageList, PageSummary};

fn summary(row: &PgRow) -> Result<PageSummary, Error> {
    Ok(PageSummary {
        id: row.try_get("id").map_err(storage)?,
        name: row.try_get("name").map_err(storage)?,
        updated_at: row.try_get("updated_at").map_err(storage)?,
        excerpt: String::new(),
        excerpt_truncated: false,
    })
}
impl PostgresWikiStore {
    fn summaries(&self, rows: &[PgRow]) -> Result<PageList, Error> {
        Ok(PageList {
            has_more: rows.len() > self.limit() as usize,
            items: rows
                .iter()
                .take(self.limit() as usize)
                .map(summary)
                .collect::<Result<_, _>>()?,
        })
    }
    pub(super) async fn read_collection(
        &self,
        actor: Uuid,
        ns: Uuid,
        kind: Collection,
        offset: u32,
    ) -> Result<PageList, Error> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        authorize(&mut tx, actor, ns, false).await?;
        let rows=sqlx::query("SELECT p.id,p.name,p.updated_at FROM wiki_pages p WHERE p.namespace=$1 AND NOT p.deleted AND CASE WHEN $2 THEN EXISTS(SELECT 1 FROM wiki_favorites f WHERE f.namespace=p.namespace AND f.page=p.id AND f.account=$3) ELSE p.parent IS NULL AND NOT EXISTS(SELECT 1 FROM wiki_pages c WHERE c.namespace=p.namespace AND c.parent=p.id) END ORDER BY p.name,p.id LIMIT $4 OFFSET $5")
            .bind(ns).bind(matches!(kind,Collection::Favorites)).bind(actor.to_string()).bind(self.limit()+1).bind(i64::from(offset)).fetch_all(&mut *tx).await.map_err(storage)?;
        let result = self.summaries(&rows)?;
        tx.commit().await.map_err(storage)?;
        Ok(result)
    }
    pub(super) async fn read_organization(
        &self,
        actor: Uuid,
        ns: Uuid,
        id: Uuid,
        offset: u32,
    ) -> Result<Organization, Error> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let (_, role) = authorize(&mut tx, actor, ns, false).await?;
        let page = load_page(&mut tx, ns, id, role).await?;
        let parent=sqlx::query("SELECT id,name,updated_at FROM wiki_pages WHERE namespace=$1 AND id=$2 AND NOT deleted").bind(ns).bind(page.parent).fetch_optional(&mut *tx).await.map_err(storage)?.as_ref().map(summary).transpose()?;
        let rows=sqlx::query("SELECT id,name,updated_at FROM wiki_pages WHERE namespace=$1 AND parent=$2 AND NOT deleted ORDER BY name,id LIMIT $3 OFFSET $4").bind(ns).bind(id).bind(self.limit()+1).bind(i64::from(offset)).fetch_all(&mut *tx).await.map_err(storage)?;
        let favorite=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM wiki_favorites WHERE namespace=$1 AND page=$2 AND account=$3)").bind(ns).bind(id).bind(actor.to_string()).fetch_one(&mut *tx).await.map_err(storage)?;
        let result = Organization {
            parent,
            children: self.summaries(&rows)?,
            favorite,
        };
        tx.commit().await.map_err(storage)?;
        Ok(result)
    }
    pub(super) async fn set_favorite(
        &self,
        actor: Uuid,
        ns: Uuid,
        id: Uuid,
        favorite: bool,
    ) -> Result<(), Error> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let (_, role) = authorize(&mut tx, actor, ns, true).await?;
        let page = load_page(&mut tx, ns, id, role).await?;
        if page.deleted {
            return Err(Error::Conflict);
        }
        if favorite {
            sqlx::query("INSERT INTO wiki_favorites(namespace,page,account) VALUES($1,$2,$3) ON CONFLICT DO NOTHING").bind(ns).bind(id).bind(actor.to_string()).execute(&mut *tx).await.map_err(storage)?;
        } else {
            sqlx::query("DELETE FROM wiki_favorites WHERE namespace=$1 AND page=$2 AND account=$3")
                .bind(ns)
                .bind(id)
                .bind(actor.to_string())
                .execute(&mut *tx)
                .await
                .map_err(storage)?;
        }
        tx.commit().await.map_err(storage)
    }
    pub(super) async fn ensure_subscription_root(
        &self,
        actor: Uuid,
        ns: Uuid,
    ) -> Result<Page, Error> {
        self.limits.name("Subscriptions")?;
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let (_, role) = authorize(&mut tx, actor, ns, true).await?;
        editor(role)?;
        let existing: Option<Uuid> =
            sqlx::query_scalar("SELECT page FROM wiki_subscription_roots WHERE namespace=$1")
                .bind(ns)
                .fetch_optional(&mut *tx)
                .await
                .map_err(storage)?;
        if let Some(id) = existing {
            let p = load_page(&mut tx, ns, id, role).await?;
            if p.deleted {
                return Err(Error::Invalid(
                    "restore the Subscriptions parent page from trash first".into(),
                ));
            }
            tx.commit().await.map_err(storage)?;
            return Ok(p);
        }
        let occupied: bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM wiki_page_names WHERE namespace=$1 AND name='Subscriptions')").bind(ns).fetch_one(&mut *tx).await.map_err(storage)?;
        if occupied {
            return Err(Error::NameTaken);
        }
        let id = Uuid::new_v4();
        let revision = Uuid::new_v4();
        let row=sqlx::query("INSERT INTO wiki_pages(namespace,id,revision,name,markdown,deleted,author,updated_at,parent) VALUES($1,$2,$3,'Subscriptions','',false,$4,clock_timestamp(),NULL) RETURNING *").bind(ns).bind(id).bind(revision).bind(actor.to_string()).fetch_one(&mut *tx).await.map_err(storage)?;
        sqlx::query("INSERT INTO wiki_revisions(namespace,page,revision,name,markdown,deleted,author,created_at,action,parent) SELECT namespace,id,revision,name,markdown,deleted,author,updated_at,'create_subscription_root',parent FROM wiki_pages WHERE namespace=$1 AND id=$2").bind(ns).bind(id).execute(&mut *tx).await.map_err(storage)?;
        sqlx::query(
            "INSERT INTO wiki_page_names(namespace,name,page) VALUES($1,'Subscriptions',$2)",
        )
        .bind(ns)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(storage)?;
        sqlx::query("INSERT INTO wiki_subscription_roots(namespace,page) VALUES($1,$2)")
            .bind(ns)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        let result = page_row(&row)?;
        tx.commit().await.map_err(storage)?;
        Ok(result)
    }
}
