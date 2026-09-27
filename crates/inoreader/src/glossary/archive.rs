use super::*;
use reader_glossary::{ObservedPost, CHANNEL_USERNAME};
use std::{
    collections::HashSet,
    io::{BufRead, BufReader, Read},
    path::Path,
};

/// Validate the entire archive before writing. Streaming passes keep memory
/// bounded by the operator's configured record limit; original files are read-only.
pub(crate) async fn import(
    config: &Config,
    pool: sqlx::PgPool,
    root: PathBuf,
    owner: uuid::Uuid,
    workspace: uuid::Uuid,
) -> Result<(), Box<dyn std::error::Error>> {
    let policy = GlossaryPolicy::new(
        config
            .glossary
            .clone()
            .ok_or("glossary configuration is required")?,
    )?;
    let limit = policy.config().max_archive_line_bytes;
    let root = root.canonicalize()?;
    let scan_root = root.clone();
    let (inventory, expected, artifacts) =
        tokio::task::spawn_blocking(move || validate(&scan_root, limit)).await??;
    let store = PostgresGlossaryStore::new(pool);
    store.begin_archive(owner, workspace, &inventory).await?;
    // File reads run outside Tokio workers; only one bounded record is buffered.
    let (sender, mut receiver) = tokio::sync::mpsc::channel(1);
    let posts = root.join("posts.jsonl");
    let read = tokio::task::spawn_blocking(move || -> Result<(), GlossaryError> {
        read_posts(&posts, limit, |post| {
            sender
                .blocking_send(post)
                .map_err(|_| GlossaryError::Storage)
        })
    });
    while let Some(post) = receiver.recv().await {
        store.import_post(owner, workspace, &post).await?;
    }
    read.await??;
    for name in artifacts {
        let path = root.join(&name);
        let raw = tokio::task::spawn_blocking(move || bounded_file(&path, limit)).await??;
        store.import_artifact(owner, workspace, &name, &raw).await?;
    }
    store.finish_archive(owner, workspace, expected).await?;
    println!("Imported {expected} Telegram posts; original observations and formatting retained.");
    Ok(())
}
fn bounded_file(path: &Path, limit: usize) -> Result<String, GlossaryError> {
    let file = std::fs::File::open(path).map_err(|_| GlossaryError::Storage)?;
    let mut bytes = Vec::new();
    file.take(
        u64::try_from(limit)
            .map_err(|_| GlossaryError::Configuration)?
            .checked_add(1)
            .ok_or(GlossaryError::Configuration)?,
    )
    .read_to_end(&mut bytes)
    .map_err(|_| GlossaryError::Storage)?;
    if bytes.len() > limit {
        return Err(GlossaryError::Limit);
    }
    String::from_utf8(bytes).map_err(|_| GlossaryError::Formatting)
}
fn read_posts(
    path: &Path,
    limit: usize,
    mut consume: impl FnMut(ObservedPost) -> Result<(), GlossaryError>,
) -> Result<(), GlossaryError> {
    let mut reader = BufReader::new(std::fs::File::open(path).map_err(|_| GlossaryError::Storage)?);
    loop {
        let mut bytes = Vec::new();
        let n = reader
            .by_ref()
            .take(
                u64::try_from(limit)
                    .map_err(|_| GlossaryError::Configuration)?
                    .checked_add(1)
                    .ok_or(GlossaryError::Configuration)?,
            )
            .read_until(b'\n', &mut bytes)
            .map_err(|_| GlossaryError::Storage)?;
        if n == 0 {
            break;
        }
        if n > limit {
            return Err(GlossaryError::Limit);
        }
        // Preserve the JSONL record bytes, including its authored delimiter.
        consume(ObservedPost::archive(
            String::from_utf8(bytes).map_err(|_| GlossaryError::Formatting)?,
        )?)?;
    }
    Ok(())
}
fn validate(root: &Path, limit: usize) -> Result<(String, usize, Vec<String>), GlossaryError> {
    let inventory = bounded_file(&root.join("inventory.json"), limit)?;
    let value: serde_json::Value =
        serde_json::from_str(&inventory).map_err(|_| GlossaryError::Protocol)?;
    if value["channel"] != CHANNEL_USERNAME || value["schema_version"] != 1 {
        return Err(GlossaryError::Identity);
    }
    let expected = value["post_count"]
        .as_u64()
        .and_then(|v| usize::try_from(v).ok())
        .ok_or(GlossaryError::Protocol)?;
    let mut ids = HashSet::new();
    read_posts(&root.join("posts.jsonl"), limit, |p| {
        if ids.insert(p.id()) {
            Ok(())
        } else {
            Err(GlossaryError::Conflict)
        }
    })?;
    if ids.len() != expected {
        return Err(GlossaryError::Conflict);
    }
    let mut artifacts = Vec::new();
    for entry in std::fs::read_dir(root.join("raw")).map_err(|_| GlossaryError::Storage)? {
        let entry = entry.map_err(|_| GlossaryError::Storage)?;
        if !entry
            .file_type()
            .map_err(|_| GlossaryError::Storage)?
            .is_file()
        {
            return Err(GlossaryError::Unsupported);
        }
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| GlossaryError::Formatting)?;
        bounded_file(&entry.path(), limit)?;
        artifacts.push(format!("raw/{name}"));
    }
    artifacts.sort();
    Ok((inventory, expected, artifacts))
}
