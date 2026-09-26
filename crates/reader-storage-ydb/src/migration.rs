use crate::{ProductionYdbTransport, SCHEMA_VERSION};
use async_trait::async_trait;
use reader_application::RepositoryError;
use std::io::Cursor;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MigrationColumnType {
    Text,
    Uint64,
    Uint32,
    Int64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MigrationColumn {
    pub name: &'static str,
    pub value_type: MigrationColumnType,
    pub nullable: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MigrationTable {
    pub name: &'static str,
    pub columns: &'static [MigrationColumn],
    pub primary_key: &'static [&'static str],
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MigrationCell {
    Text(String),
    Uint64(u64),
    Uint32(u32),
    Int64(i64),
    Null,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationTableSnapshot {
    pub table: MigrationTable,
    pub rows: Vec<Vec<MigrationCell>>,
    pub row_count: u64,
    pub utf8_bytes: u64,
    pub fingerprint: u128,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationSnapshot {
    pub source_schema_version: u64,
    pub tables: Vec<MigrationTableSnapshot>,
}

const ID_DOCUMENT: &[MigrationColumn] = &[
    c("id", MigrationColumnType::Text, false),
    c("revision", MigrationColumnType::Uint64, false),
    c("document", MigrationColumnType::Text, false),
];
const INGEST: &[MigrationColumn] = &[
    c("id", MigrationColumnType::Text, false),
    c("status", MigrationColumnType::Text, false),
    c("run_at_ms", MigrationColumnType::Int64, false),
    c("first_attempt_ms", MigrationColumnType::Int64, false),
    c("origin_key", MigrationColumnType::Text, false),
    c("attempt", MigrationColumnType::Uint32, false),
    c("lease_token", MigrationColumnType::Text, true),
    c("lease_deadline_ms", MigrationColumnType::Int64, true),
    c("item", MigrationColumnType::Text, false),
    c("revision", MigrationColumnType::Uint64, false),
    c("diagnostic", MigrationColumnType::Text, true),
];
const IDENTITY: &[MigrationColumn] = &[
    c("source_id", MigrationColumnType::Text, false),
    c("upstream_id", MigrationColumnType::Text, false),
    c("record_id", MigrationColumnType::Text, false),
    c("observed_at_ms", MigrationColumnType::Int64, false),
    c("revision", MigrationColumnType::Uint64, false),
    c("document", MigrationColumnType::Text, false),
];
const DEDUP: &[MigrationColumn] = &[
    c("workspace_id", MigrationColumnType::Text, false),
    c("dedup_key", MigrationColumnType::Text, false),
    c("article_id", MigrationColumnType::Text, false),
    c("revision", MigrationColumnType::Uint64, false),
    c("document", MigrationColumnType::Text, false),
];
const ORIGINS: &[MigrationColumn] = &[
    c("workspace_id", MigrationColumnType::Text, false),
    c("article_id", MigrationColumnType::Text, false),
    c("subscription_id", MigrationColumnType::Text, false),
    c("source_record_id", MigrationColumnType::Text, false),
];
const STAGED: &[MigrationColumn] = &[
    c("record_id", MigrationColumnType::Text, false),
    c("refresh_id", MigrationColumnType::Text, false),
    c("representation", MigrationColumnType::Text, false),
    c("ordinal", MigrationColumnType::Uint32, false),
    c("bytes", MigrationColumnType::Text, false),
];
const SOURCE_URL: &[MigrationColumn] = &[
    c("url", MigrationColumnType::Text, false),
    c("source_id", MigrationColumnType::Text, false),
];
const SUB_SOURCE: &[MigrationColumn] = &[
    c("subscription_id", MigrationColumnType::Text, false),
    c("source_id", MigrationColumnType::Text, false),
];
const HEALTH: &[MigrationColumn] = &[
    c("source_id", MigrationColumnType::Text, false),
    c("document", MigrationColumnType::Text, false),
];
const ACTIVITY: &[MigrationColumn] = &[
    c("subscription_id", MigrationColumnType::Text, false),
    c("occurred_at_ms", MigrationColumnType::Int64, false),
    c("id", MigrationColumnType::Text, false),
    c("document", MigrationColumnType::Text, false),
];
const WORKSPACE_URL: &[MigrationColumn] = &[
    c("id", MigrationColumnType::Text, false),
    c("subscription_id", MigrationColumnType::Text, false),
];
const EVALUATION: &[MigrationColumn] = &[
    c("workspace_id", MigrationColumnType::Text, false),
    c("article_id", MigrationColumnType::Text, false),
    c("rule_id", MigrationColumnType::Text, false),
    c("rule_version", MigrationColumnType::Uint64, false),
    c("document", MigrationColumnType::Text, false),
];
const fn c(name: &'static str, value_type: MigrationColumnType, nullable: bool) -> MigrationColumn {
    MigrationColumn {
        name,
        value_type,
        nullable,
    }
}
const fn t(
    name: &'static str,
    columns: &'static [MigrationColumn],
    primary_key: &'static [&'static str],
) -> MigrationTable {
    MigrationTable {
        name,
        columns,
        primary_key,
    }
}
pub const MIGRATION_TABLES: &[MigrationTable] = &[
    t("schema_metadata", ID_DOCUMENT, &["id"]),
    t("workspaces", ID_DOCUMENT, &["id"]),
    t("subscriptions", ID_DOCUMENT, &["id"]),
    t("articles", ID_DOCUMENT, &["id"]),
    t("jobs", ID_DOCUMENT, &["id"]),
    t("outbox", ID_DOCUMENT, &["id"]),
    t("content_manifests", ID_DOCUMENT, &["id"]),
    t("accounts", ID_DOCUMENT, &["id"]),
    t("username_reservations", ID_DOCUMENT, &["id"]),
    t("invites", ID_DOCUMENT, &["id"]),
    t("sessions", ID_DOCUMENT, &["id"]),
    t("password_resets", ID_DOCUMENT, &["id"]),
    t("login_attempts", ID_DOCUMENT, &["id"]),
    t("rules", ID_DOCUMENT, &["id"]),
    t("web_feed_recipes", ID_DOCUMENT, &["id"]),
    t("sources", ID_DOCUMENT, &["id"]),
    t("source_records", ID_DOCUMENT, &["id"]),
    t("delivery_origins", ID_DOCUMENT, &["id"]),
    t("content_chunks", ID_DOCUMENT, &["id"]),
    t("content_refresh_state", ID_DOCUMENT, &["id"]),
    t("ingest_jobs", INGEST, &["id"]),
    t(
        "source_record_identity",
        IDENTITY,
        &["source_id", "upstream_id"],
    ),
    t("library_dedup", DEDUP, &["workspace_id", "dedup_key"]),
    t(
        "library_origins",
        ORIGINS,
        &[
            "workspace_id",
            "article_id",
            "subscription_id",
            "source_record_id",
        ],
    ),
    t(
        "staged_content_chunks",
        STAGED,
        &["record_id", "refresh_id", "representation", "ordinal"],
    ),
    t("source_urls", SOURCE_URL, &["url"]),
    t("subscription_sources", SUB_SOURCE, &["subscription_id"]),
    t("source_health", HEALTH, &["source_id"]),
    t(
        "subscription_activity",
        ACTIVITY,
        &["subscription_id", "occurred_at_ms", "id"],
    ),
    t("source_url_previews", ID_DOCUMENT, &["id"]),
    t("workspace_feed_urls", WORKSPACE_URL, &["id"]),
    t("seed_items", ID_DOCUMENT, &["id"]),
    t(
        "rule_evaluations",
        EVALUATION,
        &["workspace_id", "article_id", "rule_id", "rule_version"],
    ),
];

#[async_trait]
pub trait MigrationSnapshotSource: Send + Sync {
    async fn migration_rows(
        &self,
        table: MigrationTable,
    ) -> Result<Vec<Vec<MigrationCell>>, String>;
}

pub async fn export_migration_snapshot(
    source: &impl MigrationSnapshotSource,
) -> Result<MigrationSnapshot, RepositoryError> {
    let mut tables = Vec::with_capacity(MIGRATION_TABLES.len());
    for table in MIGRATION_TABLES {
        let mut rows = source
            .migration_rows(*table)
            .await
            .map_err(RepositoryError::Storage)?;
        validate_rows(*table, &rows)?;
        rows.sort_by_key(|row| primary_key_bytes(*table, row));
        let (utf8_bytes, fingerprint) = metadata(*table, &rows)?;
        tables.push(MigrationTableSnapshot {
            table: *table,
            row_count: rows.len() as u64,
            rows,
            utf8_bytes,
            fingerprint,
        });
    }
    let schema = tables
        .first()
        .and_then(|v| {
            v.rows
                .iter()
                .find(|r| matches!(r.first(),Some(MigrationCell::Text(id)) if id=="schema"))
        })
        .and_then(|r| match r.get(2) {
            Some(MigrationCell::Text(v)) => serde_json::from_str::<serde_json::Value>(v)
                .ok()?
                .get("version")?
                .as_u64(),
            _ => None,
        })
        .ok_or_else(|| {
            RepositoryError::Storage("migration snapshot is missing the schema marker".into())
        })?;
    if schema != SCHEMA_VERSION {
        return Err(RepositoryError::Storage(format!(
            "migration requires schema version {SCHEMA_VERSION}, found {schema}"
        )));
    }
    Ok(MigrationSnapshot {
        source_schema_version: schema,
        tables,
    })
}
fn validate_rows(
    table: MigrationTable,
    rows: &[Vec<MigrationCell>],
) -> Result<(), RepositoryError> {
    let mut keys = std::collections::HashSet::new();
    for row in rows {
        if row.len() != table.columns.len() {
            return Err(RepositoryError::Storage(format!(
                "{} row has {} values, expected {}",
                table.name,
                row.len(),
                table.columns.len()
            )));
        }
        for (value, column) in row.iter().zip(table.columns) {
            let valid = matches!(
                (value, column.value_type),
                (MigrationCell::Text(_), MigrationColumnType::Text)
                    | (MigrationCell::Uint64(_), MigrationColumnType::Uint64)
                    | (MigrationCell::Uint32(_), MigrationColumnType::Uint32)
                    | (MigrationCell::Int64(_), MigrationColumnType::Int64)
            ) || matches!(value, MigrationCell::Null) && column.nullable;
            if !valid {
                return Err(RepositoryError::Storage(format!(
                    "{}.{} has an invalid migration value",
                    table.name, column.name
                )));
            }
        }
        let mut key = Vec::new();
        for name in table.primary_key {
            let index = table
                .columns
                .iter()
                .position(|c| c.name == *name)
                .ok_or_else(|| {
                    RepositoryError::Storage(format!(
                        "{}.{} primary key column is absent",
                        table.name, name
                    ))
                })?;
            encode_cell(&mut key, &row[index]);
        }
        if !keys.insert(key) {
            return Err(RepositoryError::Storage(format!(
                "{} contains a duplicate primary key",
                table.name
            )));
        }
    }
    Ok(())
}
fn metadata(
    table: MigrationTable,
    rows: &[Vec<MigrationCell>],
) -> Result<(u64, u128), RepositoryError> {
    let mut bytes = Vec::new();
    let mut utf8 = 0u64;
    put(&mut bytes, table.name.as_bytes());
    for row in rows {
        for cell in row {
            match cell {
                MigrationCell::Text(v) => {
                    utf8 = utf8.checked_add(v.len() as u64).ok_or_else(|| {
                        RepositoryError::Storage("migration byte count overflow".into())
                    })?;
                    bytes.push(1);
                    put(&mut bytes, v.as_bytes())
                }
                MigrationCell::Uint64(v) => {
                    bytes.push(2);
                    bytes.extend(v.to_le_bytes())
                }
                MigrationCell::Uint32(v) => {
                    bytes.push(3);
                    bytes.extend(v.to_le_bytes())
                }
                MigrationCell::Int64(v) => {
                    bytes.push(4);
                    bytes.extend(v.to_le_bytes())
                }
                MigrationCell::Null => bytes.push(0),
            }
        }
    }
    let hash = murmur3::murmur3_x64_128(&mut Cursor::new(bytes), 0)
        .map_err(|e| RepositoryError::Storage(e.to_string()))?;
    Ok((utf8, hash))
}
fn put(out: &mut Vec<u8>, value: &[u8]) {
    out.extend((value.len() as u64).to_le_bytes());
    out.extend(value)
}
fn encode_cell(out: &mut Vec<u8>, cell: &MigrationCell) {
    match cell {
        MigrationCell::Text(v) => {
            out.push(1);
            put(out, v.as_bytes())
        }
        MigrationCell::Uint64(v) => {
            out.push(2);
            out.extend(v.to_le_bytes())
        }
        MigrationCell::Uint32(v) => {
            out.push(3);
            out.extend(v.to_le_bytes())
        }
        MigrationCell::Int64(v) => {
            out.push(4);
            out.extend(v.to_le_bytes())
        }
        MigrationCell::Null => out.push(0),
    }
}

fn primary_key_bytes(table: MigrationTable, row: &[MigrationCell]) -> Vec<u8> {
    let mut key = Vec::new();
    for name in table.primary_key {
        let index = table
            .columns
            .iter()
            .position(|column| column.name == *name)
            .expect("validated migration table primary key");
        encode_cell(&mut key, &row[index]);
    }
    key
}

#[async_trait]
impl MigrationSnapshotSource for ProductionYdbTransport {
    async fn migration_rows(
        &self,
        table: MigrationTable,
    ) -> Result<Vec<Vec<MigrationCell>>, String> {
        let columns = table
            .columns
            .iter()
            .map(|c| format!("`{}`", c.name))
            .collect::<Vec<_>>()
            .join(",");
        let order = table
            .primary_key
            .iter()
            .map(|v| format!("`{v}`"))
            .collect::<Vec<_>>()
            .join(",");
        let query = format!("SELECT {columns} FROM `{}` ORDER BY {order}", table.name);
        let set = self
            .client
            .query_client()
            .query_result_set(query)
            .await
            .map_err(|e| e.to_string())?;
        set.into_iter()
            .map(|mut row| {
                table
                    .columns
                    .iter()
                    .map(|column| {
                        let value = row
                            .remove_field_by_name(column.name)
                            .map_err(|e| e.to_string())?;
                        match (column.value_type, column.nullable) {
                            (MigrationColumnType::Text, false) => Ok(MigrationCell::Text(
                                value.try_into().map_err(|e: ydb::YdbError| e.to_string())?,
                            )),
                            (MigrationColumnType::Text, true) => {
                                Ok(Option::<String>::try_from(value)
                                    .map_err(|e: ydb::YdbError| e.to_string())?
                                    .map_or(MigrationCell::Null, MigrationCell::Text))
                            }
                            (MigrationColumnType::Uint64, false) => Ok(MigrationCell::Uint64(
                                value.try_into().map_err(|e: ydb::YdbError| e.to_string())?,
                            )),
                            (MigrationColumnType::Uint32, false) => Ok(MigrationCell::Uint32(
                                value.try_into().map_err(|e: ydb::YdbError| e.to_string())?,
                            )),
                            (MigrationColumnType::Int64, false) => Ok(MigrationCell::Int64(
                                value.try_into().map_err(|e: ydb::YdbError| e.to_string())?,
                            )),
                            (MigrationColumnType::Int64, true) => {
                                Ok(Option::<i64>::try_from(value)
                                    .map_err(|e: ydb::YdbError| e.to_string())?
                                    .map_or(MigrationCell::Null, MigrationCell::Int64))
                            }
                            _ => Err("unsupported nullable numeric migration column".into()),
                        }
                    })
                    .collect()
            })
            .collect()
    }
}
