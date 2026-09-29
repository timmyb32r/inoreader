//! Namespace-scoped PostgreSQL adapter. SQLx observes queries without bind data.
mod access;
mod binding;
mod organization;
mod read;
mod write;
use reader_wiki::{Error, Limits, Page, Role};
use sqlx::{postgres::PgRow, PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

pub const ORGANIZATION_SCHEMA: &str = include_str!("organization.sql");
pub const SCHEMA: &str = include_str!("schema.sql");
pub struct PostgresWikiStore {
    pool: PgPool,
    limits: Limits,
}
impl PostgresWikiStore {
    pub fn new(pool: PgPool, limits: Limits) -> Self {
        Self { pool, limits }
    }
    fn limit(&self) -> i64 {
        self.limits.input().page_size as i64
    }
}
fn storage(_: impl std::fmt::Display) -> Error {
    Error::Storage
}
fn page_row(row: &PgRow) -> Result<Page, Error> {
    Ok(Page {
        parent: row.try_get("parent").map_err(storage)?,
        namespace: row.try_get("namespace").map_err(storage)?,
        id: row.try_get("id").map_err(storage)?,
        revision: row.try_get("revision").map_err(storage)?,
        name: row.try_get("name").map_err(storage)?,
        markdown: row.try_get("markdown").map_err(storage)?,
        deleted: row.try_get("deleted").map_err(storage)?,
        author: Uuid::parse_str(row.try_get("author").map_err(storage)?).map_err(storage)?,
        updated_at: row.try_get("updated_at").map_err(storage)?,
    })
}
/// Namespace lock is acquired before authorization, and retained until commit.
/// FOR SHARE allows concurrent reads while serializing against ACL revocation.
async fn authorize(
    tx: &mut Transaction<'_, Postgres>,
    actor: Uuid,
    namespace: Uuid,
    write: bool,
) -> Result<(String, Role), Error> {
    let sql = if write {
        "SELECT owner,name FROM wiki_namespaces WHERE id=$1 FOR UPDATE"
    } else {
        "SELECT owner,name FROM wiki_namespaces WHERE id=$1 FOR SHARE"
    };
    let row = sqlx::query(sql)
        .bind(namespace)
        .fetch_optional(&mut **tx)
        .await
        .map_err(storage)?
        .ok_or(Error::NotFound)?;
    let owner: String = row.try_get("owner").map_err(storage)?;
    let role = if owner == actor.to_string() {
        Role::Owner
    } else {
        let role: Option<String> =
            sqlx::query_scalar("SELECT role FROM wiki_members WHERE namespace=$1 AND account=$2")
                .bind(namespace)
                .bind(actor.to_string())
                .fetch_optional(&mut **tx)
                .await
                .map_err(storage)?;
        match role.as_deref() {
            Some("reader") => Role::Reader,
            Some("editor") => Role::Editor,
            None => return Err(Error::NotFound),
            _ => return Err(Error::Storage),
        }
    };
    Ok((row.try_get("name").map_err(storage)?, role))
}
fn editor(role: Role) -> Result<(), Error> {
    if role.can_edit() {
        Ok(())
    } else {
        Err(Error::Forbidden)
    }
}
async fn load_page(
    tx: &mut Transaction<'_, Postgres>,
    ns: Uuid,
    id: Uuid,
    role: Role,
) -> Result<Page, Error> {
    let row = sqlx::query("SELECT * FROM wiki_pages WHERE namespace=$1 AND id=$2")
        .bind(ns)
        .bind(id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(storage)?
        .ok_or(Error::NotFound)?;
    let page = page_row(&row)?;
    if page.deleted && !role.can_edit() {
        return Err(Error::NotFound);
    }
    Ok(page)
}
