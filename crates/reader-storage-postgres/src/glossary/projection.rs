use super::*;

pub(super) async fn apply_post(
    tx: &mut Transaction<'_, Postgres>,
    owner: Uuid,
    workspace: Uuid,
    post: &ObservedPost,
    receipt: Uuid,
    kind: &str,
) -> Result<(), GlossaryError> {
    let document = encode(post.projection())?;
    let previous = sqlx::query("SELECT document,source_document,edited_at,kind,parser_version FROM glossary_posts WHERE owner=$1 AND workspace=$2 AND message_id=$3 FOR UPDATE")
        .bind(owner).bind(workspace).bind(post.id()).fetch_optional(&mut **tx).await.map_err(storage)?;
    if let Some(row) = previous {
        let old: String = row.try_get("document").map_err(storage)?;
        let old_edit: Option<i64> = row.try_get("edited_at").map_err(storage)?;
        let old_kind: String = row.try_get("kind").map_err(storage)?;
        let old_source: String = row.try_get("source_document").map_err(storage)?;
        if old == document
            && row
                .try_get::<String, _>("parser_version")
                .map_err(storage)?
                == PARSER_VERSION
        {
            if kind == "bot" && post.edited_at() > old_edit {
                sqlx::query("UPDATE glossary_posts SET edited_at=$4,kind=$5,receipt=$6,source_document=$7 WHERE owner=$1 AND workspace=$2 AND message_id=$3")
                    .bind(owner).bind(workspace).bind(post.id()).bind(post.edited_at()).bind(kind).bind(receipt).bind(post.raw()).execute(&mut **tx).await.map_err(storage)?;
            }
            return Ok(());
        }
        if old_edit.is_some() && post.edited_at().is_some() && post.edited_at() < old_edit {
            return Ok(());
        }
        // A direct, explicitly newer Telegram edit is authoritative. Public HTML
        // capture time alone is not an edit version and must not roll content back.
        let newer = (old_source == post.raw() && old_kind == kind)
            || (kind == "bot"
                && post.edited_at().is_some()
                && (old_edit.is_none() || post.edited_at() > old_edit));
        if !newer {
            if kind == "archive" && old_kind == "bot" {
                return Ok(());
            }
            sqlx::query("UPDATE glossary_posts SET conflicted=true WHERE owner=$1 AND workspace=$2 AND message_id=$3")
                .bind(owner).bind(workspace).bind(post.id()).execute(&mut **tx).await.map_err(storage)?;
            return Ok(());
        }
    }
    let error = match post.projection() {
        PostProjection::Unindexed { error } => Some(error.as_str()),
        _ => None,
    };
    sqlx::query("INSERT INTO glossary_posts(owner,workspace,message_id,published_at,edited_at,kind,receipt,document,projection_error,source_document,parser_version) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11) ON CONFLICT(owner,workspace,message_id) DO UPDATE SET published_at=EXCLUDED.published_at,edited_at=EXCLUDED.edited_at,kind=EXCLUDED.kind,receipt=EXCLUDED.receipt,document=EXCLUDED.document,source_document=EXCLUDED.source_document,projection_error=EXCLUDED.projection_error,parser_version=EXCLUDED.parser_version,conflicted=CASE WHEN glossary_posts.source_document=EXCLUDED.source_document AND glossary_posts.kind=EXCLUDED.kind THEN glossary_posts.conflicted ELSE false END")
        .bind(owner).bind(workspace).bind(post.id()).bind(post.published_at()).bind(post.edited_at()).bind(kind).bind(receipt).bind(document).bind(error).bind(post.raw()).bind(PARSER_VERSION).execute(&mut **tx).await.map_err(storage)?;
    sqlx::query(
        "DELETE FROM glossary_definitions WHERE owner=$1 AND workspace=$2 AND message_id=$3",
    )
    .bind(owner)
    .bind(workspace)
    .bind(post.id())
    .execute(&mut **tx)
    .await
    .map_err(storage)?;
    if let PostProjection::Indexed { definitions, .. } = post.projection() {
        for definition in definitions {
            sqlx::query("INSERT INTO glossary_definitions(owner,workspace,message_id,position,term,term_hash,document) VALUES($1,$2,$3,$4,$5,$6,$7)")
                .bind(owner).bind(workspace).bind(post.id()).bind(i64::try_from(definition.position()).map_err(storage)?).bind(definition.term()).bind(term_hash(definition.term())?).bind(encode(definition)?).execute(&mut **tx).await.map_err(storage)?;
        }
    }
    sqlx::query("UPDATE glossary_channels SET revision=revision+1 WHERE owner=$1 AND workspace=$2")
        .bind(owner)
        .bind(workspace)
        .execute(&mut **tx)
        .await
        .map_err(storage)?;
    Ok(())
}

