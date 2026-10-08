use super::*;
use reader_ai::{AiStore, StatisticsBucket, StatisticsRange};
pub async fn verify(pool: &PgPool) {
    let reader = std::sync::Arc::new(
        PostgresRepository::new(pool.clone(), ReasonPolicy::new(4096).unwrap(), 100, 86400)
            .unwrap(),
    );
    let store = reader_storage_postgres::PostgresAiStore::new(
        pool.clone(),
        reader,
        std::num::NonZeroU32::new(2).unwrap(),
    );
    let owner = Uuid::new_v4();
    let other = Uuid::new_v4();
    for (account, day, mode, actual) in [
        (owner, "2025-12-31", "summary", Some("0.000000001")),
        (owner, "2026-01-01", "summary", Some("0.123456789")),
        (owner, "2026-01-01", "summary", None),
        (owner, "2026-01-31", "terms", Some("0")),
        (owner, "2026-02-01", "ranking", Some("0.3")),
        (other, "2026-01-01", "summary", Some("99")),
    ] {
        sqlx::query("INSERT INTO ai_spending(id,owner,day,mode,reserved,actual) VALUES($1,$2,$3::text::date,$4,0.5,$5::text::numeric)")
            .bind(Uuid::new_v4()).bind(account).bind(day).bind(mode).bind(actual).execute(pool).await.unwrap();
    }
    let date = |s: &str| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap();
    let daily = store
        .request_statistics(
            owner,
            &StatisticsRange::new(
                date("2026-01-01"),
                date("2026-01-31"),
                StatisticsBucket::Day,
            )
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(daily.rows.len(), 2);
    assert_eq!(daily.rows[0].requests, 2);
    assert_eq!(daily.rows[0].unconfirmed, 1);
    assert_eq!(daily.rows[0].spent_usd, "0.123456789");
    assert_eq!(daily.rows[0].reserved_usd, "0.5");
    assert_eq!(daily.rows[1].requests, 1, "zero-charge calls still count");
    assert_eq!(daily.rows[1].unconfirmed, 0);
    assert_eq!(daily.timezone, "Europe/Moscow");
    for (bucket, expected) in [
        (
            StatisticsBucket::Month,
            vec!["2025-12-01", "2026-01-01", "2026-01-01", "2026-02-01"],
        ),
        (
            StatisticsBucket::Year,
            vec!["2025-01-01", "2026-01-01", "2026-01-01", "2026-01-01"],
        ),
    ] {
        let stats = store
            .request_statistics(
                owner,
                &StatisticsRange::new(date("2025-12-31"), date("2026-02-01"), bucket).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            stats
                .rows
                .iter()
                .map(|r| r.period.as_str())
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(stats.rows.iter().map(|r| r.requests).sum::<i64>(), 5);
    }
    let foreign = store
        .request_statistics(
            other,
            &StatisticsRange::new(
                date("2026-01-01"),
                date("2026-01-01"),
                StatisticsBucket::Day,
            )
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(foreign.rows.len(), 1);
    assert_eq!(foreign.rows[0].requests, 1);
    assert_eq!(foreign.rows[0].spent_usd, "99");
    let empty = store
        .request_statistics(
            Uuid::new_v4(),
            &StatisticsRange::new(
                date("2026-01-01"),
                date("2026-01-01"),
                StatisticsBucket::Day,
            )
            .unwrap(),
        )
        .await
        .unwrap();
    assert!(empty.rows.is_empty());
}
