use sqlx::PgPool;
pub async fn verify(pool: &PgPool) {
    let sql = format!(
        "EXPLAIN (FORMAT JSON) {}",
        include_str!("../../src/repository/subscription_stats.sql")
    );
    let plan: serde_json::Value = sqlx::query_scalar(&sql)
        .bind(vec!["projection-subscription".to_owned()])
        .bind("projection-workspace")
        .bind(86_400_000_i64)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_small_group_keys(&plan);

    let values = [
        "-000001-12-31T23:59:59Z",
        "0000-01-01T00:00:00Z",
        "2026-09-27T12:00:59.000000001Z",
        "2026-09-27T12:00:59.000000002Z",
        "2026-09-27T12:00:60.999999999Z",
        "2026-09-27T12:01:00Z",
        "+10000-01-01T00:00:00Z",
    ];
    for pair in values.windows(2) {
        let ordered: bool =
            sqlx::query_scalar("SELECT reader_arrival_order($1)<reader_arrival_order($2)")
                .bind(pair[0])
                .bind(pair[1])
                .fetch_one(pool)
                .await
                .unwrap();
        assert!(ordered, "timestamps must retain exact ordering: {pair:?}");
    }
    let equal: bool = sqlx::query_scalar("SELECT reader_arrival_order('2026-01-01T00:00:00.1Z')=reader_arrival_order('2026-01-01T00:00:00.100000000Z')").fetch_one(pool).await.unwrap();
    assert!(equal);
    let id = "projection-fixture/article";
    let original = r#"{ "first_arrived_at":"2026-09-27T12:00:00.000000001Z", "state":{"read":false,"later":true}, "preserve":"文字", "unknown":123456789012345678901234567890 }"#;
    sqlx::query("INSERT INTO articles(id,revision,document) VALUES($1,0,$2)")
        .bind(id)
        .bind(original)
        .execute(pool)
        .await
        .unwrap();
    let row: (String, String, bool, bool) =
        sqlx::query_as("SELECT document,workspace_key,is_read,is_later FROM articles WHERE id=$1")
            .bind(id)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(
        row,
        (original.into(), "projection-fixture".into(), false, true)
    );
    let updated = original.replace("\"read\":false", "\"read\":true");
    sqlx::query("UPDATE articles SET document=$2 WHERE id=$1")
        .bind(id)
        .bind(&updated)
        .execute(pool)
        .await
        .unwrap();
    let row: (String, bool) = sqlx::query_as("SELECT document,is_read FROM articles WHERE id=$1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(row, (updated, true));
    sqlx::query("DELETE FROM articles WHERE id=$1")
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
}

// A plan regression: the former query sorted repeated icon/source payloads for
// every article. Counts must aggregate only subscription/article identities.
fn assert_small_group_keys(value: &serde_json::Value) {
    match value {
        serde_json::Value::Object(fields) => {
            if let Some(keys) = fields.get("Group Key") {
                let keys = keys.to_string();
                assert!(
                    !keys.contains("data_url") && !keys.contains("document"),
                    "large payload in aggregation: {keys}"
                );
            }
            for child in fields.values() {
                assert_small_group_keys(child);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                assert_small_group_keys(item);
            }
        }
        _ => {}
    }
}
