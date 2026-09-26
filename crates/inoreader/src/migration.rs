use async_trait::async_trait;
use reader_storage_ydb::{
    export_migration_snapshot, MigrationCell, MigrationColumnType, MigrationSnapshot,
    MigrationSnapshotSource, MigrationTable, MIGRATION_TABLES,
};
use sqlx::{PgPool, Postgres, QueryBuilder, Row};

pub async fn import_snapshot(
    pool: &PgPool,
    snapshot: &MigrationSnapshot,
) -> Result<(), Box<dyn std::error::Error>> {
    if snapshot.tables.len() != MIGRATION_TABLES.len() {
        return Err("migration snapshot does not contain every required table".into());
    }
    let mut transaction = pool.begin().await?;
    for expected in MIGRATION_TABLES {
        let table = snapshot
            .tables
            .iter()
            .find(|value| value.table.name == expected.name)
            .ok_or_else(|| format!("migration snapshot is missing {}", expected.name))?;
        let count_sql = format!("SELECT COUNT(*) AS count FROM {}", expected.name);
        let count: i64 = sqlx::query_scalar(&count_sql)
            .fetch_one(&mut *transaction)
            .await?;
        if count != 0 {
            return Err(format!(
                "PostgreSQL migration target table {} is not empty",
                expected.name
            )
            .into());
        }
        if table.rows.is_empty() {
            continue;
        }
        let converted = table
            .rows
            .iter()
            .map(|row| {
                if row.len() != expected.columns.len() {
                    return Err(format!("{} row has an invalid width", expected.name));
                }
                row.iter()
                    .zip(expected.columns)
                    .map(|(cell, column)| match (cell, column.value_type) {
                        (MigrationCell::Text(value), MigrationColumnType::Text) => {
                            Ok(PgCell::Text(Some(value.clone())))
                        }
                        (MigrationCell::Uint64(value), MigrationColumnType::Uint64) => {
                            i64::try_from(*value)
                                .map(|value| PgCell::Integer(Some(value)))
                                .map_err(|_| {
                                    format!(
                                        "{}.{} exceeds PostgreSQL BIGINT",
                                        expected.name, column.name
                                    )
                                })
                        }
                        (MigrationCell::Uint32(value), MigrationColumnType::Uint32) => {
                            Ok(PgCell::Integer(Some(i64::from(*value))))
                        }
                        (MigrationCell::Int64(value), MigrationColumnType::Int64) => {
                            Ok(PgCell::Integer(Some(*value)))
                        }
                        (MigrationCell::Null, MigrationColumnType::Text) if column.nullable => {
                            Ok(PgCell::Text(None))
                        }
                        (MigrationCell::Null, _) if column.nullable => Ok(PgCell::Integer(None)),
                        _ => Err(format!(
                            "{}.{} has an invalid value",
                            expected.name, column.name
                        )),
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, String>>()?;
        let columns = expected
            .columns
            .iter()
            .map(|column| column.name)
            .collect::<Vec<_>>()
            .join(", ");
        for row in &converted {
            let mut query = QueryBuilder::<Postgres>::new(format!(
                "INSERT INTO {} ({columns}) ",
                expected.name
            ));
            query.push_values(std::iter::once(row), |mut values, row| {
                for cell in row {
                    match cell {
                        PgCell::Text(value) => values.push_bind(value),
                        PgCell::Integer(value) => values.push_bind(value),
                    };
                }
            });
            query.build().execute(&mut *transaction).await?;
        }
    }
    transaction.commit().await?;

    let imported = export_migration_snapshot(&PostgresSnapshotSource(pool.clone())).await?;
    if imported.tables.len() != snapshot.tables.len()
        || imported
            .tables
            .iter()
            .zip(&snapshot.tables)
            .any(|(actual, expected)| {
                actual.table.name != expected.table.name
                    || actual.row_count != expected.row_count
                    || actual.utf8_bytes != expected.utf8_bytes
                    || actual.fingerprint != expected.fingerprint
                    || actual.rows != expected.rows
            })
    {
        return Err("PostgreSQL import verification differs from the YDB snapshot".into());
    }
    Ok(())
}

enum PgCell {
    Text(Option<String>),
    Integer(Option<i64>),
}

struct PostgresSnapshotSource(PgPool);

#[async_trait]
impl MigrationSnapshotSource for PostgresSnapshotSource {
    async fn migration_rows(
        &self,
        table: MigrationTable,
    ) -> Result<Vec<Vec<MigrationCell>>, String> {
        let columns = table
            .columns
            .iter()
            .map(|column| column.name)
            .collect::<Vec<_>>()
            .join(", ");
        let order = table.primary_key.join(", ");
        let query = format!("SELECT {columns} FROM {} ORDER BY {order}", table.name);
        let rows = sqlx::query(&query)
            .fetch_all(&self.0)
            .await
            .map_err(|error| error.to_string())?;
        rows.into_iter()
            .map(|row| {
                table
                    .columns
                    .iter()
                    .enumerate()
                    .map(
                        |(index, column)| match (column.value_type, column.nullable) {
                            (MigrationColumnType::Text, false) => row
                                .try_get::<String, _>(index)
                                .map(MigrationCell::Text)
                                .map_err(|error| error.to_string()),
                            (MigrationColumnType::Text, true) => row
                                .try_get::<Option<String>, _>(index)
                                .map(|value| value.map_or(MigrationCell::Null, MigrationCell::Text))
                                .map_err(|error| error.to_string()),
                            (MigrationColumnType::Int64, false) => row
                                .try_get::<i64, _>(index)
                                .map(MigrationCell::Int64)
                                .map_err(|error| error.to_string()),
                            (MigrationColumnType::Int64, true) => row
                                .try_get::<Option<i64>, _>(index)
                                .map(|value| {
                                    value.map_or(MigrationCell::Null, MigrationCell::Int64)
                                })
                                .map_err(|error| error.to_string()),
                            (MigrationColumnType::Uint64, false) => row
                                .try_get::<i64, _>(index)
                                .and_then(|value| {
                                    u64::try_from(value)
                                        .map_err(|error| sqlx::Error::Decode(error.into()))
                                })
                                .map(MigrationCell::Uint64)
                                .map_err(|error| error.to_string()),
                            (MigrationColumnType::Uint32, false) => row
                                .try_get::<i64, _>(index)
                                .and_then(|value| {
                                    u32::try_from(value)
                                        .map_err(|error| sqlx::Error::Decode(error.into()))
                                })
                                .map(MigrationCell::Uint32)
                                .map_err(|error| error.to_string()),
                            _ => Err(format!(
                                "unsupported nullable unsigned PostgreSQL column {}.{}",
                                table.name, column.name
                            )),
                        },
                    )
                    .collect()
            })
            .collect()
    }
}
