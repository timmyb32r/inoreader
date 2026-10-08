use super::*;
use reader_ingest::ContentManifestPointer;

pub async fn verify(pool: &PgPool) {
    let id = Uuid::new_v4();
    let source = Uuid::new_v4();
    let refresh = Uuid::new_v4();
    let document = serde_json::json!({
        "id":id,"source_id":source,"upstream_id":"post","key":{
            "location":{"Url":{"exact":"https://example.com/post","fetch":"https://example.com/post"}},
            "title":"Post","description":null
        },"description_media_type":null,"feed_content_html":null,"published_at":null,"revision":0
    }).to_string();
    sqlx::query("INSERT INTO source_records(id,revision,document) VALUES($1,0,$2)")
        .bind(id.to_string())
        .bind(&document)
        .execute(pool)
        .await
        .unwrap();
    let pointer = ContentManifestPointer {
        reddit_flair: None,
        video: None,
        publication: None,
        record_id: reader_core::SourceRecordId::from_uuid(id),
        source_revision: 0,
        refresh_id: refresh,
        raw_chunks: 1,
        safe_html_chunks: 1,
        fetched_at: Utc::now(),
        final_url: url::Url::parse("https://example.com/post").unwrap(),
    };
    let manifest = serde_json::to_string(&pointer).unwrap();
    sqlx::query("INSERT INTO content_manifests(id,revision,document) VALUES($1,0,$2)")
        .bind(id.to_string())
        .bind(&manifest)
        .execute(pool)
        .await
        .unwrap();
    let bytes = b"<html><head><meta property='article:published_time' content='2020-02-03'></head><body>Retained content</body></html>";
    sqlx::query("INSERT INTO staged_content_chunks(record_id,refresh_id,representation,ordinal,bytes) VALUES($1,$2,'raw',0,$3)").bind(id.to_string()).bind(refresh.to_string()).bind(bytes.as_slice()).execute(pool).await.unwrap();
    let batch = std::num::NonZeroU32::new(1).unwrap();
    let preview = reader_storage_postgres::backfill_publication_dates(pool, batch, 4096, false)
        .await
        .unwrap();
    assert_eq!(preview[&source.to_string()].page_dates, 1);
    let unchanged: String =
        sqlx::query_scalar("SELECT document FROM content_manifests WHERE id=$1")
            .bind(id.to_string())
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(unchanged, manifest);
    let result = reader_storage_postgres::backfill_publication_dates(pool, batch, 4096, true)
        .await
        .unwrap();
    assert_eq!(result[&source.to_string()].updated, 1);
    let updated: String = sqlx::query_scalar("SELECT document FROM content_manifests WHERE id=$1")
        .bind(id.to_string())
        .fetch_one(pool)
        .await
        .unwrap();
    let updated: ContentManifestPointer = serde_json::from_str(&updated).unwrap();
    assert_eq!(updated.refresh_id, refresh);
    assert_eq!(
        reader_core::resolve_publication(updated.publication.as_ref().unwrap())
            .unwrap()
            .as_str(),
        "2020-02-03"
    );
    let unchanged: String = sqlx::query_scalar("SELECT document FROM source_records WHERE id=$1")
        .bind(id.to_string())
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(unchanged, document);
    let result = reader_storage_postgres::backfill_publication_dates(pool, batch, 4096, true)
        .await
        .unwrap();
    assert_eq!(result[&source.to_string()].updated, 0);
    sqlx::query("UPDATE staged_content_chunks SET ordinal=2 WHERE record_id=$1")
        .bind(id.to_string())
        .execute(pool)
        .await
        .unwrap();
    assert!(
        reader_storage_postgres::backfill_publication_dates(pool, batch, 4096, true)
            .await
            .is_err()
    );
    sqlx::query("DELETE FROM staged_content_chunks WHERE record_id=$1")
        .bind(id.to_string())
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM content_manifests WHERE id=$1")
        .bind(id.to_string())
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM source_records WHERE id=$1")
        .bind(id.to_string())
        .execute(pool)
        .await
        .unwrap();
}
