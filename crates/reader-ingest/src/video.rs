use crate::FetchError;
use reader_core::VideoMetadata;
use url::Url;
/// Only explicit YouTube player declarations and /shorts/ URLs qualify. A short
/// duration alone is not a Shorts declaration. Original HTML remains archived.
pub fn extract_video(html: &str, url: &Url) -> Result<Option<VideoMetadata>, FetchError> {
    if !matches!(
        url.host_str(),
        Some("www.youtube.com" | "youtube.com" | "youtu.be")
    ) {
        return Ok(None);
    }
    let details = html.find("\"videoDetails\"").and_then(|at| {
        let tail = &html[at + "\"videoDetails\"".len()..];
        let tail = tail.trim_start().strip_prefix(':')?.trim_start();
        serde_json::Deserializer::from_str(tail)
            .into_iter::<serde_json::Value>()
            .next()?
            .ok()
    });
    let duration_seconds = details
        .as_ref()
        .and_then(|v| v.get("lengthSeconds"))
        .map(|v| {
            v.as_str()
                .ok_or_else(|| FetchError::Rejected("invalid_video_duration".into()))
                .and_then(|v| {
                    v.parse::<u64>()
                        .map_err(|_| FetchError::Rejected("invalid_video_duration".into()))
                })
        })
        .transpose()?;
    let shorts = if url.path().starts_with("/shorts/") {
        Some(true)
    } else {
        details
            .as_ref()
            .and_then(|v| v.get("isShorts"))
            .and_then(|v| v.as_bool())
    };
    Ok(Some(VideoMetadata {
        duration_seconds,
        shorts,
    }))
}

/// The page's authored post flair is metadata; never infer it from title words.
pub fn extract_reddit_flair(html: &str, url: &Url) -> Option<String> {
    if !matches!(
        url.host_str(),
        Some("www.reddit.com" | "reddit.com" | "old.reddit.com")
    ) {
        return None;
    }
    let doc = scraper::Html::parse_document(html);
    let selector = scraper::Selector::parse(
        "shreddit-post-flair, .linkflairlabel, [data-testid='post-flair']",
    )
    .expect("static flair selector");
    doc.select(&selector).find_map(|node| {
        let value = node.text().collect::<String>();
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_owned())
    })
}
