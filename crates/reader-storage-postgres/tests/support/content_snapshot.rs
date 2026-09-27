use super::*;

pub async fn verify(
    pool: &PgPool,
    store: Arc<PostgresAiStore>,
    owner: Uuid,
    workspace: Uuid,
    article: Uuid,
    pointer: &reader_ingest::ContentManifestPointer,
) {
    // Pause exactly at manifest evaluation while another connection atomically
    // publishes a new generation and cleans up the old chunks. Separate SELECTs
    // under READ COMMITTED would read the old pointer and then no old chunks.
    sqlx::raw_sql("ALTER TABLE content_manifests RENAME TO snapshot_fixture_manifests;
        CREATE FUNCTION snapshot_fixture_gate(payload TEXT) RETURNS TEXT LANGUAGE plpgsql VOLATILE AS $$ BEGIN PERFORM pg_advisory_xact_lock(90210017); RETURN payload; END $$;
        CREATE VIEW content_manifests AS SELECT id,revision,snapshot_fixture_gate(document) AS document FROM snapshot_fixture_manifests;")
        .execute(pool).await.unwrap();
    let mut barrier = pool.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(90210017)")
        .execute(&mut *barrier)
        .await
        .unwrap();
    let reader = store.clone();
    let reading =
        tokio::spawn(async move { reader.article_input(owner, workspace, article).await });
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let waiting: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_locks WHERE locktype='advisory' AND objid=90210017 AND NOT granted)").fetch_one(pool).await.unwrap();
            if waiting { break; }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    }).await.unwrap();
    let mut replacement = pointer.clone();
    replacement.refresh_id = Uuid::new_v4();
    replacement.source_revision += 1;
    let record_id = pointer.record_id.as_uuid().to_string();
    let mut write = pool.begin().await.unwrap();
    sqlx::query("UPDATE snapshot_fixture_manifests SET document=$2 WHERE id=$1")
        .bind(&record_id)
        .bind(serde_json::to_string(&replacement).unwrap())
        .execute(&mut *write)
        .await
        .unwrap();
    sqlx::query("DELETE FROM staged_content_chunks WHERE record_id=$1")
        .bind(&record_id)
        .execute(&mut *write)
        .await
        .unwrap();
    sqlx::query("INSERT INTO staged_content_chunks(record_id,refresh_id,representation,ordinal,bytes) VALUES($1,$2,'safe',0,$3)").bind(&record_id).bind(replacement.refresh_id.to_string()).bind(b"<p>Replacement 99%.</p>".as_slice()).execute(&mut *write).await.unwrap();
    write.commit().await.unwrap();
    barrier.commit().await.unwrap();
    let ArticleInput::Ready(snapshot) = reading.await.unwrap().unwrap() else {
        panic!("must retain old complete snapshot");
    };
    assert_eq!(snapshot.text, "Exact source 12.5%.");
    assert_eq!(
        snapshot.source_revision,
        format!("{}/7/{}", record_id, pointer.refresh_id)
    );
    let ArticleInput::Ready(snapshot) = store
        .article_input(owner, workspace, article)
        .await
        .unwrap()
    else {
        panic!("must read published replacement");
    };
    assert_eq!(snapshot.text, "Replacement 99%.");
    // Restore the shared fixture exactly for subsequent AI tests.
    sqlx::raw_sql("DROP VIEW content_manifests; DROP FUNCTION snapshot_fixture_gate(TEXT); ALTER TABLE snapshot_fixture_manifests RENAME TO content_manifests;").execute(pool).await.unwrap();
    let mut restore = pool.begin().await.unwrap();
    sqlx::query("UPDATE content_manifests SET document=$2 WHERE id=$1")
        .bind(&record_id)
        .bind(serde_json::to_string(pointer).unwrap())
        .execute(&mut *restore)
        .await
        .unwrap();
    sqlx::query("UPDATE staged_content_chunks SET refresh_id=$2,bytes=$3 WHERE record_id=$1")
        .bind(record_id)
        .bind(pointer.refresh_id.to_string())
        .bind(b"<p>Exact source 12.5%.</p>".as_slice())
        .execute(&mut *restore)
        .await
        .unwrap();
    restore.commit().await.unwrap();
}