impl PostgresGlossaryStore {
    pub(super) async fn project_events(&self, batch_size: u16) -> Result<usize, GlossaryError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        // Consistent channel -> event -> post lock order with polling/history.
        let channel = sqlx::query("SELECT c.owner,c.workspace,c.binding FROM glossary_channels c JOIN workspaces w ON w.id=c.workspace::text AND w.document::jsonb->>'owner'=c.owner::text WHERE c.binding IS NOT NULL AND EXISTS(SELECT 1 FROM glossary_events e WHERE e.owner=c.owner AND e.workspace=c.workspace AND NOT e.applied AND e.error IS NULL) FOR UPDATE OF c SKIP LOCKED LIMIT 1")
            .fetch_optional(&mut *tx).await.map_err(storage)?;
        let Some(channel) = channel else {
            return Ok(0);
        };
        let owner: Uuid = channel.try_get("owner").map_err(storage)?;
        let workspace: Uuid = channel.try_get("workspace").map_err(storage)?;
        let binding: ChannelBinding =
            decode(channel.try_get::<&str, _>("binding").map_err(storage)?)?;
        let events = sqlx::query("SELECT update_id,receipt,document FROM glossary_events WHERE owner=$1 AND workspace=$2 AND NOT applied AND error IS NULL ORDER BY update_id FOR UPDATE LIMIT $3")
            .bind(owner).bind(workspace).bind(i64::from(batch_size)).fetch_all(&mut *tx).await.map_err(storage)?;
        let count = events.len();
        for event in events {
            let id: i64 = event.try_get("update_id").map_err(storage)?;
            let raw_id: Uuid = event.try_get("receipt").map_err(storage)?;
            let value: serde_json::Value =
                decode(event.try_get::<&str, _>("document").map_err(storage)?)?;
            let message = value
                .get("edited_channel_post")
                .or_else(|| value.get("channel_post"));
            let result = if let Some(message) = message {
                if message["chat"]["id"].as_i64() != Some(binding.channel_id()) {
                    Ok(()) // Explicit source filter; the complete raw event remains retained.
                } else {
                    match ObservedPost::telegram(message, binding.channel_id()) {
                        Ok(post) => {
                            apply_post(&mut tx, owner, workspace, &post, raw_id, "bot").await
                        }
                        Err(error) => Err(error),
                    }
                }
            } else if value.get("my_chat_member").is_some() {
                Ok(())
            } else {
                Err(GlossaryError::Unsupported)
            };
            if matches!(&result, Err(GlossaryError::Storage)) {
                return result.map(|_| 0);
            }
            let error = result.as_ref().err().map(ToString::to_string);
            sqlx::query("UPDATE glossary_events SET applied=$4,error=$5 WHERE owner=$1 AND workspace=$2 AND update_id=$3")
                .bind(owner).bind(workspace).bind(id).bind(result.is_ok()).bind(error).execute(&mut *tx).await.map_err(storage)?;
        }
        tx.commit().await.map_err(storage)?;
        Ok(count)
    }
}
