use super::*;

#[test]
fn cli_exposes_postgres_operations_without_retired_migration() {
    for command in ["serve", "check-config", "prepare-schema", "health"] {
        assert!(Cli::try_parse_from(["inoreader", command]).is_ok());
    }
    assert!(Cli::try_parse_from(["inoreader", "migrate-ydb-to-postgres"]).is_err());
}
