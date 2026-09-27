//! Bounded icon refresh: keyset pages, bounded tasks, immediate persistence.
use super::*;

pub(super) async fn refresh(
    pool: sqlx::PgPool,
    fetcher: Arc<SecureWebFetcher<TokioDnsResolver, ReqwestPinnedTransport, RequestObserver>>,
    concurrency: usize,
    batch: usize,
    stop: &mut reader_runtime::Shutdown,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if concurrency == 0 || batch == 0 {
        return Err("invalid icon worker limits".into());
    }
    let batch = i64::try_from(batch)?;
    let mut cursor = String::new();
    loop {
        if stop.requested() {
            return Ok(());
        }
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT m.subscription_id,s.document FROM subscription_sources m JOIN sources s ON s.id=m.source_id LEFT JOIN subscription_icons i ON i.subscription_id=m.subscription_id WHERE i.subscription_id IS NULL AND m.subscription_id>$1 ORDER BY m.subscription_id LIMIT $2")
            .bind(&cursor).bind(batch).fetch_all(&pool).await?;
        let Some(last) = rows.last() else {
            return Ok(());
        };
        cursor = last.0.clone();
        let mut grouped = HashMap::<String, Vec<String>>::new();
        for (id, document) in rows {
            let source: SourceDefinition = serde_json::from_str(&document)?;
            let mut favicon = source.url().clone();
            favicon.set_path("/favicon.ico");
            favicon.set_query(None);
            favicon.set_fragment(None);
            grouped.entry(favicon.to_string()).or_default().push(id);
        }
        run_page(
            grouped.into_iter(), concurrency, stop,
            |(favicon, ids)| {
                let fetcher = fetcher.clone();
                async move {
                    let url = Url::parse(&favicon).map_err(|_| "invalid_icon_url")?;
                    let page = fetcher.fetch(&url, &CacheValidators::default()).await.map_err(|_| "icon_fetch_failed")?;
                    let media_type = page.content_type.as_deref().and_then(|v| v.split(';').next()).filter(|v| v.starts_with("image/")).ok_or("icon_not_image")?;
                    Ok((ids, format!("data:{media_type};base64,{}", BASE64.encode(page.body))))
                }
            },
            |(ids, data)| {
                let pool = pool.clone();
                async move {
                    sqlx::query("INSERT INTO subscription_icons(subscription_id,data_url,fetched_at_ms) SELECT id,$2,$3 FROM unnest($1::text[]) AS id ON CONFLICT(subscription_id) DO UPDATE SET data_url=EXCLUDED.data_url,fetched_at_ms=EXCLUDED.fetched_at_ms")
                        .bind(ids).bind(data).bind(chrono::Utc::now().timestamp_millis()).execute(&pool).await?;
                    Ok(())
                }
            },
        ).await?;
    }
}

type Icon = (Vec<String>, String);
type Error = Box<dyn std::error::Error + Send + Sync>;

/// Admit at most the configured tasks; persist each result before replacing its
/// slot. Shutdown stops admission and drains exactly the already accepted page.
async fn run_page<F, Fut, P, Persist>(
    mut input: impl Iterator<Item = (String, Vec<String>)>,
    concurrency: usize,
    stop: &reader_runtime::Shutdown,
    fetch: F,
    persist: P,
) -> Result<(), Error>
where
    F: Fn((String, Vec<String>)) -> Fut,
    Fut: std::future::Future<Output = Result<Icon, &'static str>> + Send + 'static,
    P: Fn(Icon) -> Persist,
    Persist: std::future::Future<Output = Result<(), Error>>,
{
    if concurrency == 0 {
        return Err("invalid icon worker concurrency".into());
    }
    let mut tasks = tokio::task::JoinSet::new();
    loop {
        while !stop.requested() && tasks.len() < concurrency {
            let Some(item) = input.next() else {
                break;
            };
            tasks.spawn(fetch(item));
        }
        let Some(result) = tasks.join_next().await else {
            break;
        };
        match result {
            Ok(Ok(icon)) => persist(icon).await?,
            Ok(Err(classification)) => {
                log::debug!("icon_refresh outcome=failed classification={classification}")
            }
            Err(_) => return Err("icon worker panicked".into()),
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "tests/icons.rs"]
mod tests;
