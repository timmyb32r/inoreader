use super::*;
use reader_wiki::{link_names, ChangeInput, Write};
impl PostgresWikiStore {
    pub(super) async fn change_page(
        &self,
        actor: Uuid,
        ns: Uuid,
        command: Write,
    ) -> Result<Page, Error> {
        let input = command.input();
        let request = serde_json::to_value(input).map_err(storage)?;
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let (_, role) = authorize(&mut tx, actor, ns, true).await?;
        editor(role)?;
        let prior=sqlx::query("SELECT request,response FROM wiki_operations WHERE namespace=$1 AND actor=$2 AND operation=$3").bind(ns).bind(actor.to_string()).bind(input.operation).fetch_optional(&mut *tx).await.map_err(storage)?;
        if let Some(prior) = prior {
            if prior
                .try_get::<serde_json::Value, _>("request")
                .map_err(storage)?
                != request
            {
                return Err(Error::Conflict);
            }
            let page = serde_json::from_value(prior.try_get("response").map_err(storage)?)
                .map_err(storage)?;
            tx.commit().await.map_err(storage)?;
            return Ok(page);
        }
        let current = sqlx::query("SELECT * FROM wiki_pages WHERE namespace=$1 AND id=$2")
            .bind(ns)
            .bind(input.page)
            .fetch_optional(&mut *tx)
            .await
            .map_err(storage)?
            .as_ref()
            .map(page_row)
            .transpose()?;
        if current.as_ref().map(|p| p.revision) != input.expected_revision {
            return Err(Error::Conflict);
        }
        let mut parent = current.as_ref().and_then(|p| p.parent);
        let (name, markdown, deleted, action) = match &input.change {
            ChangeInput::Save { name, markdown } => {
                if current.as_ref().is_some_and(|p| p.deleted) {
                    return Err(Error::Conflict);
                }
                (name.clone(), markdown.clone(), false, "save")
            }
            ChangeInput::SetParent { parent: selected } => {
                let p = current.as_ref().ok_or(Error::NotFound)?;
                if p.deleted {
                    return Err(Error::Conflict);
                }
                parent = *selected;
                (p.name.clone(), p.markdown.clone(), false, "set_parent")
            }
            ChangeInput::Rename { name } => {
                let p = current.as_ref().ok_or(Error::NotFound)?;
                if p.deleted {
                    return Err(Error::Conflict);
                }
                (name.clone(), p.markdown.clone(), false, "rename")
            }
            ChangeInput::Trash | ChangeInput::Restore => {
                let p = current.as_ref().ok_or(Error::NotFound)?;
                let deleted = matches!(input.change, ChangeInput::Trash);
                if p.deleted == deleted {
                    return Err(Error::Conflict);
                }
                (
                    p.name.clone(),
                    p.markdown.clone(),
                    deleted,
                    if deleted { "trash" } else { "restore" },
                )
            }
            ChangeInput::RestoreRevision { revision } => {
                let p = current.as_ref().ok_or(Error::NotFound)?;
                if p.deleted {
                    return Err(Error::Conflict);
                }
                let r=sqlx::query("SELECT name,markdown,parent FROM wiki_revisions WHERE namespace=$1 AND page=$2 AND revision=$3").bind(ns).bind(input.page).bind(revision).fetch_optional(&mut *tx).await.map_err(storage)?.ok_or(Error::NotFound)?;
                parent = r.try_get("parent").map_err(storage)?;
                (
                    r.try_get("name").map_err(storage)?,
                    r.try_get("markdown").map_err(storage)?,
                    false,
                    "restore_revision",
                )
            }
        };
        if let Some(parent_id) = parent {
            // Namespace write lock serializes concurrent reparenting and prevents
            // write skew: A→B and B→A cannot both pass the cycle check.
            let target = load_page(&mut tx, ns, parent_id, role).await?;
            if target.deleted
                && matches!(
                    input.change,
                    ChangeInput::SetParent { .. } | ChangeInput::RestoreRevision { .. }
                )
            {
                return Err(Error::Invalid("parent page is in trash".into()));
            }
            let cycle: bool = sqlx::query_scalar("WITH RECURSIVE ancestors AS (SELECT id,parent FROM wiki_pages WHERE namespace=$1 AND id=$2 UNION SELECT p.id,p.parent FROM wiki_pages p JOIN ancestors a ON p.id=a.parent WHERE p.namespace=$1) SELECT EXISTS(SELECT 1 FROM ancestors WHERE id=$3)")
                .bind(ns).bind(parent_id).bind(input.page).fetch_one(&mut *tx).await.map_err(storage)?;
            if cycle {
                return Err(Error::Invalid("parent would create a cycle".into()));
            }
        }
        // Revalidate historical text against the current configured limits too.
        self.limits.name(&name)?;
        self.limits.markdown(&markdown)?;
        let occupied: Option<Uuid> =
            sqlx::query_scalar("SELECT page FROM wiki_page_names WHERE namespace=$1 AND name=$2")
                .bind(ns)
                .bind(&name)
                .fetch_optional(&mut *tx)
                .await
                .map_err(storage)?;
        if occupied.is_some_and(|id| id != input.page) {
            return Err(Error::NameTaken);
        }
        let revision = Uuid::new_v4();
        let row=sqlx::query("INSERT INTO wiki_pages(namespace,id,revision,name,markdown,deleted,author,updated_at,parent) VALUES($1,$2,$3,$4,$5,$6,$7,clock_timestamp(),$8) ON CONFLICT(namespace,id) DO UPDATE SET revision=excluded.revision,name=excluded.name,markdown=excluded.markdown,deleted=excluded.deleted,author=excluded.author,updated_at=excluded.updated_at,parent=excluded.parent RETURNING *").bind(ns).bind(input.page).bind(revision).bind(&name).bind(&markdown).bind(deleted).bind(actor.to_string()).bind(parent).fetch_one(&mut *tx).await.map_err(storage)?;
        let page = page_row(&row)?;
        if occupied.is_none() {
            sqlx::query("INSERT INTO wiki_page_names(namespace,name,page) VALUES($1,$2,$3)")
                .bind(ns)
                .bind(&name)
                .bind(input.page)
                .execute(&mut *tx)
                .await
                .map_err(storage)?;
        }
        sqlx::query("INSERT INTO wiki_revisions(namespace,page,revision,name,markdown,deleted,author,created_at,action,parent) SELECT namespace,id,revision,name,markdown,deleted,author,updated_at,$3,parent FROM wiki_pages WHERE namespace=$1 AND id=$2").bind(ns).bind(input.page).bind(action).execute(&mut *tx).await.map_err(storage)?;
        sqlx::query("DELETE FROM wiki_links WHERE namespace=$1 AND source=$2")
            .bind(ns)
            .bind(input.page)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        for target in link_names(&markdown) {
            sqlx::query("INSERT INTO wiki_links(namespace,source,target_name) VALUES($1,$2,$3)")
                .bind(ns)
                .bind(input.page)
                .bind(target)
                .execute(&mut *tx)
                .await
                .map_err(storage)?;
        }
        sqlx::query("INSERT INTO wiki_operations(namespace,actor,operation,request,response) VALUES($1,$2,$3,$4,$5)").bind(ns).bind(actor.to_string()).bind(input.operation).bind(request).bind(serde_json::to_value(&page).map_err(storage)?).execute(&mut *tx).await.map_err(storage)?;
        tx.commit().await.map_err(storage)?;
        Ok(page)
    }
}
