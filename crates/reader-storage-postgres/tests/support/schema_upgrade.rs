use super::*;

pub async fn verify(pool: &PgPool) {
    let mut connection = pool.acquire().await.unwrap();
    sqlx::raw_sql("CREATE SCHEMA upgrade_fixture; SET search_path TO upgrade_fixture; CREATE TABLE articles(id TEXT,document TEXT); CREATE TABLE library_dedup(workspace_id TEXT,article_id TEXT,dedup_key TEXT,revision BIGINT,document TEXT); CREATE TABLE ai_chats(id UUID,document TEXT);")
        .execute(&mut *connection).await.unwrap();
    let key = serde_json::json!({"title":"原文 <T>","description":"0.000000000000000001"});
    let article = serde_json::json!({"key":key,"state":{"read":true,"later":true},"revision":999});
    sqlx::query("INSERT INTO articles VALUES('w/a',$1)")
        .bind(article.to_string())
        .execute(&mut *connection)
        .await
        .unwrap();
    sqlx::query("INSERT INTO library_dedup VALUES('w','a',$1,0,'obsolete derived state')")
        .bind(key.to_string())
        .execute(&mut *connection)
        .await
        .unwrap();
    let old = serde_json::json!({"snapshot":{"text":"Source 中文\n\"quote\" & <T>","safe_html":"<p>source</p>"},"system_prompt":"Exact prompt\n  spacing", "generation_mode":{"standard":0.3},"cost_rates":{"input":"0.00000000001"},"max_output_tokens":4096,"limits":{"input":10000},"review":{"prompt":"Exact review"},"view":{"messages":[{"content":"Saved response"}]},"operations":[],"owner":"fixture"});
    sqlx::query("INSERT INTO ai_chats VALUES($1,$2)")
        .bind(Uuid::new_v4())
        .bind(old.to_string())
        .execute(&mut *connection)
        .await
        .unwrap();
    sqlx::raw_sql(include_str!("../../../../tools/upgrade_article_index.sql"))
        .execute(&mut *connection)
        .await
        .unwrap();
    sqlx::raw_sql(include_str!("../../../../tools/upgrade_ai_chat_inputs.sql"))
        .execute(&mut *connection)
        .await
        .unwrap();
    let saved: String = sqlx::query_scalar("SELECT document FROM articles")
        .fetch_one(&mut *connection)
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&saved).unwrap(),
        article
    );
    let (document, inputs): (String, String) =
        sqlx::query_as("SELECT document,inputs FROM ai_chats")
            .fetch_one(&mut *connection)
            .await
            .unwrap();
    let mut restored: serde_json::Value = serde_json::from_str(&document).unwrap();
    let pinned: serde_json::Value = serde_json::from_str(&inputs).unwrap();
    restored
        .as_object_mut()
        .unwrap()
        .extend(pinned.as_object().unwrap().clone());
    assert_eq!(
        restored, old,
        "upgrade preserves every source, prompt and history value"
    );
    // Invalid historical input rolls back the whole upgrade, retaining exact bytes.
    sqlx::raw_sql("DROP TABLE ai_chats; CREATE TABLE ai_chats(id UUID,document TEXT); INSERT INTO ai_chats VALUES(gen_random_uuid(),'broken original');").execute(&mut *connection).await.unwrap();
    assert!(
        sqlx::raw_sql(include_str!("../../../../tools/upgrade_ai_chat_inputs.sql"))
            .execute(&mut *connection)
            .await
            .is_err()
    );
    sqlx::query("ROLLBACK")
        .execute(&mut *connection)
        .await
        .unwrap();
    let saved: String = sqlx::query_scalar("SELECT document FROM ai_chats")
        .fetch_one(&mut *connection)
        .await
        .unwrap();
    assert_eq!(saved, "broken original");
    sqlx::raw_sql("SET search_path TO public; DROP SCHEMA upgrade_fixture CASCADE;")
        .execute(&mut *connection)
        .await
        .unwrap();
}
